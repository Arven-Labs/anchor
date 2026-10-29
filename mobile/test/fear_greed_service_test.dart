import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/crypto_source_http.dart';
import 'package:anchor_mobile/services/fear_greed_service.dart';

// Resposta real do alternative.me, copiada dos testes do desktop
// (`fear_greed.rs`). Tudo vem como texto; o timestamp é em segundos Unix.
const _latest =
    r'{"name":"Fear and Greed Index","data":[{"value":"34","value_classification":"Fear","timestamp":"1792022400","time_until_update":"3600"}],"metadata":{"error":null}}';

String _entry({
  String value = '34',
  String classification = 'Fear',
  String timestamp = '1792022400',
}) =>
    '{"data":[{"value":"$value","value_classification":"$classification","timestamp":"$timestamp"}]}';

void main() {
  group('parseFearGreedLatest', () {
    test('lê valor, classificação e a data do dado (UTC)', () {
      final reading = parseFearGreedLatest(_latest);

      expect(reading.value, 34);
      expect(reading.classification, 'Fear');
      expect(reading.readingDate, '2026-10-15');
    });

    test('a data vem do timestamp em UTC, não do fuso do aparelho', () {
      // 1792022399 = 2026-10-14T23:59:59Z; 1792022400 = 2026-10-15T00:00:00Z.
      expect(
        parseFearGreedLatest(_entry(timestamp: '1792022399')).readingDate,
        '2026-10-14',
      );
      expect(
        parseFearGreedLatest(_entry(timestamp: '1792022400')).readingDate,
        '2026-10-15',
      );
    });

    test('aceita os extremos 0 e 100', () {
      expect(parseFearGreedLatest(_entry(value: '0')).value, 0);
      expect(parseFearGreedLatest(_entry(value: '100')).value, 100);
    });

    test('usa a primeira entrada quando a fonte manda mais de uma', () {
      final body =
          '{"data":[{"value":"34","value_classification":"Fear","timestamp":"1792022400"},{"value":"60","value_classification":"Greed","timestamp":"1791936000"}]}';

      expect(parseFearGreedLatest(body).value, 34);
    });

    test('corpo malformado ou valor fora de 0 a 100 vira erro da fonte', () {
      for (final body in [
        '{"data":[]}',
        _entry(value: 'x'),
        _entry(value: '34.5'),
        _entry(value: '-1'),
        _entry(value: '101'),
        _entry(timestamp: 'abc'),
        _entry(timestamp: '99999999999999999999'),
        '{"data":[{"value":34,"value_classification":"Fear","timestamp":"1"}]}',
        '{"data":[{"value":"34","timestamp":"1792022400"}]}',
        '{"metadata":{"error":"rate limit"}}',
        '<html>',
      ]) {
        expect(
          () => parseFearGreedLatest(body),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              contains('alternative.me Fear & Greed'),
            ),
          ),
          reason: body,
        );
      }
    });
  });

  group('FearGreedService', () {
    (FearGreedService, List<Uri>) service(http.Response Function(Uri) handler) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return handler(request.url);
      });
      return (FearGreedService(client: client), requests);
    }

    test('pede só a leitura mais recente', () async {
      final (fearGreed, requests) = service((_) => http.Response(_latest, 200));

      final reading = await fearGreed.latest();

      expect(reading.value, 34);
      expect(requests.single.host, 'api.alternative.me');
      expect(requests.single.path, '/fng/');
      expect(requests.single.queryParameters, {'limit': '1'});
    });

    test('falha de HTTP sobe como erro da fonte, com o nome dela', () async {
      final (fearGreed, _) = service((_) => http.Response('', 429));

      await expectLater(
        fearGreed.latest(),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('alternative.me Fear & Greed'),
          ),
        ),
      );
    });
  });
}
