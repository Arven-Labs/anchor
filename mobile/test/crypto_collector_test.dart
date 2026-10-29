import 'dart:async';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:sqflite/sqflite.dart';

import 'package:anchor_mobile/data/db.dart';
import 'package:anchor_mobile/domain/crypto_score.dart';
import 'package:anchor_mobile/models/crypto_reading.dart';
import 'package:anchor_mobile/services/coingecko_global_service.dart';
import 'package:anchor_mobile/services/crypto_collector.dart';
import 'package:anchor_mobile/services/crypto_score_repository.dart';
import 'package:anchor_mobile/services/crypto_source_http.dart';
import 'package:anchor_mobile/services/defillama_stablecoins_service.dart';
import 'package:anchor_mobile/services/deribit_service.dart';
import 'package:anchor_mobile/services/fear_greed_service.dart';
import 'package:anchor_mobile/services/perp_derivatives_service.dart';

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

const _binance = 'fapi.binance.com';
const _bybit = 'api.bybit.com';
const _deribit = 'www.deribit.com';
const _llama = 'stablecoins.llama.fi';
const _alternative = 'api.alternative.me';
const _coingecko = 'api.coingecko.com';

/// O que o coletor sabe buscar hoje, por moeda; o resto do conjunto de cada
/// moeda (TVL, NVT, on-chain...) entra nas próximas tarefas.
const _ethIndicators = {
  'funding_rate_7d',
  'open_interest_change_30d',
  'long_short_ratio',
  'dvol',
  'put_call_ratio',
  'stablecoin_supply_change_30d',
  'fear_greed_index',
  'btc_dominance',
};
const _btcIndicators = {
  'funding_rate_7d',
  'open_interest_change_30d',
  'long_short_ratio',
  'dvol',
  'put_call_ratio',
  'stablecoin_supply_change_30d',
  'fear_greed_index',
};
const _altIndicators = {
  'funding_rate_7d',
  'open_interest_change_30d',
  'long_short_ratio',
  'stablecoin_supply_change_30d',
  'fear_greed_index',
};

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
    _alternative: (_) => http.Response(_fearGreed, 200),
    _coingecko: (_) => http.Response(_global, 200),
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
      'ETH grava os 8 indicadores que já têm fonte, cada um com a sua',
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
          },
        );
        expect(result.errors, isEmpty);
        expect(result.skipped, 0);
        // O que o dono vê no banco, não só no resultado.
        expect(await repository.listReadings('ETH'), hasLength(8));
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

    test('indicadores sem fonte ainda não viram erro nem leitura', () async {
      final result = await collector.collect(['ETH']);

      expect(result.errors, isEmpty);
      expect(
        byIndicator(
          result,
          'ETH',
        ).keys.toSet().intersection({'tvl_trend', 'mvrv_z_score', 'nvt_ratio'}),
        isEmpty,
      );
    });

    test('cada moeda pega só os indicadores do seu conjunto', () async {
      final result = await collector.collect(['BTC', 'ADA', 'XRP', 'BNB']);

      expect(byIndicator(result, 'BTC').keys.toSet(), _btcIndicators);
      for (final coin in ['ADA', 'XRP', 'BNB']) {
        expect(
          byIndicator(result, coin).keys.toSet(),
          _altIndicators,
          reason: coin,
        );
      }
      expect(result.errors, isEmpty);
    });

    test(
      'a Deribit só é chamada para ETH e BTC; a dominância, só para o ETH',
      () async {
        await collector.collect(['ADA', 'XRP', 'BNB']);

        expect(net.count(_deribit), 0);
        expect(net.count(_coingecko), 0);
      },
    );

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
        expect(net.count(_coingecko), 1);
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
      expect(second.skipped, 8);
      expect(net.requests.length, before);
      expect(await repository.listReadings('ETH'), hasLength(8));
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
        expect(result.readings, hasLength(7));
        expect(net.paths(_binance), isNot(contains('/fapi/v1/fundingRate')));
      },
    );

    test('depois da meia-noite UTC coleta de novo', () async {
      current = DateTime.utc(2026, 10, 29, 23, 59);
      await collector.collect(['ETH']);

      current = DateTime.utc(2026, 10, 30, 0, 1);
      final second = await collector.collect(['ETH']);

      expect(second.readings, hasLength(8));
      expect(second.skipped, 0);
      expect(
        byIndicator(second, 'ETH')['funding_rate_7d']!.readingDate,
        '2026-10-30',
      );
    });

    test('só o que falhou é tentado de novo', () async {
      net.handlers[_deribit] = _unavailable;
      final first = await collector.collect(['ETH']);
      expect(first.readings, hasLength(6));
      expect(first.errors, hasLength(2));

      net.handlers[_deribit] = _deribitOk;
      final callsBefore = net.requests.length;
      final second = await collector.collect(['ETH']);

      expect(byIndicator(second, 'ETH').keys.toSet(), {
        'dvol',
        'put_call_ratio',
      });
      expect(second.skipped, 6);
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

        expect(byIndicator(result, 'ETH'), hasLength(6));
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
          _alternative,
          _coingecko,
        ]) {
          net.handlers[host] = _unavailable;
        }

        await expectLater(
          collector.collect(['ETH']),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              allOf(
                contains('ETH funding_rate_7d'),
                contains('ETH dvol'),
                contains('ETH fear_greed_index'),
                contains('ETH btc_dominance'),
              ),
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
        expect(second.skipped, 6);
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

      expect(byIndicator(result, 'ETH'), hasLength(8));
      expect(net.count(_alternative), 1);
      expect(await repository.listReadings('ETH'), hasLength(8));
    });

    test('lista vazia não faz nada', () async {
      final result = await collector.collect([]);

      expect(result.readings, isEmpty);
      expect(result.errors, isEmpty);
      expect(result.skipped, 0);
      expect(net.requests, isEmpty);
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
