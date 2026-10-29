import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/crypto_source_http.dart';
import 'package:anchor_mobile/services/ultrasound_service.dart';

// `/api/v2/fees/supply-over-time` real (2026-10-29), reduzido à janela `d30`
// (722 pontos, de hora em hora) com os 2 primeiros e os 2 últimos: o parser só
// olha o primeiro e o último. A resposta completa tem 431 KB, porque traz
// também h1, d1, d7, since_burn e since_merge.
const _supply =
    r'{"d30":[{"supply":122084779.16370456,"timestamp":"2026-09-29T14:00:00Z"},{"supply":122084881.54429322,"timestamp":"2026-09-29T15:00:00Z"},{"supply":122172553.41964117,"timestamp":"2026-10-29T14:00:00Z"},{"supply":122172566.17161618,"timestamp":"2026-10-29T14:05:11Z"}]}';

// `/api/v2/fees/burn-sums` real (2026-10-29), inteira (878 bytes).
const _burn =
    r'{"d1":{"block_number":26298877,"sum":{"eth":21.76984510803734,"usd":50883.952828874615},"timestamp":"2026-10-29T14:04:47Z"},"d30":{"block_number":26298877,"sum":{"eth":1549.1340232797174,"usd":3952178.2595489},"timestamp":"2026-10-29T14:04:47Z"},"d7":{"block_number":26298877,"sum":{"eth":172.5274225210065,"usd":406363.94948195847},"timestamp":"2026-10-29T14:04:47Z"},"h1":{"block_number":26298877,"sum":{"eth":0.9952733670480916,"usd":2310.1977752764565},"timestamp":"2026-10-29T14:04:47Z"},"m5":{"block_number":26298877,"sum":{"eth":0.11956180365613885,"usd":277.9541983278251},"timestamp":"2026-10-29T14:04:47Z"},"since_burn":{"block_number":26298877,"sum":{"eth":4641343.609534672,"usd":13112566365.87659},"timestamp":"2026-10-29T14:04:47Z"},"since_merge":{"block_number":26298877,"sum":{"eth":2018034.267950105,"usd":4562666186.540813},"timestamp":"2026-10-29T14:04:47Z"}}';

/// Oferta na forma real: `{"d30":[{"supply":..}, ...]}`.
String _supplyOf(List<Object?> supplies) =>
    '{"d30":[${[for (final s in supplies) '{"supply":$s}'].join(',')}]}';

String _burnOf(Object? eth) => '{"d30":{"sum":{"eth":$eth,"usd":1}}}';

void main() {
  group('parseNetIssuanceAnnualizedPct', () {
    test('resposta real: variação de 30 dias anualizada', () {
      // (122172566,17 − 122084779,16) ÷ 122084779,16 × 100 × 365 ÷ 30.
      expect(
        parseNetIssuanceAnnualizedPct(_supply),
        closeTo(0.8748635745104221, 1e-9),
      );
    });

    test('100 → 100,1 em 30 dias é +0,1%, vezes 365/30', () {
      final body = _supplyOf([100.0, 100.05, 100.1]);

      expect(
        parseNetIssuanceAnnualizedPct(body),
        closeTo(0.1 * 365 / 30, 1e-9),
      );
    });

    test('oferta caindo dá negativo (queima maior que a emissão)', () {
      expect(
        parseNetIssuanceAnnualizedPct(_supplyOf([100.0, 99.0])),
        lessThan(0),
      );
    });

    test('janela vazia ou de um ponto só é erro da fonte', () {
      for (final body in [
        _supplyOf([]),
        _supplyOf([100.0]),
      ]) {
        expect(
          () => parseNetIssuanceAnnualizedPct(body),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              contains('ultrasound.money supply'),
            ),
          ),
          reason: body,
        );
      }
    });

    test('oferta inicial zero, negativa ou não numérica é recusada', () {
      for (final bad in ['0', '-5', '"100"', 'null']) {
        expect(
          () => parseNetIssuanceAnnualizedPct(_supplyOf([bad, 100.0])),
          throwsA(isA<CryptoSourceException>()),
          reason: bad,
        );
      }
    });

    test('corpo fora do formato vira erro da fonte, não TypeError', () {
      for (final body in [
        '[]',
        '{"d7":[]}',
        '{"d30":{}}',
        '{"d30":[1,2]}',
        '{"d30":[{"x":1},{"x":2}]}',
        'não é json',
      ]) {
        expect(
          () => parseNetIssuanceAnnualizedPct(body),
          throwsA(isA<CryptoSourceException>()),
          reason: body,
        );
      }
    });
  });

  group('parseFeesVsEmissionRatio', () {
    test('resposta real: queima ÷ emissão bruta de 30 dias', () {
      // emissão bruta = (fim − início) + queima = 87.786,99 + 1.549,13.
      expect(
        parseFeesVsEmissionRatio(_supply, _burn),
        closeTo(0.017340507321309505, 1e-12),
      );
    });

    test('oferta +10 e queima 30 reconstroem emissão bruta 40: razão 0,75', () {
      expect(
        parseFeesVsEmissionRatio(_supplyOf([1000.0, 1010.0]), _burnOf(30.0)),
        closeTo(0.75, 1e-12),
      );
    });

    test('emissão bruta zero ou negativa é recusada', () {
      // Oferta 1000 → 1000 e queima 0: bruta 0. Oferta 1000 → 990 e queima 5:
      // bruta −5, impossível (emissão não é negativa), dado incoerente.
      for (final (supply, burn) in [
        (_supplyOf([1000.0, 1000.0]), _burnOf(0.0)),
        (_supplyOf([1000.0, 990.0]), _burnOf(5.0)),
      ]) {
        expect(
          () => parseFeesVsEmissionRatio(supply, burn),
          throwsA(isA<CryptoSourceException>()),
          reason: '$supply $burn',
        );
      }
    });

    test('queima ausente, negativa ou não numérica é recusada', () {
      for (final burn in [
        '{"d30":{}}',
        '{"d30":{"sum":{}}}',
        _burnOf(-1.0),
        _burnOf('"30"'),
        '[]',
        'não é json',
      ]) {
        expect(
          () => parseFeesVsEmissionRatio(_supplyOf([1000.0, 1010.0]), burn),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              contains('ultrasound.money'),
            ),
          ),
          reason: burn,
        );
      }
    });
  });

  group('UltrasoundService', () {
    (UltrasoundService, List<Uri>) service({
      http.Response Function(Uri)? onSupply,
      http.Response Function(Uri)? onBurn,
    }) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return switch (request.url.path) {
          '/api/v2/fees/supply-over-time' =>
            (onSupply ?? (_) => http.Response(_supply, 200))(request.url),
          '/api/v2/fees/burn-sums' =>
            (onBurn ?? (_) => http.Response(_burn, 200))(request.url),
          _ => http.Response('not found', 404),
        };
      });
      return (UltrasoundService(client: client), requests);
    }

    // O serviço só busca; quem junta oferta e queima é o coletor, que baixa a
    // oferta (431 KB) uma vez para os dois indicadores.
    test('fetchSupply pede só a oferta e devolve o corpo como veio', () async {
      final (ultrasound, requests) = service();

      final body = await ultrasound.fetchSupply();

      expect(body, _supply);
      expect(requests.single.host, 'ultrasound.money');
      expect(requests.single.path, '/api/v2/fees/supply-over-time');
    });

    test('fetchBurn pede só a queima e devolve o corpo como veio', () async {
      final (ultrasound, requests) = service();

      final body = await ultrasound.fetchBurn();

      expect(body, _burn);
      expect(requests.single.host, 'ultrasound.money');
      expect(requests.single.path, '/api/v2/fees/burn-sums');
    });

    test('o corpo buscado alimenta os dois parsers', () async {
      final (ultrasound, _) = service();

      final supply = await ultrasound.fetchSupply();
      final burn = await ultrasound.fetchBurn();

      expect(
        parseNetIssuanceAnnualizedPct(supply),
        closeTo(0.8748635745104221, 1e-9),
      );
      expect(
        parseFeesVsEmissionRatio(supply, burn),
        closeTo(0.017340507321309505, 1e-12),
      );
    });

    test('cada pedido que falha nomeia a própria fonte', () async {
      final (supplyDown, _) = service(onSupply: (_) => http.Response('', 503));
      await expectLater(
        supplyDown.fetchSupply(),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('ultrasound.money supply'),
          ),
        ),
      );

      final (burnDown, _) = service(onBurn: (_) => http.Response('', 429));
      await expectLater(
        burnDown.fetchBurn(),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('ultrasound.money burn'),
          ),
        ),
      );
    });
  });
}
