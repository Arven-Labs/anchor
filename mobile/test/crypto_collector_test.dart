import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/crypto_score.dart';
import 'package:anchor_mobile/models/crypto_reading.dart';
import 'package:anchor_mobile/services/bitcoin_data_service.dart';
import 'package:anchor_mobile/services/coingecko_global_service.dart';
import 'package:anchor_mobile/services/coingecko_nvt_service.dart';
import 'package:anchor_mobile/services/coinmetrics_service.dart';
import 'package:anchor_mobile/services/crypto_collector.dart';
import 'package:anchor_mobile/services/crypto_score_repository.dart';
import 'package:anchor_mobile/services/crypto_source_http.dart';
import 'package:anchor_mobile/services/defillama_stablecoins_service.dart';
import 'package:anchor_mobile/services/defillama_tvl_service.dart';
import 'package:anchor_mobile/services/defillama_yields_service.dart';
import 'package:anchor_mobile/services/deribit_service.dart';
import 'package:anchor_mobile/services/fear_greed_service.dart';
import 'package:anchor_mobile/services/perp_derivatives_service.dart';
import 'package:anchor_mobile/services/ultrasound_service.dart';

// Respostas reais (ou copiadas dos testes do desktop) de cada fonte, as mesmas
// dos testes dos serviços. Servem a qualquer moeda: o coletor não olha o corpo.
const _binanceFunding =
    r'[{"symbol":"ETHUSDT","fundingTime":1792080000000,"fundingRate":"0.00005355"},{"symbol":"ETHUSDT","fundingTime":1792108800001,"fundingRate":"0.00007466"},{"symbol":"ETHUSDT","fundingTime":1792137600001,"fundingRate":"0.00004648"}]';
const _binanceOpenInterest =
    r'[{"symbol":"ETHUSDT","sumOpenInterest":"2270359.16000000","timestamp":1791936000000},{"symbol":"ETHUSDT","sumOpenInterest":"2258306.77400000","timestamp":1792108800000}]';
const _binanceLongShort =
    r'[{"symbol":"ETHUSDT","longShortRatio":"2.6590","timestamp":1791936000000},{"symbol":"ETHUSDT","longShortRatio":"2.6390","timestamp":1792108800000}]';
const _bybitFunding =
    r'{"retCode":0,"retMsg":"OK","result":{"list":[{"fundingRate":"0.00005006"},{"fundingRate":"0.00009644"}]}}';
const _bybitOpenInterest =
    r'{"retCode":0,"retMsg":"OK","result":{"list":[{"openInterest":"781262.72"},{"openInterest":"774923.49"}]}}';
const _bybitRatio =
    r'{"retCode":0,"retMsg":"OK","result":{"list":[{"buyRatio":"0.724","sellRatio":"0.276"},{"buyRatio":"0.7228","sellRatio":"0.2772"}]}}';
const _dvol =
    r'{"jsonrpc":"2.0","result":{"data":[[1791936000000,50.1,52.0,49.0,51.5],[1792108800000,52.4,53.0,50.0,51.08]]}}';
const _options =
    r'{"jsonrpc":"2.0","result":[{"instrument_name":"ETH-25JUN27-2900-P","open_interest":508.0},{"instrument_name":"ETH-25JUN27-3500-C","open_interest":800.0},{"instrument_name":"ETH-30OCT26-2500-P","open_interest":200.0},{"instrument_name":"ETH-30OCT26-2500-C","open_interest":300.0},{"instrument_name":"ETH-PERPETUAL","open_interest":999.0}]}';
const _stablecoins =
    r'[{"date":"1000000","totalCirculatingUSD":{"peggedUSD":300.0}},{"date":"1864000","totalCirculatingUSD":{"peggedUSD":310.0}},{"date":"3592000","totalCirculatingUSD":{"peggedUSD":306.0}}]';
const _fearGreed =
    r'{"data":[{"value":"34","value_classification":"Fear","timestamp":"1792022400"}]}';
const _global =
    r'{"data":{"market_cap_percentage":{"btc":59.98151875544407,"eth":10.6}}}';

// TVL, ultrasound.money e NVT: os parsers têm respostas reais nos testes de
// cada serviço; aqui bastam corpos válidos de valor conhecido.
/// 31 pontos de 100 a 130: TVL +30 % em 30 posições.
final _tvl =
    '[${[for (var i = 0; i < 31; i++) '{"date":${1700000000 + i * 86400},"tvl":${100 + i}}'].join(',')}]';

/// Oferta 1000 → 1010 em 30 dias: +1 %, vezes 365/30 = 12,1667 % ao ano.
const _supply = r'{"d30":[{"supply":1000.0},{"supply":1010.0}]}';

/// Queima 30 sobre emissão bruta 10 + 30: razão 0,75.
const _burn = r'{"d30":{"sum":{"eth":30.0,"usd":1}}}';

/// 61 dias com NVT 20 todos os dias: hoje ÷ média = 1,0.
final _nvtChart = jsonEncode({
  'market_caps': [
    for (var i = 0; i < 61; i++) [1700000000000 + i * 86400000, 200],
  ],
  'total_volumes': [
    for (var i = 0; i < 61; i++) [1700000000000 + i * 86400000, 10],
  ],
});

/// Corpo do CoinMetrics para as métricas pedidas, igual para qualquer moeda (o
/// coletor não olha o corpo). Valores conhecidos: MVRV Z 1,7889 (a série do MVRV
/// pede 10.000 linhas; 4 bastam), Puell 365 ÷ 183, Mayer 200 ÷ 100,5, as três
/// tendências +30 % e netflow 0,5. A última linha é de 2026-10-28.
String _coinmetricsBody(Uri url) {
  final metrics = url.queryParameters['metrics']!.split(',');
  final pageSize = int.parse(url.queryParameters['page_size']!);
  final count = pageSize == 10000 ? 4 : pageSize;
  final lastDay = DateTime.utc(2026, 10, 28);
  Object cell(String metric, int i) => switch (metric) {
    'CapMrktCurUSD' => '${10 * (i + 1)}',
    'CapMVRVCur' => '2',
    'IssTotUSD' || 'PriceUSD' => '${i + 1}',
    'AdrActCnt' || 'SplyExNtv' || 'PriceBTC' => '${100 + i}',
    'FlowInExUSD' => '3',
    'FlowOutExUSD' => '1',
    // 3000 ETH por dia sobre 120 M: inflação bruta de 0,9125 % ao ano.
    'IssTotNtv' => '3000',
    'SplyCur' => '120000000',
    _ => throw StateError('métrica inesperada: $metric'),
  };
  return jsonEncode({
    'data': [
      for (var i = 0; i < count; i++)
        {
          'asset': 'eth',
          'time':
              '${lastDay.subtract(Duration(days: count - 1 - i)).toIso8601String().substring(0, 10)}T00:00:00.000000000Z',
          for (final metric in metrics) metric: cell(metric, i),
        },
    ],
  });
}

/// Série do stETH na DefiLlama yields: um ponto antigo absurdo (99) fora da janela
/// de 7 e 7 pontos de APY 3,0, o último de 2026-10-28. Staking real = 3,0 − 0,9125.
final _stethChart = jsonEncode({
  'status': 'success',
  'data': [
    for (var i = 0; i < 8; i++)
      {
        'timestamp': '2026-10-${21 + i}T23:01:00.000Z',
        'tvlUsd': 1000000,
        'apy': i == 0 ? 99 : 3.0,
      },
  ],
});

/// SOPR e NUPL com a data do dado 7 dias antes de "hoje" (2026-10-29), como o
/// plano grátis documenta: a data do dado e a da coleta são diferentes de propósito.
const _soprBody = r'{"d":"2026-10-22","unixTs":1792627200,"sopr":1.0057}';
const _nuplBody = r'{"d":"2026-10-22","unixTs":1792627200,"nupl":0.347}';

http.Response _bitcoinDataOk(Uri url) => http.Response(switch (url.path) {
  '/v1/sopr/last' => _soprBody,
  '/v1/nupl/last' => _nuplBody,
  _ => '',
}, url.path == '/v1/sopr/last' || url.path == '/v1/nupl/last' ? 200 : 404);

const _binance = 'fapi.binance.com';
const _bybit = 'api.bybit.com';
const _deribit = 'www.deribit.com';
const _llama = 'stablecoins.llama.fi';
const _llamaChain = 'api.llama.fi';
const _ultrasound = 'ultrasound.money';
const _alternative = 'api.alternative.me';
const _coingecko = 'api.coingecko.com';
const _yields = 'yields.llama.fi';
const _bitcoinData = 'bitcoin-data.com';
const _coinmetrics = 'community-api.coinmetrics.io';

/// O que o coletor busca, por moeda: o conjunto inteiro do score (o teste de
/// cobertura confere com `IndicatorGroup`).
const _ethIndicators = {
  'funding_rate_7d',
  'open_interest_change_30d',
  'long_short_ratio',
  'dvol',
  'put_call_ratio',
  'stablecoin_supply_change_30d',
  'fear_greed_index',
  'btc_dominance',
  'tvl_trend',
  'net_issuance',
  'fees_vs_emission',
  'nvt_ratio',
  'mvrv_z_score',
  'puell_multiple',
  'active_addresses_trend',
  'exchange_netflow',
  'exchange_supply_change_30d',
  'eth_btc_trend_30d',
  'mayer_multiple',
  'staking_yield',
};
const _btcIndicators = {
  'funding_rate_7d',
  'open_interest_change_30d',
  'long_short_ratio',
  'dvol',
  'put_call_ratio',
  'stablecoin_supply_change_30d',
  'fear_greed_index',
  'nvt_ratio',
  'mvrv_z_score',
  'puell_multiple',
  'active_addresses_trend',
  'exchange_netflow',
  'exchange_supply_change_30d',
  'mayer_multiple',
  'sopr',
  'nupl',
};

/// O que ADA, XRP e BNB têm em comum (mercado, stablecoins, Fear & Greed, NVT).
const _altIndicators = {
  'funding_rate_7d',
  'open_interest_change_30d',
  'long_short_ratio',
  'stablecoin_supply_change_30d',
  'fear_greed_index',
  'nvt_ratio',
  'mayer_multiple',
};

/// O CoinMetrics tem MVRV, Puell e endereços ativos do ADA, mas não os fluxos nem
/// a oferta nas exchanges.
const _adaIndicators = {
  ..._altIndicators,
  'mvrv_z_score',
  'puell_multiple',
  'active_addresses_trend',
};

/// O XRP não tem emissão em USD (`IssTotUSD` nulo), então fica sem Puell.
const _xrpIndicators = {
  ..._altIndicators,
  'mvrv_z_score',
  'active_addresses_trend',
};

/// As métricas on-chain do BNB no CoinMetrics são nulas (a série parou em 2019);
/// só o preço é atual, então só o Mayer entra.
const _bnbIndicators = {..._altIndicators, 'tvl_trend'};

const _everyCoin = ['ETH', 'BTC', 'ADA', 'XRP', 'BNB'];

/// Todas as moedas juntas, para contar o que uma rodada completa coleta.
final _allCoinsTotal =
    _ethIndicators.length +
    _btcIndicators.length +
    _adaIndicators.length +
    _xrpIndicators.length +
    _bnbIndicators.length;

typedef _Handler = FutureOr<http.Response> Function(Uri url);

http.Response _binanceOk(Uri url) => switch (url.path) {
  '/fapi/v1/fundingRate' => http.Response(_binanceFunding, 200),
  '/futures/data/openInterestHist' => http.Response(_binanceOpenInterest, 200),
  '/futures/data/globalLongShortAccountRatio' => http.Response(
    _binanceLongShort,
    200,
  ),
  _ => http.Response('not found', 404),
};

http.Response _bybitOk(Uri url) => switch (url.path) {
  '/v5/market/funding/history' => http.Response(_bybitFunding, 200),
  '/v5/market/open-interest' => http.Response(_bybitOpenInterest, 200),
  '/v5/market/account-ratio' => http.Response(_bybitRatio, 200),
  _ => http.Response('not found', 404),
};

http.Response _deribitOk(Uri url) =>
    url.path.endsWith('get_volatility_index_data')
    ? http.Response(_dvol, 200)
    : http.Response(_options, 200);

http.Response _ultrasoundOk(Uri url) => url.path.endsWith('burn-sums')
    ? http.Response(_burn, 200)
    : http.Response(_supply, 200);

http.Response _coingeckoOk(Uri url) => url.path.endsWith('/global')
    ? http.Response(_global, 200)
    : http.Response(_nvtChart, 200);

http.Response _unavailable(Uri url) => http.Response('', 503);

/// Rede falsa: cada host tem um handler (trocável por teste) e todo pedido fica
/// anotado.
class _Net {
  final requests = <Uri>[];
  final handlers = <String, _Handler>{
    _binance: _binanceOk,
    _bybit: _bybitOk,
    _deribit: _deribitOk,
    _llama: (_) => http.Response(_stablecoins, 200),
    _llamaChain: (_) => http.Response(_tvl, 200),
    _yields: (_) => http.Response(_stethChart, 200),
    _bitcoinData: _bitcoinDataOk,
    _ultrasound: _ultrasoundOk,
    _alternative: (_) => http.Response(_fearGreed, 200),
    _coingecko: _coingeckoOk,
    _coinmetrics: (url) => http.Response(_coinmetricsBody(url), 200),
  };

  late final MockClient client = MockClient((request) async {
    requests.add(request.url);
    final handler = handlers[request.url.host];
    if (handler == null) {
      throw StateError('host inesperado: ${request.url.host}');
    }
    return handler(request.url);
  });

  int count(String host) => requests.where((u) => u.host == host).length;
  List<String> paths(String host) => [
    for (final u in requests)
      if (u.host == host) u.path,
  ];
}

void main() {
  late Directory dir;
  late _Net net;
  late DateTime current;
  late CryptoScoreRepository repository;
  late CryptoCollector collector;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('anchor-collector-test-');
    await databaseFactory.setDatabasesPath(dir.path);
    await AppDatabase.resetForTest();
    net = _Net();
    current = DateTime.utc(2026, 10, 29, 12);
    repository = CryptoScoreRepository(now: () => current);
    collector = CryptoCollector(
      repository: repository,
      perp: PerpDerivativesService(client: net.client),
      deribit: DeribitService(client: net.client, now: () => current),
      stablecoins: DefiLlamaStablecoinsService(client: net.client),
      fearGreed: FearGreedService(client: net.client),
      coinGeckoGlobal: CoinGeckoGlobalService(client: net.client),
      tvl: DefiLlamaTvlService(client: net.client),
      ultrasound: UltrasoundService(client: net.client),
      nvt: CoinGeckoNvtService(client: net.client),
      coinmetrics: CoinMetricsService(client: net.client, now: () => current),
      yields: DefiLlamaYieldsService(client: net.client, now: () => current),
      bitcoinData: BitcoinDataService(client: net.client, now: () => current),
      now: () => current,
    );
  });

  tearDown(() async {
    await AppDatabase.resetForTest();
    await dir.delete(recursive: true);
  });

  Map<String, CryptoReading> byIndicator(CollectionResult r, String coin) => {
    for (final reading in r.readings)
      if (reading.coin == coin) reading.indicator: reading,
  };

  group('o que coleta', () {
    test(
      'ETH grava os 20 indicadores, cada um com a sua fonte',
      () async {
        final result = await collector.collect(['ETH']);

        final readings = byIndicator(result, 'ETH');
        expect(readings.keys.toSet(), _ethIndicators);
        expect(
          {for (final e in readings.entries) e.key: e.value.source},
          {
            'funding_rate_7d': 'binance',
            'open_interest_change_30d': 'binance',
            'long_short_ratio': 'binance',
            'dvol': 'deribit',
            'put_call_ratio': 'deribit',
            'stablecoin_supply_change_30d': 'defillama',
            'fear_greed_index': 'alternative.me',
            'btc_dominance': 'coingecko',
            'tvl_trend': 'defillama',
            'net_issuance': 'ultrasound.money',
            'fees_vs_emission': 'ultrasound.money',
            'nvt_ratio': 'coingecko',
            'mvrv_z_score': 'coinmetrics',
            'puell_multiple': 'coinmetrics',
            'active_addresses_trend': 'coinmetrics',
            'exchange_netflow': 'coinmetrics',
            'exchange_supply_change_30d': 'coinmetrics',
            'eth_btc_trend_30d': 'coinmetrics',
            'mayer_multiple': 'coinmetrics',
            'staking_yield': 'defillama+coinmetrics',
          },
        );
        expect(result.errors, isEmpty);
        expect(result.skipped, 0);
        // O que o dono vê no banco, não só no resultado.
        expect(
          await repository.listReadings('ETH'),
          hasLength(_ethIndicators.length),
        );
      },
    );

    test(
      'TVL, ultrasound.money e NVT gravam o valor que o parser calcula',
      () async {
        final readings = byIndicator(await collector.collect(['ETH']), 'ETH');

        expect(readings['tvl_trend']!.rawValue, closeTo(30.0, 1e-9));
        expect(readings['net_issuance']!.rawValue, closeTo(12.1667, 1e-4));
        expect(readings['fees_vs_emission']!.rawValue, closeTo(0.75, 1e-12));
        expect(readings['nvt_ratio']!.rawValue, closeTo(1.0, 1e-12));
      },
    );

    test('cada leitura é classificada pelos limites da moeda', () async {
      final readings = byIndicator(await collector.collect(['ETH']), 'ETH');

      expect(readings['dvol']!.rawValue, closeTo(51.08, 1e-9));
      expect(readings['dvol']!.signal, Signal.green.wire); // verde ≤ 55
      expect(readings['funding_rate_7d']!.rawValue, closeTo(6.376, 0.001));
      expect(
        readings['funding_rate_7d']!.signal,
        Signal.neutral.wire,
      ); // 5 a 20
    });

    test(
      'CoinMetrics grava o valor que o parser calcula para cada indicador',
      () async {
        final readings = byIndicator(await collector.collect(['ETH']), 'ETH');

        expect(
          readings['mvrv_z_score']!.rawValue,
          closeTo(1.7888543819998317, 1e-9),
        );
        expect(
          readings['puell_multiple']!.rawValue,
          closeTo(1.994535519125683, 1e-12),
        );
        expect(
          readings['mayer_multiple']!.rawValue,
          closeTo(1.9900497512437811, 1e-12),
        );
        for (final trend in [
          'active_addresses_trend',
          'exchange_supply_change_30d',
          'eth_btc_trend_30d',
        ]) {
          expect(readings[trend]!.rawValue, closeTo(30.0, 1e-9), reason: trend);
        }
        expect(readings['exchange_netflow']!.rawValue, closeTo(0.5, 1e-12));
      },
    );

    test(
      'staking yield real: a mediana do APY menos a inflação bruta (0,9125 %)',
      () async {
        final readings = byIndicator(await collector.collect(['ETH']), 'ETH');

        // Mediana 3,0 (o 99 antigo fica fora da janela de 7) − 0,9125.
        expect(
          readings['staking_yield']!.rawValue,
          closeTo(2.0875, 1e-12),
        );
      },
    );

    test('toda moeda é coletada por inteiro: o conjunto do score, sem sobra', () async {
      // Fecha a tarefa 4: nenhum indicador do score ficou sem fonte.
      final result = await collector.collect(_everyCoin);

      expect(result.errors, isEmpty);
      for (final coin in _everyCoin) {
        final expected = {
          for (final group in IndicatorGroup.forCoin(coin))
            ...group.indicators(coin),
        };
        expect(byIndicator(result, coin).keys.toSet(), expected, reason: coin);
      }
    });

    test('SOPR e NUPL gravam o valor da fonte com a data do dado, não a de hoje', () async {
      final readings = byIndicator(await collector.collect(['BTC']), 'BTC');

      expect(readings['sopr']!.rawValue, closeTo(1.0057, 1e-12));
      expect(readings['nupl']!.rawValue, closeTo(0.347, 1e-12));
      for (final indicator in ['sopr', 'nupl']) {
        expect(readings[indicator]!.source, 'bitcoin-data.com', reason: indicator);
        // A data do dado (7 dias antes), e não a da coleta (2026-10-29).
        expect(readings[indicator]!.readingDate, '2026-10-22', reason: indicator);
      }
    });

    test('cada moeda pega só os indicadores do seu conjunto', () async {
      final result = await collector.collect(['BTC', 'ADA', 'XRP', 'BNB']);

      expect(byIndicator(result, 'BTC').keys.toSet(), _btcIndicators);
      expect(byIndicator(result, 'ADA').keys.toSet(), _adaIndicators);
      expect(byIndicator(result, 'XRP').keys.toSet(), _xrpIndicators);
      expect(byIndicator(result, 'BNB').keys.toSet(), _bnbIndicators);
      expect(result.errors, isEmpty);
    });

    test('uma rodada completa grava 63 leituras', () async {
      final result = await collector.collect(_everyCoin);

      expect(_allCoinsTotal, 63);
      expect(result.readings, hasLength(63));
      expect(result.errors, isEmpty);
    });

    test(
      'Deribit só para ETH e BTC; dominância e ultrasound.money, só para o ETH',
      () async {
        await collector.collect(['ADA', 'XRP', 'BNB']);

        expect(net.count(_deribit), 0);
        expect(net.count(_ultrasound), 0);
        // A CoinGecko só é chamada para o NVT, nunca para o /global.
        expect(net.paths(_coingecko), everyElement(contains('market_chart')));
      },
    );

    test('cada moeda pede ao CoinMetrics só as métricas do seu conjunto', () async {
      Set<String> metricsOf(String asset) => {
        for (final url in net.requests)
          if (url.host == _coinmetrics && url.queryParameters['assets'] == asset)
            ...url.queryParameters['metrics']!.split(','),
      };

      await collector.collect(_everyCoin);

      expect(metricsOf('eth'), {
        'CapMrktCurUSD',
        'CapMVRVCur',
        'IssTotUSD',
        'AdrActCnt',
        'FlowInExUSD',
        'FlowOutExUSD',
        'SplyExNtv',
        'PriceBTC',
        'PriceUSD',
        // O staking real desconta a inflação bruta (só o ETH).
        'IssTotNtv',
        'SplyCur',
      });
      // O ETH/BTC é só do ETH; o BTC tem o resto.
      expect(metricsOf('btc'), {
        'CapMrktCurUSD',
        'CapMVRVCur',
        'IssTotUSD',
        'AdrActCnt',
        'FlowInExUSD',
        'FlowOutExUSD',
        'SplyExNtv',
        'PriceUSD',
      });
      // Sem fluxos nem oferta nas exchanges no ADA.
      expect(metricsOf('ada'), {
        'CapMrktCurUSD',
        'CapMVRVCur',
        'IssTotUSD',
        'AdrActCnt',
        'PriceUSD',
      });
      // Sem emissão em USD no XRP.
      expect(metricsOf('xrp'), {
        'CapMrktCurUSD',
        'CapMVRVCur',
        'AdrActCnt',
        'PriceUSD',
      });
      // As métricas on-chain do BNB são nulas: só o preço.
      expect(metricsOf('bnb'), {'PriceUSD'});
    });

    test('o TVL só é pedido para ETH e BNB, cada um na sua cadeia', () async {
      await collector.collect(['BTC', 'ADA', 'XRP']);
      expect(net.count(_llamaChain), 0);

      await collector.collect(['ETH', 'BNB']);

      expect(net.paths(_llamaChain), [
        '/v2/historicalChainTvl/Ethereum',
        '/v2/historicalChainTvl/BSC',
      ]);
    });

    test(
      'Binance bloqueada: a leitura sai da Bybit e a fonte diz isso',
      () async {
        net.handlers[_binance] = (_) => http.Response('blocked', 451);

        final readings = byIndicator(await collector.collect(['ETH']), 'ETH');

        expect(readings['funding_rate_7d']!.source, 'bybit');
        expect(readings['open_interest_change_30d']!.source, 'bybit');
        expect(readings['long_short_ratio']!.source, 'bybit');
      },
    );
  });

  group('valores globais', () {
    test(
      'Fear & Greed, stablecoins e dominância são buscados uma vez só',
      () async {
        final result = await collector.collect(['ETH', 'BTC', 'ADA']);

        expect(net.count(_llama), 1);
        expect(net.count(_alternative), 1);
        expect(
          net.paths(_coingecko).where((p) => p.endsWith('/global')),
          hasLength(1),
        );
        for (final coin in ['ETH', 'BTC', 'ADA']) {
          final readings = byIndicator(result, coin);
          expect(readings['fear_greed_index']!.rawValue, 34, reason: coin);
          expect(
            readings['stablecoin_supply_change_30d']!.rawValue,
            closeTo(2.0, 1e-9),
            reason: coin,
          );
        }
      },
    );
  });

  group('data da leitura', () {
    test(
      'mercado vale o dia UTC de hoje; Fear & Greed, o dia que a fonte informa',
      () async {
        current = DateTime.utc(2026, 10, 29, 23, 30);

        final readings = byIndicator(await collector.collect(['ETH']), 'ETH');

        expect(readings['funding_rate_7d']!.readingDate, '2026-10-29');
        expect(readings['dvol']!.readingDate, '2026-10-29');
        expect(readings['fear_greed_index']!.readingDate, '2026-10-15');
      },
    );
  });

  group('já coletado hoje', () {
    test('a segunda coleta no mesmo dia não pede nada à rede', () async {
      await collector.collect(['ETH']);
      final before = net.requests.length;

      final second = await collector.collect(['ETH']);

      expect(second.readings, isEmpty);
      expect(second.errors, isEmpty);
      expect(second.skipped, _ethIndicators.length);
      expect(net.requests.length, before);
      expect(
        await repository.listReadings('ETH'),
        hasLength(_ethIndicators.length),
      );
    });

    test(
      'leitura manual de hoje também conta e a fonte não é chamada para ela',
      () async {
        await repository.recordReading(
          coin: 'ETH',
          indicator: 'funding_rate_7d',
          readingDate: '2026-10-29',
          rawValue: 7,
          source: 'manual',
        );

        final result = await collector.collect(['ETH']);

        expect(result.skipped, 1);
        expect(result.readings, hasLength(_ethIndicators.length - 1));
        expect(net.paths(_binance), isNot(contains('/fapi/v1/fundingRate')));
      },
    );

    test('depois da meia-noite UTC coleta de novo', () async {
      current = DateTime.utc(2026, 10, 29, 23, 59);
      await collector.collect(['ETH']);

      current = DateTime.utc(2026, 10, 30, 0, 1);
      final second = await collector.collect(['ETH']);

      expect(second.readings, hasLength(_ethIndicators.length));
      expect(second.skipped, 0);
      expect(
        byIndicator(second, 'ETH')['funding_rate_7d']!.readingDate,
        '2026-10-30',
      );
    });

    test('só o que falhou é tentado de novo', () async {
      net.handlers[_deribit] = _unavailable;
      final first = await collector.collect(['ETH']);
      expect(first.readings, hasLength(_ethIndicators.length - 2));
      expect(first.errors, hasLength(2));

      net.handlers[_deribit] = _deribitOk;
      final callsBefore = net.requests.length;
      final second = await collector.collect(['ETH']);

      expect(byIndicator(second, 'ETH').keys.toSet(), {
        'dvol',
        'put_call_ratio',
      });
      expect(second.skipped, _ethIndicators.length - 2);
      expect(net.requests.skip(callsBefore).map((u) => u.host).toSet(), {
        _deribit,
      });
    });
  });

  group('falha parcial', () {
    test(
      'uma fonte fora não derruba as outras e o erro nomeia moeda e indicador',
      () async {
        net.handlers[_deribit] = _unavailable;

        final result = await collector.collect(['ETH']);

        expect(
          byIndicator(result, 'ETH'),
          hasLength(_ethIndicators.length - 2),
        );
        expect(result.errors, hasLength(2));
        expect(
          result.errors,
          everyElement(allOf(startsWith('ETH '), contains('Deribit'))),
        );
        expect(result.errors.any((e) => e.contains('dvol')), isTrue);
        expect(result.errors.any((e) => e.contains('put_call_ratio')), isTrue);
      },
    );

    test(
      'nenhum indicador coletado e nada pulado: lança com todas as linhas',
      () async {
        for (final host in [
          _binance,
          _bybit,
          _deribit,
          _llama,
          _llamaChain,
          _yields,
          _bitcoinData,
          _ultrasound,
          _alternative,
          _coingecko,
          _coinmetrics,
        ]) {
          net.handlers[host] = _unavailable;
        }

        await expectLater(
          collector.collect(['ETH']),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              allOf([
                for (final indicator in _ethIndicators)
                  contains('ETH $indicator'),
              ]),
            ),
          ),
        );
        expect(await repository.listReadings('ETH'), isEmpty);
      },
    );

    test(
      'com leituras já feitas hoje, falha nova volta no resultado, sem lançar',
      () async {
        net.handlers[_deribit] = _unavailable;
        await collector.collect(['ETH']);

        final second = await collector.collect(['ETH']);

        expect(second.readings, isEmpty);
        expect(second.skipped, _ethIndicators.length - 2);
        expect(second.errors, hasLength(2));
      },
    );

    test('bug (não é erro de fonte) não é engolido', () async {
      net.handlers[_deribit] = (_) => throw StateError('bug');

      await expectLater(collector.collect(['ETH']), throwsStateError);
    });
  });

  group('entrada', () {
    test(
      'moeda sem score próprio é recusada antes de qualquer pedido',
      () async {
        await expectLater(
          collector.collect(['ETH', 'SOL']),
          throwsArgumentError,
        );

        expect(net.requests, isEmpty);
        expect(await repository.listReadings('ETH'), isEmpty);
      },
    );

    test('aceita minúsculas e coleta uma vez moeda repetida', () async {
      final result = await collector.collect(['eth', 'ETH', 'Eth']);

      expect(byIndicator(result, 'ETH'), hasLength(_ethIndicators.length));
      expect(net.count(_alternative), 1);
      expect(
        await repository.listReadings('ETH'),
        hasLength(_ethIndicators.length),
      );
    });

    test('lista vazia não faz nada', () async {
      final result = await collector.collect([]);

      expect(result.readings, isEmpty);
      expect(result.errors, isEmpty);
      expect(result.skipped, 0);
      expect(net.requests, isEmpty);
    });
  });

  group('oferta do ultrasound.money', () {
    test('é baixada uma vez e serve aos dois indicadores', () async {
      await collector.collect(['ETH']);

      expect(net.paths(_ultrasound), [
        '/api/v2/fees/supply-over-time',
        '/api/v2/fees/burn-sums',
      ]);
    });

    test(
      'queima fora: só o fees vs emission falha, o net issuance sai',
      () async {
        net.handlers[_ultrasound] = (url) => url.path.endsWith('burn-sums')
            ? http.Response('', 503)
            : _ultrasoundOk(url);

        final result = await collector.collect(['ETH']);

        expect(byIndicator(result, 'ETH').keys, contains('net_issuance'));
        expect(
          byIndicator(result, 'ETH').keys,
          isNot(contains('fees_vs_emission')),
        );
        expect(result.errors, hasLength(1));
        expect(
          result.errors.single,
          allOf(
            contains('ETH fees_vs_emission'),
            contains('ultrasound.money burn'),
          ),
        );
      },
    );

    test(
      'oferta fora: os dois falham com uma tentativa só, sem pedir a queima',
      () async {
        net.handlers[_ultrasound] = _unavailable;

        final result = await collector.collect(['ETH']);

        expect(
          result.errors.where((e) => e.contains('ultrasound.money supply')),
          hasLength(2),
        );
        expect(
          result.errors.any((e) => e.contains('ETH net_issuance')),
          isTrue,
        );
        expect(
          result.errors.any((e) => e.contains('ETH fees_vs_emission')),
          isTrue,
        );
        expect(net.paths(_ultrasound), ['/api/v2/fees/supply-over-time']);
      },
    );
  });

  group('limite da CoinGecko (429)', () {
    const allCoins = ['ETH', 'BTC', 'ADA', 'XRP', 'BNB'];

    test(
      'a dominância é a última da faixa, mesmo sendo do primeiro da lista',
      () async {
        await collector.collect(['ETH', 'BTC']);

        expect(net.paths(_coingecko), [
          '/api/v3/coins/ethereum/market_chart',
          '/api/v3/coins/bitcoin/market_chart',
          '/api/v3/global',
        ]);
      },
    );

    test('os pedidos à CoinGecko saem um de cada vez', () async {
      var inFlight = 0;
      var maxInFlight = 0;
      net.handlers[_coingecko] = (url) async {
        inFlight++;
        maxInFlight = math.max(maxInFlight, inFlight);
        await Future<void>.delayed(const Duration(milliseconds: 5));
        inFlight--;
        return _coingeckoOk(url);
      };

      await collector.collect(allCoins);

      expect(net.count(_coingecko), 6); // 5 de NVT e a dominância
      expect(maxInFlight, 1);
    });

    test(
      'a 6ª chamada seguida leva 429: só a dominância fica de fora e o toque seguinte a completa',
      () async {
        // Medido ao vivo: 5 chamadas passam e a 6ª dá 429 com Retry-After de 60 s.
        var calls = 0;
        net.handlers[_coingecko] = (url) {
          calls++;
          return calls == 6 ? http.Response('', 429) : _coingeckoOk(url);
        };

        final first = await collector.collect(allCoins);

        expect(first.errors, hasLength(1));
        expect(
          first.errors.single,
          allOf(contains('ETH btc_dominance'), contains('CoinGecko global')),
        );
        expect(first.readings, hasLength(_allCoinsTotal - 1));
        expect(
          first.readings.where((r) => r.indicator == 'nvt_ratio'),
          hasLength(5),
        );

        net.handlers[_coingecko] = _coingeckoOk; // 60 s depois
        final callsBefore = net.requests.length;
        final second = await collector.collect(allCoins);

        expect(second.readings.map((r) => r.indicator), ['btc_dominance']);
        expect(second.errors, isEmpty);
        expect(second.skipped, _allCoinsTotal - 1);
        expect(net.requests.skip(callsBefore).map((u) => u.path), [
          '/api/v3/global',
        ]);
      },
    );
  });

  group('DefiLlama', () {
    test(
      'TVL e stablecoins saem um de cada vez, mesmo em hosts diferentes',
      () async {
        var inFlight = 0;
        var maxInFlight = 0;
        Future<http.Response> slow(String body) async {
          inFlight++;
          maxInFlight = math.max(maxInFlight, inFlight);
          await Future<void>.delayed(const Duration(milliseconds: 5));
          inFlight--;
          return http.Response(body, 200);
        }

        net.handlers[_llama] = (_) => slow(_stablecoins);
        net.handlers[_llamaChain] = (_) => slow(_tvl);
        net.handlers[_yields] = (_) => slow(_stethChart);

        await collector.collect(['ETH', 'BNB']);

        // Stablecoins uma vez (valor global), o TVL de ETH e de BNB e o APY do
        // stETH (staking do ETH).
        expect(
          net.count(_llama) + net.count(_llamaChain) + net.count(_yields),
          4,
        );
        expect(maxInFlight, 1);
      },
    );

    test('yields fora do ar: só o staking yield falha, o resto sai', () async {
      net.handlers[_yields] = _unavailable;

      final result = await collector.collect(['ETH']);

      expect(
        byIndicator(result, 'ETH').keys.toSet(),
        _ethIndicators.difference({'staking_yield'}),
      );
      expect(result.errors, hasLength(1));
      expect(
        result.errors.single,
        allOf(contains('ETH staking_yield'), contains('DefiLlama yields')),
      );
    });

    test('APY velho (série parada) falha o staking, sem gravar valor velho', () async {
      net.handlers[_yields] = (_) => http.Response(
        _stethChart.replaceAll('2026-10-', '2026-09-'),
        200,
      );

      final result = await collector.collect(['ETH']);

      expect(byIndicator(result, 'ETH').keys, isNot(contains('staking_yield')));
      expect(result.errors.single, allOf(contains('staking_yield'), contains('stale')));
    });
  });

  group('CoinMetrics', () {
    test('os pedidos saem um de cada vez (mesmo host)', () async {
      var inFlight = 0;
      var maxInFlight = 0;
      net.handlers[_coinmetrics] = (url) async {
        inFlight++;
        maxInFlight = math.max(maxInFlight, inFlight);
        await Future<void>.delayed(const Duration(milliseconds: 5));
        inFlight--;
        return http.Response(_coinmetricsBody(url), 200);
      };

      // Sem o ETH: o job do staking pede a inflação ao CoinMetrics de dentro da faixa
      // `defillama` (de propósito, para a DefiLlama seguir um pedido por vez), então
      // pode sobrepor a faixa `coinmetrics`. O limite do servidor (6.000 por 20 s)
      // torna isso irrelevante.
      await collector.collect(['BTC', 'ADA', 'XRP', 'BNB']);

      // BTC 6, ADA 4, XRP 3 e BNB 1 (Mayer): um pedido por indicador.
      expect(net.count(_coinmetrics), 14);
      expect(maxInFlight, 1);
    });

    test('fora do ar: só os indicadores dele falham, o resto sai', () async {
      net.handlers[_coinmetrics] = _unavailable;

      final result = await collector.collect(['ETH']);

      expect(
        byIndicator(result, 'ETH').keys.toSet(),
        _ethIndicators.difference({
          'staking_yield', // a inflação bruta também vem do CoinMetrics
          'mvrv_z_score',
          'puell_multiple',
          'active_addresses_trend',
          'exchange_netflow',
          'exchange_supply_change_30d',
          'eth_btc_trend_30d',
          'mayer_multiple',
        }),
      );
      expect(result.errors, hasLength(8));
      expect(
        result.errors,
        everyElement(allOf(startsWith('ETH '), contains('CoinMetrics'))),
      );
    });

    test('dado velho de uma moeda vira erro só dela e deixa as outras', () async {
      // O Community parou de atualizar o BNB em 2019: a última linha é antiga.
      net.handlers[_coinmetrics] = (url) {
        if (url.queryParameters['assets'] != 'bnb') {
          return http.Response(_coinmetricsBody(url), 200);
        }
        return http.Response(
          _coinmetricsBody(url).replaceAll('2026-10-', '2019-04-'),
          200,
        );
      };

      final result = await collector.collect(['ETH', 'BNB']);

      expect(byIndicator(result, 'ETH').keys.toSet(), _ethIndicators);
      expect(byIndicator(result, 'BNB').keys, isNot(contains('mayer_multiple')));
      expect(result.errors, hasLength(1));
      expect(
        result.errors.single,
        allOf(contains('BNB mayer_multiple'), contains('stale')),
      );
    });

    test('depois de coletado hoje, nada é pedido de novo', () async {
      await collector.collect(_everyCoin);
      final before = net.count(_coinmetrics);

      final second = await collector.collect(_everyCoin);

      expect(second.readings, isEmpty);
      expect(net.count(_coinmetrics), before);
    });
  });

  group('cota do bitcoin-data.com (10 por hora, 15 por dia)', () {
    test('só o BTC chama a fonte, uma chamada por indicador', () async {
      await collector.collect(['ETH', 'ADA', 'XRP', 'BNB']);
      expect(net.count(_bitcoinData), 0);

      await collector.collect(['BTC']);

      expect(net.paths(_bitcoinData), ['/v1/sopr/last', '/v1/nupl/last']);
    });

    test('a segunda coleta no mesmo dia não gasta nenhuma chamada', () async {
      await collector.collect(['BTC']);
      expect(net.count(_bitcoinData), 2);

      // As leituras têm `reading_date` de 7 dias atrás; o que impede a chamada é
      // terem sido CRIADAS hoje.
      final second = await collector.collect(['BTC']);

      expect(second.readings, isEmpty);
      expect(net.count(_bitcoinData), 2);
    });

    test('SOPR coletado e NUPL com erro: o toque seguinte faz só 1 chamada, a do NUPL', () async {
      net.handlers[_bitcoinData] = (url) => url.path.contains('nupl')
          ? http.Response('', 503)
          : _bitcoinDataOk(url);
      final first = await collector.collect(['BTC']);
      expect(byIndicator(first, 'BTC').keys, contains('sopr'));
      expect(byIndicator(first, 'BTC').keys, isNot(contains('nupl')));
      expect(
        first.errors.where((e) => e.contains('BTC nupl') && e.contains('bitcoin-data.com')),
        hasLength(1),
      );

      net.handlers[_bitcoinData] = _bitcoinDataOk;
      final before = net.count(_bitcoinData);
      final second = await collector.collect(['BTC']);

      expect(second.readings.map((r) => r.indicator), ['nupl']);
      expect(net.count(_bitcoinData) - before, 1);
      expect(net.paths(_bitcoinData).last, '/v1/nupl/last');
    });

    test('depois da meia-noite UTC busca de novo as duas', () async {
      current = DateTime.utc(2026, 10, 29, 23, 59);
      await collector.collect(['BTC']);

      current = DateTime.utc(2026, 10, 30, 0, 1);
      await collector.collect(['BTC']);

      expect(net.count(_bitcoinData), 4);
    });

    test('429 (cota esgotada): os dois falham com uma tentativa cada, sem retry, e o resto do BTC sai', () async {
      net.handlers[_bitcoinData] = (_) => http.Response('', 429);

      final result = await collector.collect(['BTC']);

      expect(net.count(_bitcoinData), 2);
      expect(result.errors, hasLength(2));
      expect(
        result.errors,
        everyElement(allOf(startsWith('BTC '), contains('bitcoin-data.com'))),
      );
      expect(
        byIndicator(result, 'BTC').keys.toSet(),
        _btcIndicators.difference({'sopr', 'nupl'}),
      );
    });

    test('dado parado da fonte (mais de 10 dias) falha sem gravar leitura velha', () async {
      net.handlers[_bitcoinData] = (url) => http.Response(
        _bitcoinDataOk(url).body.replaceAll('2026-10-22', '2026-10-01'),
        200,
      );

      final result = await collector.collect(['BTC']);

      expect(byIndicator(result, 'BTC').keys, isNot(contains('sopr')));
      expect(result.errors.where((e) => e.contains('stale')), hasLength(2));
    });

    test('SOPR e NUPL saem um de cada vez (mesmo host)', () async {
      var inFlight = 0;
      var maxInFlight = 0;
      net.handlers[_bitcoinData] = (url) async {
        inFlight++;
        maxInFlight = math.max(maxInFlight, inFlight);
        await Future<void>.delayed(const Duration(milliseconds: 5));
        inFlight--;
        return _bitcoinDataOk(url);
      };

      await collector.collect(['BTC']);

      expect(net.count(_bitcoinData), 2);
      expect(maxInFlight, 1);
    });
  });

  group('ordem dos pedidos', () {
    test('hosts diferentes rodam em paralelo', () async {
      // A Binance só responde depois que a Deribit foi chamada. Em sequência
      // ela esperaria em vão, estouraria o tempo e a leitura sairia da Bybit.
      final deribitSeen = Completer<void>();
      net.handlers[_deribit] = (url) {
        if (!deribitSeen.isCompleted) deribitSeen.complete();
        return _deribitOk(url);
      };
      net.handlers[_binance] = (url) async {
        await deribitSeen.future.timeout(const Duration(seconds: 2));
        return _binanceOk(url);
      };

      final readings = byIndicator(await collector.collect(['ETH']), 'ETH');

      expect(readings['funding_rate_7d']!.source, 'binance');
    });

    test('no mesmo host os pedidos saem um de cada vez', () async {
      var inFlight = 0;
      var maxInFlight = 0;
      net.handlers[_binance] = (url) async {
        inFlight++;
        maxInFlight = math.max(maxInFlight, inFlight);
        await Future<void>.delayed(const Duration(milliseconds: 5));
        inFlight--;
        return _binanceOk(url);
      };

      await collector.collect(['ETH', 'BTC', 'ADA', 'XRP', 'BNB']);

      expect(net.count(_binance), 15);
      expect(maxInFlight, 1);
    });

    test('a Bybit só é chamada depois que a Binance falha', () async {
      await collector.collect(['ETH']);

      expect(net.count(_bybit), 0);
    });
  });
}
