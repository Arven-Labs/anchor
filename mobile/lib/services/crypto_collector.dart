import '../domain/crypto_score.dart';
import '../models/crypto_reading.dart';
import 'coingecko_global_service.dart';
import 'coingecko_nvt_service.dart';
import 'crypto_score_repository.dart';
import 'crypto_source_http.dart';
import 'defillama_stablecoins_service.dart';
import 'defillama_tvl_service.dart';
import 'deribit_service.dart';
import 'fear_greed_service.dart';
import 'perp_derivatives_service.dart';
import 'ultrasound_service.dart';

/// O que uma rodada de coleta fez. Uma fonte fora do ar não derruba as demais:
/// o que falhou volta em [errors], uma linha por indicador, com moeda e nome.
class CollectionResult {
  final List<CryptoReading> readings;
  final List<String> errors;

  /// Indicadores que já tinham leitura de hoje (UTC) e nem foram pedidos.
  final int skipped;

  const CollectionResult({
    required this.readings,
    required this.errors,
    required this.skipped,
  });
}

/// Valor pronto para gravar. [readingDate] nulo = hoje (UTC); a fonte que
/// informa a data do dado (Fear & Greed) a preenche.
typedef _Fetched = ({double value, String source, String? readingDate});

/// Um indicador de uma moeda a buscar, na faixa (host) que o serve.
class _Job {
  final String coin;
  final String indicator;
  final String lane;
  final Future<_Fetched> Function() fetch;

  const _Job(this.coin, this.indicator, this.lane, this.fetch);
}

const _scoredCoins = [coinEth, coinBtc, coinAda, coinXrp, coinBnb];

/// Faixa de cada indicador que já tem fonte. Pedidos da mesma faixa saem um de
/// cada vez (limite de requisições é por host); faixas diferentes rodam juntas.
/// Indicador do conjunto de uma moeda que não está aqui ainda não tem fonte e é
/// ignorado, sem virar erro.
const _laneOf = {
  'funding_rate_7d': 'perp',
  'open_interest_change_30d': 'perp',
  'long_short_ratio': 'perp',
  'dvol': 'deribit',
  'put_call_ratio': 'deribit',
  'stablecoin_supply_change_30d': 'defillama',
  // O TVL fica na faixa das stablecoins: são dois hosts da DefiLlama, e o limite de
  // requisições é da DefiLlama, não de cada host.
  'tvl_trend': 'defillama',
  'net_issuance': 'ultrasound',
  'fees_vs_emission': 'ultrasound',
  'fear_greed_index': 'alternative.me',
  'btc_dominance': 'coingecko',
  'nvt_ratio': 'coingecko',
};

/// A dominância do BTC vai por último na faixa da CoinGecko. Uma rodada completa são
/// 6 chamadas (5 de NVT e a dominância) e a 6ª seguida leva 429 (medido na
/// Sessão 260): assim o que fica de fora é o indicador menos importante, e o toque
/// seguinte, depois do `Retry-After`, refaz só ele.
const _lastInLane = 'btc_dominance';

/// Busca os indicadores do score cripto nas fontes públicas e grava as leituras
/// pelo [CryptoScoreRepository]. Espelha `collect_indicators` do desktop
/// (`finance_api/crypto.rs`), com duas diferenças: pula o que já foi coletado
/// hoje e busca uma vez só os valores que são do mercado todo.
class CryptoCollector {
  final CryptoScoreRepository _repository;
  final PerpDerivativesService _perp;
  final DeribitService _deribit;
  final DefiLlamaStablecoinsService _stablecoins;
  final FearGreedService _fearGreed;
  final CoinGeckoGlobalService _coinGeckoGlobal;
  final DefiLlamaTvlService _tvl;
  final UltrasoundService _ultrasound;
  final CoinGeckoNvtService _nvt;
  final DateTime Function() _now;

  /// [now] é o relógio (injetável nos testes) e deve ser o mesmo do
  /// repositório, senão "hoje" para gravar e "hoje" para pular podem divergir.
  CryptoCollector({
    required this._repository,
    PerpDerivativesService? perp,
    DeribitService? deribit,
    DefiLlamaStablecoinsService? stablecoins,
    FearGreedService? fearGreed,
    CoinGeckoGlobalService? coinGeckoGlobal,
    DefiLlamaTvlService? tvl,
    UltrasoundService? ultrasound,
    CoinGeckoNvtService? nvt,
    DateTime Function()? now,
  }) : _perp = perp ?? PerpDerivativesService(),
       _deribit = deribit ?? DeribitService(),
       _stablecoins = stablecoins ?? DefiLlamaStablecoinsService(),
       _fearGreed = fearGreed ?? FearGreedService(),
       _coinGeckoGlobal = coinGeckoGlobal ?? CoinGeckoGlobalService(),
       _tvl = tvl ?? DefiLlamaTvlService(),
       _ultrasound = ultrasound ?? UltrasoundService(),
       _nvt = nvt ?? CoinGeckoNvtService(),
       _now = now ?? DateTime.now;

  /// Coleta o que falta hoje das [coins] (ETH, BTC, ADA, XRP e BNB; as demais
  /// são recusadas antes de qualquer pedido). Só lança [CryptoSourceException]
  /// se nada foi coletado **e** nada foi pulado; com leituras já feitas hoje, a
  /// falha de uma fonte volta em [CollectionResult.errors].
  Future<CollectionResult> collect(Iterable<String> coins) async {
    final wanted = _validated(coins);
    final today = _now().toUtc().toIso8601String().substring(0, 10);
    // Valores do mercado todo: buscados uma vez, gravados em cada moeda.
    final shared = <String, Future<_Fetched>>{};
    // Corpos baixados uma vez e lidos por mais de um indicador (a oferta do
    // ultrasound.money, 431 KB, serve ao net issuance e ao fees vs emission).
    final bodies = <String, Future<String>>{};

    final jobs = <_Job>[];
    var skipped = 0;
    for (final coin in wanted) {
      final done = await _repository.indicatorsCollectedToday(coin);
      for (final group in IndicatorGroup.forCoin(coin)) {
        for (final indicator in group.indicators(coin)) {
          final lane = _laneOf[indicator];
          if (lane == null) continue;
          if (done.contains(indicator)) {
            skipped++;
            continue;
          }
          jobs.add(
            _Job(
              coin,
              indicator,
              lane,
              _fetcher(coin, indicator, shared, bodies),
            ),
          );
        }
      }
    }

    final lanes = <String, List<_Job>>{};
    for (final job in jobs) {
      lanes.putIfAbsent(job.lane, () => []).add(job);
    }
    final readings = <CryptoReading>[];
    final errors = <String>[];
    await Future.wait([
      for (final lane in lanes.values)
        _runLane(_lastInLaneAtEnd(lane), today, readings, errors),
    ]);

    if (readings.isEmpty && skipped == 0 && errors.isNotEmpty) {
      throw CryptoSourceException('crypto score', errors.join('; '));
    }
    return CollectionResult(
      readings: readings,
      errors: errors,
      skipped: skipped,
    );
  }

  /// Mantém a ordem dos pedidos, com o indicador de [_lastInLane] no fim.
  List<_Job> _lastInLaneAtEnd(List<_Job> lane) => [
    ...lane.where((job) => job.indicator != _lastInLane),
    ...lane.where((job) => job.indicator == _lastInLane),
  ];

  List<String> _validated(Iterable<String> coins) {
    final result = <String>[];
    for (final coin in coins) {
      final upper = coin.toUpperCase();
      if (!_scoredCoins.contains(upper)) {
        throw ArgumentError(
          'O score cripto só cobre ETH, BTC, ADA, XRP e BNB, não $coin',
        );
      }
      if (!result.contains(upper)) result.add(upper);
    }
    return result;
  }

  /// Uma faixa: um pedido depois do outro. Só [CryptoSourceException] e
  /// [ArgumentError] (valor recusado pelo repositório) viram linha de erro; um
  /// bug sobe.
  Future<void> _runLane(
    List<_Job> lane,
    String today,
    List<CryptoReading> readings,
    List<String> errors,
  ) async {
    for (final job in lane) {
      try {
        final fetched = await job.fetch();
        readings.add(
          await _repository.recordReading(
            coin: job.coin,
            indicator: job.indicator,
            readingDate: fetched.readingDate ?? today,
            rawValue: fetched.value,
            source: fetched.source,
          ),
        );
      } on CryptoSourceException catch (e) {
        errors.add('${job.coin} ${job.indicator}: $e');
      } on ArgumentError catch (e) {
        errors.add('${job.coin} ${job.indicator}: ${e.message}');
      }
    }
  }

  Future<_Fetched> Function() _fetcher(
    String coin,
    String indicator,
    Map<String, Future<_Fetched>> shared,
    Map<String, Future<String>> bodies,
  ) {
    Future<_Fetched> sourced(Future<Sourced> pending) async {
      final (:value, :source) = await pending;
      return (value: value, source: source, readingDate: null);
    }

    Future<String> supplyBody() =>
        bodies.putIfAbsent('supply', () => _ultrasound.fetchSupply());
    Future<String> burnBody() =>
        bodies.putIfAbsent('burn', () => _ultrasound.fetchBurn());

    return switch (indicator) {
      'funding_rate_7d' => () => sourced(_perp.fundingRate7d(coin)),
      'open_interest_change_30d' => () => sourced(
        _perp.openInterestChange30d(coin),
      ),
      'long_short_ratio' => () => sourced(_perp.longShortRatio(coin)),
      'dvol' => () async => (
        value: await _deribit.dvol(coin),
        source: 'deribit',
        readingDate: null,
      ),
      'put_call_ratio' => () async => (
        value: await _deribit.putCallRatio(coin),
        source: 'deribit',
        readingDate: null,
      ),
      'stablecoin_supply_change_30d' => () => shared.putIfAbsent(
        indicator,
        () async => (
          value: await _stablecoins.supplyChange30d(),
          source: 'defillama',
          readingDate: null,
        ),
      ),
      'fear_greed_index' => () => shared.putIfAbsent(indicator, () async {
        final reading = await _fearGreed.latest();
        return (
          value: reading.value.toDouble(),
          source: 'alternative.me',
          readingDate: reading.readingDate,
        );
      }),
      'btc_dominance' => () => shared.putIfAbsent(
        indicator,
        () async => (
          value: await _coinGeckoGlobal.btcDominance(),
          source: 'coingecko',
          readingDate: null,
        ),
      ),
      'tvl_trend' => () async => (
        value: await _tvl.tvlTrend(coin),
        source: 'defillama',
        readingDate: null,
      ),
      'net_issuance' => () async => (
        value: parseNetIssuanceAnnualizedPct(await supplyBody()),
        source: 'ultrasound.money',
        readingDate: null,
      ),
      // A oferta primeiro: se ela falha, a queima nem é pedida.
      'fees_vs_emission' => () async {
        final supply = await supplyBody();
        final burn = await burnBody();
        return (
          value: parseFeesVsEmissionRatio(supply, burn),
          source: 'ultrasound.money',
          readingDate: null,
        );
      },
      'nvt_ratio' => () async => (
        value: await _nvt.nvtVsMa90(coin),
        source: 'coingecko',
        readingDate: null,
      ),
      _ => throw StateError('indicador sem fonte: $indicator'),
    };
  }
}
