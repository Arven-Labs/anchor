import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/crypto_source_http.dart';
import 'package:anchor_mobile/services/defillama_stablecoins_service.dart';

// Forma real da DefiLlama, copiada dos testes do desktop (`crypto_sources.rs`):
// datas em segundos Unix, como texto. Último ponto = 1_000_000 + 30 dias;
// base = 300; agora = 306 (+2%). O ponto de 310 fica a ~20 dias do fim.
const _stablecoins =
    r'[{"date":"1000000","totalCirculating":{"peggedUSD":300.0},"totalCirculatingUSD":{"peggedUSD":300.0}},{"date":"1864000","totalCirculating":{"peggedUSD":310.0},"totalCirculatingUSD":{"peggedUSD":310.0}},{"date":"3592000","totalCirculating":{"peggedUSD":306.0},"totalCirculatingUSD":{"peggedUSD":306.0}}]';

String _point(String date, Object total) =>
    '{"date":"$date","totalCirculatingUSD":{"peggedUSD":$total}}';

void main() {
  group('parseStablecoinChange30d', () {
    test('usa o ponto de 30 dias antes como base, não o mais próximo', () {
      // Se a base fosse o ponto de 310 (a ~20 dias), daria −1,29%.
      expect(parseStablecoinChange30d(_stablecoins), closeTo(2.0, 1e-9));
    });

    test('um ponto exatamente 30 dias antes serve de base', () {
      final body = '[${_point('1000000', 300)},${_point('3592000', 330)}]';

      expect(parseStablecoinChange30d(body), closeTo(10.0, 1e-9));
    });

    test(
      'a base é o último ponto até 30 dias antes, mesmo com falhas na série',
      () {
        // Faltam dias entre 1.000.000 e 1.500.000; o último ponto ≤ limite vence.
        final body =
            '[${_point('500000', 100)},${_point('900000', 200)},${_point('2000000', 250)},${_point('3592000', 220)}]';

        expect(parseStablecoinChange30d(body), closeTo(10.0, 1e-9));
      },
    );

    test('ponto com data ilegível é ignorado, não derruba a série', () {
      final body =
          '[${_point('x', 999)},${_point('1000000', 300)},${_point('3592000', 306)}]';

      expect(parseStablecoinChange30d(body), closeTo(2.0, 1e-9));
    });

    test('menos de 30 dias de histórico ou série vazia é erro da fonte', () {
      for (final body in [
        '[${_point('1000000', 300)},${_point('1100000', 301)}]',
        '[]',
        '[${_point('x', 1)}]',
      ]) {
        expect(
          () => parseStablecoinChange30d(body),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              contains('DefiLlama stablecoins'),
            ),
          ),
          reason: body,
        );
      }
    });

    test('base zero é recusada (não divide por zero)', () {
      final body = '[${_point('1000000', 0)},${_point('3592000', 306)}]';

      expect(
        () => parseStablecoinChange30d(body),
        throwsA(isA<CryptoSourceException>()),
      );
    });

    test('total negativo é recusado', () {
      final body = '[${_point('1000000', 300)},${_point('3592000', -5)}]';

      expect(
        () => parseStablecoinChange30d(body),
        throwsA(isA<CryptoSourceException>()),
      );
    });

    test('corpo fora do formato vira erro da fonte, não TypeError', () {
      for (final body in [
        'não é json',
        '{"error":"x"}',
        '[1,2]',
        '[{"date":"1000000"}]',
        '[{"date":"1000000","totalCirculatingUSD":{"peggedUSD":"300"}}]',
        '[{"date":1000000,"totalCirculatingUSD":{"peggedUSD":300.0}}]',
      ]) {
        expect(
          () => parseStablecoinChange30d(body),
          throwsA(isA<CryptoSourceException>()),
          reason: body,
        );
      }
    });
  });

  group('DefiLlamaStablecoinsService', () {
    (DefiLlamaStablecoinsService, List<Uri>) service(
      http.Response Function(Uri) handler,
    ) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return handler(request.url);
      });
      return (DefiLlamaStablecoinsService(client: client), requests);
    }

    test('pede a série completa e devolve a variação de 30 dias', () async {
      final (llama, requests) = service(
        (_) => http.Response(_stablecoins, 200),
      );

      final value = await llama.supplyChange30d();

      expect(value, closeTo(2.0, 1e-9));
      expect(requests.single.host, 'stablecoins.llama.fi');
      expect(requests.single.path, '/stablecoincharts/all');
    });

    test('falha de HTTP sobe como erro da fonte, com o nome dela', () async {
      final (llama, _) = service((_) => http.Response('', 503));

      await expectLater(
        llama.supplyChange30d(),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('DefiLlama stablecoins'),
          ),
        ),
      );
    });
  });
}
