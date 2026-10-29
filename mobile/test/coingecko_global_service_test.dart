import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/coingecko_global_service.dart';
import 'package:anchor_mobile/services/crypto_source_http.dart';

// Trecho da resposta real de `/global` de 2026-10-26 (só as chaves usadas),
// copiado dos testes do desktop (`crypto_sources.rs`).
const _global =
    r'{"data":{"active_cryptocurrencies":22501,"market_cap_percentage":{"btc":59.98151875544407,"eth":10.606521696958916,"usdt":6.4},"updated_at":1792973614}}';

String _dominance(Object? btc) =>
    '{"data":{"market_cap_percentage":{"btc":$btc}}}';

void main() {
  group('parseBtcDominance', () {
    test('lê a dominância do BTC em market_cap_percentage', () {
      expect(parseBtcDominance(_global), closeTo(59.98151875544407, 1e-9));
    });

    test('aceita número inteiro e os extremos 0 e 100', () {
      expect(parseBtcDominance(_dominance(60)), 60.0);
      expect(parseBtcDominance(_dominance(0)), 0.0);
      expect(parseBtcDominance(_dominance(100)), 100.0);
    });

    test('ausente, implausível ou fora do formato vira erro da fonte', () {
      for (final body in [
        '{"data":{"market_cap_percentage":{"eth":10.6}}}',
        _dominance(140.0),
        _dominance(-1),
        _dominance('"59.9"'),
        _dominance('null'),
        '{"status":{"error_code":429}}',
        '{"data":{"market_cap_percentage":[]}}',
        '[]',
        'not json',
      ]) {
        expect(
          () => parseBtcDominance(body),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              contains('CoinGecko global'),
            ),
          ),
          reason: body,
        );
      }
    });
  });

  group('CoinGeckoGlobalService', () {
    (CoinGeckoGlobalService, List<Uri>) service(
      http.Response Function(Uri) handler,
    ) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return handler(request.url);
      });
      return (CoinGeckoGlobalService(client: client), requests);
    }

    test('pede o /global e devolve a dominância de hoje', () async {
      final (coingecko, requests) = service((_) => http.Response(_global, 200));

      final value = await coingecko.btcDominance();

      expect(value, closeTo(59.98151875544407, 1e-9));
      expect(requests.single.host, 'api.coingecko.com');
      expect(requests.single.path, '/api/v3/global');
    });

    test('limite de requisições (429) sobe como erro da fonte', () async {
      final (coingecko, _) = service((_) => http.Response('', 429));

      await expectLater(
        coingecko.btcDominance(),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('CoinGecko global'),
          ),
        ),
      );
    });
  });
}
