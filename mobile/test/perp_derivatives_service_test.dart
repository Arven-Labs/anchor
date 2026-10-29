import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/crypto_source_http.dart';
import 'package:anchor_mobile/services/perp_derivatives_service.dart';

// Respostas reais da Binance (2026-10-16), copiadas dos testes do desktop
// (`crypto_sources.rs`): mais antigo primeiro.
const _binanceFunding =
    r'[{"symbol":"ETHUSDT","fundingTime":1792080000000,"fundingRate":"0.00005355","markPrice":"2439.89","rateType":"Regular"},{"symbol":"ETHUSDT","fundingTime":1792108800001,"fundingRate":"0.00007466","markPrice":"2441.05","rateType":"Regular"},{"symbol":"ETHUSDT","fundingTime":1792137600001,"fundingRate":"0.00004648","markPrice":"2454.27","rateType":"Regular"}]';
const _binanceOpenInterest =
    r'[{"symbol":"ETHUSDT","sumOpenInterest":"2270359.16000000","sumOpenInterestValue":"5668837083.01","CMCCirculatingSupply":"122127761.09","timestamp":1791936000000},{"symbol":"ETHUSDT","sumOpenInterest":"2258306.77400000","sumOpenInterestValue":"5513177087.62","CMCCirculatingSupply":"122133648.45","timestamp":1792108800000}]';
const _binanceLongShort =
    r'[{"symbol":"ETHUSDT","longAccount":"0.7267","longShortRatio":"2.6590","shortAccount":"0.2733","timestamp":1791936000000},{"symbol":"ETHUSDT","longAccount":"0.7252","longShortRatio":"2.6390","shortAccount":"0.2748","timestamp":1792108800000}]';

// Bybit: **mais novo primeiro** (o contrário da Binance).
const _bybitFunding =
    r'{"retCode":0,"retMsg":"OK","result":{"category":"linear","list":[{"symbol":"ETHUSDT","fundingRate":"0.00005006","fundingRateTimestamp":"1792137600000"},{"symbol":"ETHUSDT","fundingRate":"0.00009644","fundingRateTimestamp":"1792108800000"}]},"retExtInfo":{},"time":1792157675806}';
const _bybitOpenInterest =
    r'{"retCode":0,"retMsg":"OK","result":{"symbol":"ETHUSDT","category":"linear","list":[{"openInterest":"781262.72000000","singleOpenInterest":"390631.36","timestamp":"1792108800000"},{"openInterest":"774923.49000000","singleOpenInterest":"387461.75","timestamp":"1792022400000"}],"nextPageCursor":"x"},"retExtInfo":{},"time":1792157676206}';
const _bybitRatio =
    r'{"retCode":0,"retMsg":"OK","result":{"list":[{"symbol":"ETHUSDT","buyRatio":"0.724","sellRatio":"0.276","timestamp":"1792108800000"},{"symbol":"ETHUSDT","buyRatio":"0.7228","sellRatio":"0.2772","timestamp":"1792022400000"}],"nextPageCursor":"x"},"retExtInfo":{},"time":1792157676601}';

typedef _Handler = http.Response Function(Uri url);

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

http.Response _blocked(Uri url) => http.Response('blocked', 451);

void main() {
  group('cálculos puros', () {
    test('funding é a média das taxas de 8h anualizada em %', () {
      // 0,0001 por 8h × 3 × 365 × 100 = 10,95% ao ano.
      expect(fundingAnnualizedPct([0.0001, 0.0001]), closeTo(10.95, 1e-9));
      expect(fundingAnnualizedPct([]), isNull);
    });

    test('funding negativo continua negativo', () {
      expect(fundingAnnualizedPct([-0.0001, -0.0003])!, lessThan(0));
    });

    test('variação percentual protege base zero', () {
      expect(pctChange(200, 250), closeTo(25, 1e-9));
      expect(pctChange(200, 150), closeTo(-25, 1e-9));
      expect(pctChange(0, 10), isNull);
    });

    test('símbolo do perpétuo segue a moeda, em maiúsculas', () {
      expect(perpSymbol('ETH'), 'ETHUSDT');
      expect(perpSymbol('btc'), 'BTCUSDT');
    });
  });

  group('parsers da Binance', () {
    test('funding real: três taxas, na ordem recebida', () {
      final rates = parseBinanceFunding(_binanceFunding);
      expect(rates, hasLength(3));
      expect(rates.first, closeTo(0.00005355, 1e-12));
    });

    test('open interest real vem em unidades da moeda', () {
      final oi = parseBinanceOpenInterest(_binanceOpenInterest);
      expect(oi.first, closeTo(2270359.16, 1e-6));
      expect(pctChange(oi.first, oi.last)!, lessThan(0));
    });

    test('long/short real: média de duas leituras', () {
      final ratios = parseBinanceLongShort(_binanceLongShort);
      expect(ratios, [2.659, 2.639]);
    });

    test('número inválido, NaN e infinito nomeiam a fonte', () {
      for (final bad in ['abc', 'NaN', 'Infinity']) {
        expect(
          () => parseBinanceFunding('[{"fundingRate":"$bad"}]'),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              contains('Binance funding'),
            ),
          ),
          reason: bad,
        );
      }
    });

    test(
      'corpo que não é a lista esperada vira erro da fonte, não TypeError',
      () {
        // Erro da Binance chega como objeto `{"code":..,"msg":..}`.
        for (final body in [
          '{"code":-1121,"msg":"Invalid symbol."}',
          'não é json',
          '[{"semCampo":"1"}]',
          '[1,2]',
        ]) {
          expect(
            () => parseBinanceOpenInterest(body),
            throwsA(isA<CryptoSourceException>()),
            reason: body,
          );
        }
      },
    );
  });

  group('parsers da Bybit', () {
    test(
      'séries voltam em ordem cronológica (a Bybit manda a mais nova antes)',
      () {
        final funding = parseBybitFunding(_bybitFunding);
        expect(funding, [0.00009644, 0.00005006]);

        final oi = parseBybitOpenInterest(_bybitOpenInterest);
        expect(oi.first, closeTo(774923.49, 1e-6));
        // Subiu de 774.923 para 781.262 (+0,82%), não caiu.
        expect(pctChange(oi.first, oi.last)!, greaterThan(0.8));
      },
    );

    test('long/short é buyRatio ÷ sellRatio', () {
      final ratios = parseBybitLongShort(_bybitRatio);
      expect(ratios.last, closeTo(0.724 / 0.276, 1e-9));
      // Bate com a razão da Binance no mesmo dia (2,64).
      final average = ratios.reduce((a, b) => a + b) / ratios.length;
      expect(average, closeTo(2.64, 0.05));
    });

    test('sellRatio zero é recusado', () {
      expect(
        () => parseBybitLongShort(
          '{"retCode":0,"retMsg":"OK","result":{"list":[{"buyRatio":"0.5","sellRatio":"0"}]}}',
        ),
        throwsA(isA<CryptoSourceException>()),
      );
    });

    test('retCode diferente de 0 vira erro com fonte e mensagem', () {
      expect(
        () => parseBybitFunding(
          '{"retCode":10001,"retMsg":"params error","result":{}}',
        ),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            allOf(contains('Bybit funding'), contains('params error')),
          ),
        ),
      );
    });

    test(
      'retCode 0 sem lista é série vazia; corpo estranho é erro da fonte',
      () {
        expect(
          parseBybitFunding('{"retCode":0,"retMsg":"OK","result":{}}'),
          isEmpty,
        );
        for (final body in ['não é json', '[]', '{"retCode":"0"}']) {
          expect(
            () => parseBybitFunding(body),
            throwsA(isA<CryptoSourceException>()),
            reason: body,
          );
        }
      },
    );
  });

  group('binanceThenBybit', () {
    Future<double> ok(double v) async => v;
    Future<double> fail(String detail) async =>
        throw CryptoSourceException('x', detail);

    test('usa a Binance e nem chama a Bybit quando ela responde', () async {
      var bybitCalls = 0;
      final result = await binanceThenBybit('m', () => ok(1), () {
        bybitCalls++;
        return ok(2);
      });

      expect(result, (value: 1.0, source: 'binance'));
      expect(bybitCalls, 0);
    });

    test('cai para a Bybit só quando a Binance falha', () async {
      final result = await binanceThenBybit(
        'm',
        () => fail('geo-blocked'),
        () => ok(2),
      );

      expect(result, (value: 2.0, source: 'bybit'));
    });

    test('as duas falhando: o erro traz a métrica e os dois motivos', () async {
      await expectLater(
        binanceThenBybit(
          'funding',
          () => fail('geo-blocked'),
          () => fail('timeout'),
        ),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            allOf(
              contains('funding'),
              contains('geo-blocked'),
              contains('timeout'),
            ),
          ),
        ),
      );
    });

    test(
      'erro que não é de fonte (bug) não é engolido pelo fallback',
      () async {
        await expectLater(
          binanceThenBybit(
            'm',
            () async => throw StateError('bug'),
            () => ok(2),
          ),
          throwsStateError,
        );
      },
    );
  });

  group('PerpDerivativesService', () {
    (PerpDerivativesService, List<Uri>) service({
      _Handler binance = _binanceOk,
      _Handler bybit = _bybitOk,
    }) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return switch (request.url.host) {
          'fapi.binance.com' => binance(request.url),
          'api.bybit.com' => bybit(request.url),
          _ => throw StateError('host inesperado: ${request.url.host}'),
        };
      });
      return (PerpDerivativesService(client: client), requests);
    }

    test(
      'funding de 7 dias: 21 taxas de 8h, anualizado, fonte binance',
      () async {
        final (perp, requests) = service();

        final result = await perp.fundingRate7d('ETH');

        // média (0,00005355 + 0,00007466 + 0,00004648) / 3 × 3 × 365 × 100
        expect(result.value, closeTo(6.376, 0.001));
        expect(result.source, 'binance');
        expect(requests.single.host, 'fapi.binance.com');
        expect(requests.single.path, '/fapi/v1/fundingRate');
        expect(requests.single.queryParameters, {
          'symbol': 'ETHUSDT',
          'limit': '21',
        });
      },
    );

    test(
      'open interest: variação % em 30 dias do primeiro ao último ponto',
      () async {
        final (perp, requests) = service();

        final result = await perp.openInterestChange30d('ETH');

        expect(result.value, closeTo(-0.5309, 0.001));
        expect(result.source, 'binance');
        expect(requests.single.path, '/futures/data/openInterestHist');
        expect(requests.single.queryParameters, {
          'symbol': 'ETHUSDT',
          'period': '1d',
          'limit': '30',
        });
      },
    );

    test('long/short: média de 7 dias', () async {
      final (perp, requests) = service();

      final result = await perp.longShortRatio('ETH');

      expect(result.value, closeTo(2.649, 1e-9));
      expect(result.source, 'binance');
      expect(requests.single.path, '/futures/data/globalLongShortAccountRatio');
      expect(requests.single.queryParameters, {
        'symbol': 'ETHUSDT',
        'period': '1d',
        'limit': '7',
      });
    });

    test('Binance bloqueada (451): cada métrica sai da Bybit', () async {
      final (perp, requests) = service(binance: _blocked);

      final funding = await perp.fundingRate7d('ETH');
      final oi = await perp.openInterestChange30d('ETH');
      final ratio = await perp.longShortRatio('ETH');

      expect(funding.source, 'bybit');
      expect(funding.value, closeTo(8.021, 0.001));
      expect(oi.source, 'bybit');
      expect(oi.value, closeTo(0.818, 0.001));
      expect(ratio.source, 'bybit');
      expect(ratio.value, closeTo(2.6153, 0.001));
      final bybit = requests.where((u) => u.host == 'api.bybit.com').toList();
      expect(bybit.map((u) => u.path), [
        '/v5/market/funding/history',
        '/v5/market/open-interest',
        '/v5/market/account-ratio',
      ]);
      expect(bybit.first.queryParameters, {
        'category': 'linear',
        'symbol': 'ETHUSDT',
        'limit': '21',
      });
      expect(bybit[1].queryParameters, {
        'category': 'linear',
        'symbol': 'ETHUSDT',
        'intervalTime': '1d',
        'limit': '30',
      });
      expect(bybit[2].queryParameters, {
        'category': 'linear',
        'symbol': 'ETHUSDT',
        'period': '1d',
        'limit': '7',
      });
    });

    test('rede fora na Binance também cai para a Bybit', () async {
      final (perp, _) = service(
        binance: (_) => throw const SocketException('sem rede'),
      );

      expect((await perp.fundingRate7d('BTC')).source, 'bybit');
    });

    test(
      'dado inutilizável da Binance (1 ponto de OI) cai para a Bybit',
      () async {
        final (perp, _) = service(
          binance: (url) => http.Response('[{"sumOpenInterest":"100"}]', 200),
        );

        final result = await perp.openInterestChange30d('ETH');

        expect(result.source, 'bybit');
      },
    );

    test('o símbolo segue a moeda nas duas fontes', () async {
      final (perp, requests) = service(binance: _blocked);

      await perp.fundingRate7d('ada');

      expect(requests.map((u) => u.queryParameters['symbol']), [
        'ADAUSDT',
        'ADAUSDT',
      ]);
    });

    test('open interest começando em zero é recusado nas duas fontes', () async {
      const zeroStart = '[{"sumOpenInterest":"0"},{"sumOpenInterest":"10"}]';
      const bybitZero =
          '{"retCode":0,"retMsg":"OK","result":{"list":[{"openInterest":"10"},{"openInterest":"0"}]}}';
      final (perp, _) = service(
        binance: (_) => http.Response(zeroStart, 200),
        bybit: (_) => http.Response(bybitZero, 200),
      );

      await expectLater(
        perp.openInterestChange30d('ETH'),
        throwsA(isA<CryptoSourceException>()),
      );
    });

    test('funding sem nenhuma taxa é recusado', () async {
      final (perp, _) = service(
        binance: (_) => http.Response('[]', 200),
        bybit: (_) => http.Response(
          '{"retCode":0,"retMsg":"OK","result":{"list":[]}}',
          200,
        ),
      );

      await expectLater(
        perp.fundingRate7d('ETH'),
        throwsA(isA<CryptoSourceException>()),
      );
    });

    test(
      'Binance e Bybit fora: o erro nomeia a métrica e os dois motivos',
      () async {
        final (perp, _) = service(binance: _blocked, bybit: _blocked);

        await expectLater(
          perp.longShortRatio('ETH'),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              allOf(
                contains('long/short'),
                contains('Binance'),
                contains('Bybit'),
                contains('HTTP 451'),
              ),
            ),
          ),
        );
      },
    );
  });
}
