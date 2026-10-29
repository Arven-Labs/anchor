import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/crypto_source_http.dart';
import 'package:anchor_mobile/services/deribit_service.dart';

// Respostas reais da Deribit, copiadas dos testes do desktop
// (`crypto_sources.rs`). DVOL: `[timestamp, open, high, low, close]`.
const _dvol =
    r'{"jsonrpc":"2.0","result":{"data":[[1791936000000,50.1,52.0,49.0,51.5],[1792022400000,51.5,53.2,50.8,52.4],[1792108800000,52.4,53.0,50.0,51.08]],"continuation":null},"testnet":false}';
const _options =
    r'{"jsonrpc":"2.0","result":[{"instrument_name":"ETH-25JUN27-2900-P","open_interest":508.0,"mark_price":0.26},{"instrument_name":"ETH-25JUN27-3500-C","open_interest":800.0,"mark_price":0.1},{"instrument_name":"ETH-30OCT26-2500-P","open_interest":200.0,"mark_price":0.05},{"instrument_name":"ETH-30OCT26-2500-C","open_interest":300.0,"mark_price":0.2},{"instrument_name":"ETH-PERPETUAL","open_interest":999.0,"mark_price":2440.0}]}';

void main() {
  group('parsers', () {
    test('DVOL é o fechamento do ponto mais recente', () {
      expect(parseDvolLatest(_dvol), closeTo(51.08, 1e-9));
    });

    test(
      'DVOL sem pontos, ponto curto ou corpo estranho viram erro da fonte',
      () {
        for (final body in [
          '{"result":{"data":[]}}',
          '{"result":{"data":[[1,2,3]]}}',
          '{"result":{"data":[[1,2,3,4,"x"]]}}',
          '{"error":{"code":10001}}',
          'não é json',
        ]) {
          expect(
            () => parseDvolLatest(body),
            throwsA(
              isA<CryptoSourceException>().having(
                (e) => e.toString(),
                'mensagem',
                contains('Deribit DVOL'),
              ),
            ),
            reason: body,
          );
        }
      },
    );

    test('open interest de opções separa puts de calls e ignora o resto', () {
      final oi = parseOptionOpenInterest(_options);

      expect(oi, hasLength(4)); // o perpétuo fica de fora
      expect(putCallRatio(oi), closeTo(708.0 / 1100.0, 1e-9));
    });

    test('put/call sem nenhuma call é nulo', () {
      expect(putCallRatio([(true, 10.0)]), isNull);
      expect(putCallRatio([]), isNull);
    });

    test('lista de opções malformada vira erro da fonte', () {
      for (final body in [
        '{"result":[{"instrument_name":"ETH-1-P"}]}',
        '{"result":{"x":1}}',
        'não é json',
      ]) {
        expect(
          () => parseOptionOpenInterest(body),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              contains('Deribit options'),
            ),
          ),
          reason: body,
        );
      }
    });

    test('open interest negativo ou não finito é recusado', () {
      expect(
        () => parseOptionOpenInterest(
          '{"result":[{"instrument_name":"ETH-1-C","open_interest":-5.0}]}',
        ),
        throwsA(isA<CryptoSourceException>()),
      );
    });
  });

  group('DeribitService', () {
    final fixedNow = DateTime.utc(2026, 10, 16, 12);

    (DeribitService, List<Uri>) service(http.Response Function(Uri) handler) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return handler(request.url);
      });
      return (DeribitService(client: client, now: () => fixedNow), requests);
    }

    test(
      'DVOL pede 3 dias de candles diários da moeda em maiúsculas',
      () async {
        final (deribit, requests) = service((_) => http.Response(_dvol, 200));

        final value = await deribit.dvol('eth');

        expect(value, closeTo(51.08, 1e-9));
        final url = requests.single;
        expect(url.host, 'www.deribit.com');
        expect(url.path, '/api/v2/public/get_volatility_index_data');
        final end = fixedNow.millisecondsSinceEpoch;
        expect(url.queryParameters, {
          'currency': 'ETH',
          'start_timestamp': '${end - 3 * 86400000}',
          'end_timestamp': '$end',
          'resolution': '86400',
        });
      },
    );

    test('put/call pede o resumo das opções da moeda', () async {
      final (deribit, requests) = service((_) => http.Response(_options, 200));

      final value = await deribit.putCallRatio('BTC');

      expect(value, closeTo(708.0 / 1100.0, 1e-9));
      final url = requests.single;
      expect(url.path, '/api/v2/public/get_book_summary_by_currency');
      expect(url.queryParameters, {'currency': 'BTC', 'kind': 'option'});
    });

    test('put/call sem call em aberto é erro da fonte', () async {
      final (deribit, _) = service(
        (_) => http.Response(
          '{"result":[{"instrument_name":"ETH-1-P","open_interest":10.0}]}',
          200,
        ),
      );

      await expectLater(
        deribit.putCallRatio('ETH'),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('no call open interest'),
          ),
        ),
      );
    });

    test('falha de HTTP sobe como erro da fonte, com o nome dela', () async {
      final (deribit, _) = service((_) => http.Response('', 503));

      await expectLater(
        deribit.dvol('ETH'),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('Deribit DVOL'),
          ),
        ),
      );
      await expectLater(
        deribit.putCallRatio('ETH'),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('Deribit options'),
          ),
        ),
      );
    });
  });
}
