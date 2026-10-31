import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/bitcoin_data_service.dart';
import 'package:anchor_mobile/services/crypto_source_http.dart';

// Respostas reais de `/v1/sopr/last` e `/v1/nupl/last` de 2026-10-31. Hoje o dado
// vem com 1 dia de atraso e sem `delayed`/`message`; a fonte documenta 7 dias de
// atraso no plano grátis (os testes do desktop trazem uma resposta assim, abaixo).
const _soprLast = r'{"d":"2026-10-30","unixTs":1793318400,"sopr":0.9933}';
const _nuplLast = r'{"d":"2026-10-30","unixTs":1793318400,"nupl":0.3017}';

// Resposta no formato antigo, com o aviso de dado atrasado (2026-10-26, dado de 19/10).
const _soprDelayed =
    r'{"d":"2026-10-19","unixTs":1792368000,"sopr":1.0057,"delayed":true,"message":"Real-time data (last 7 days) requires an active subscription. https://bitcoin-data.com/bguser/pricing"}';

final _today = DateTime.utc(2026, 10, 31);

Matcher _sourceError() => throwsA(
  isA<CryptoSourceException>().having(
    (e) => e.toString(),
    'mensagem',
    startsWith('bitcoin-data.com'),
  ),
);

/// Corpo mínimo de um campo, com a data do dado.
String _body(String field, Object? value, {String date = '2026-10-30'}) =>
    '{"d":"$date","unixTs":1793318400,"$field":$value}';

void main() {
  group('parseBitcoinData', () {
    test('SOPR real: o valor e a data do dado', () {
      final reading = parseBitcoinData(_soprLast, 'sopr', today: _today);

      expect(reading.value, closeTo(0.9933, 1e-12));
      expect(reading.date, '2026-10-30');
    });

    test('NUPL real: o valor e a data do dado', () {
      final reading = parseBitcoinData(_nuplLast, 'nupl', today: _today);

      expect(reading.value, closeTo(0.3017, 1e-12));
      expect(reading.date, '2026-10-30');
    });

    test('formato antigo, com delayed e message: a data é a do dado (7 dias antes)', () {
      final reading = parseBitcoinData(
        _soprDelayed,
        'sopr',
        today: DateTime.utc(2026, 10, 26),
      );

      expect(reading.value, closeTo(1.0057, 1e-12));
      expect(reading.date, '2026-10-19');
    });

    test('o corpo de um indicador não vale para o outro', () {
      expect(
        () => parseBitcoinData(_soprLast, 'nupl', today: _today),
        _sourceError(),
      );
      expect(
        () => parseBitcoinData(_nuplLast, 'sopr', today: _today),
        _sourceError(),
      );
    });

    test('campo que a fonte não atende aqui é erro de uso, antes de ler o corpo', () {
      expect(
        () => parseBitcoinData(_soprLast, 'mvrv', today: _today),
        throwsArgumentError,
      );
    });

    test('SOPR é uma razão positiva: 0 e negativo são recusados, 10 ainda passa', () {
      for (final bad in ['0', '0.0', '-1.2', '10.01']) {
        expect(
          () => parseBitcoinData(_body('sopr', bad), 'sopr', today: _today),
          _sourceError(),
          reason: bad,
        );
      }
      expect(
        parseBitcoinData(_body('sopr', '10'), 'sopr', today: _today).value,
        10.0,
      );
      expect(
        parseBitcoinData(_body('sopr', '0.0001'), 'sopr', today: _today).value,
        closeTo(0.0001, 1e-12),
      );
    });

    test('NUPL fica em [-1, 1]: os extremos passam, fora deles não', () {
      for (final bad in ['1.01', '-1.01', '7']) {
        expect(
          () => parseBitcoinData(_body('nupl', bad), 'nupl', today: _today),
          _sourceError(),
          reason: bad,
        );
      }
      for (final ok in ['1', '-1', '0']) {
        expect(
          parseBitcoinData(_body('nupl', ok), 'nupl', today: _today).value,
          double.parse(ok),
          reason: ok,
        );
      }
    });

    test('valor ausente, nulo, texto ou booleano é recusado', () {
      for (final value in ['null', '"0.99"', 'true', '[]']) {
        expect(
          () => parseBitcoinData(_body('sopr', value), 'sopr', today: _today),
          _sourceError(),
          reason: value,
        );
      }
      expect(
        () => parseBitcoinData(
          '{"d":"2026-10-30","unixTs":1793318400}',
          'sopr',
          today: _today,
        ),
        _sourceError(),
      );
    });

    test('data do dado ausente, fora do formato ou de um dia que não existe é recusada', () {
      for (final body in [
        '{"sopr":1.0}',
        '{"d":"19/10/2026","sopr":1.0}',
        '{"d":"2026-10-30T00:00:00Z","sopr":1.0}',
        '{"d":"2026-02-30","sopr":1.0}',
        '{"d":20261030,"sopr":1.0}',
        '{"d":null,"sopr":1.0}',
      ]) {
        expect(
          () => parseBitcoinData(body, 'sopr', today: _today),
          _sourceError(),
          reason: body,
        );
      }
    });

    test('dado velho: 10 dias passa, 11 não (a fonte parada não passa por atual)', () {
      expect(
        parseBitcoinData(
          _body('sopr', '1.0', date: '2026-10-21'),
          'sopr',
          today: _today,
        ).value,
        1.0,
      );
      expect(
        () => parseBitcoinData(
          _body('sopr', '1.0', date: '2026-10-20'),
          'sopr',
          today: _today,
        ),
        _sourceError(),
      );
    });

    test('dado de hoje passa; dado do futuro é recusado', () {
      expect(
        parseBitcoinData(
          _body('nupl', '0.3', date: '2026-10-31'),
          'nupl',
          today: _today,
        ).date,
        '2026-10-31',
      );
      expect(
        () => parseBitcoinData(
          _body('nupl', '0.3', date: '2026-11-01'),
          'nupl',
          today: _today,
        ),
        _sourceError(),
      );
    });

    test('corpo fora do formato vira erro da fonte, não TypeError', () {
      for (final body in ['não é json', '{}', '[]', '1', '{"error":"limit"}']) {
        expect(
          () => parseBitcoinData(body, 'sopr', today: _today),
          _sourceError(),
          reason: body,
        );
      }
    });
  });

  group('BitcoinDataService', () {
    (BitcoinDataService, List<Uri>) service(
      http.Response Function(Uri) handler, {
      DateTime? now,
    }) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return handler(request.url);
      });
      return (
        BitcoinDataService(client: client, now: () => now ?? _today),
        requests,
      );
    }

    test('SOPR e NUPL pedem cada um o seu endpoint, uma chamada por indicador', () async {
      final (bitcoinData, requests) = service(
        (url) => http.Response(
          url.path.contains('sopr') ? _soprLast : _nuplLast,
          200,
        ),
      );

      final sopr = await bitcoinData.sopr();
      final nupl = await bitcoinData.nupl();

      expect(sopr.value, closeTo(0.9933, 1e-12));
      expect(nupl.value, closeTo(0.3017, 1e-12));
      expect(requests.map((u) => u.host).toSet(), {'bitcoin-data.com'});
      expect(requests.map((u) => u.path), ['/v1/sopr/last', '/v1/nupl/last']);
    });

    test('dado velho depois do download vira erro (usa o relógio injetado)', () async {
      final (bitcoinData, _) = service(
        (_) => http.Response(_soprLast, 200),
        now: DateTime.utc(2026, 11, 20),
      );

      await expectLater(bitcoinData.sopr(), _sourceError());
    });

    test('429 (cota esgotada) e outras falhas de HTTP sobem como erro da fonte', () async {
      for (final status in [429, 503, 404]) {
        final (bitcoinData, requests) = service((_) => http.Response('', status));

        await expectLater(
          bitcoinData.nupl(),
          _sourceError(),
          reason: '$status',
        );
        // Sem retry: cada falha gasta uma chamada da cota e só.
        expect(requests, hasLength(1), reason: '$status');
      }
    });
  });
}
