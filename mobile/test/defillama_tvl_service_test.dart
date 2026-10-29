import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/crypto_source_http.dart';
import 'package:anchor_mobile/services/defillama_tvl_service.dart';

// Os 35 últimos pontos reais de `/v2/historicalChainTvl/Ethereum` (2026-10-29):
// datas diárias contíguas em segundos Unix, `tvl` inteiro em USD. A série
// completa tem 3.320 pontos; o parser só olha o último e o de 30 posições atrás.
const _ethereumTail =
    r'[{"date":1790294400,"tvl":53471180650},{"date":1790380800,"tvl":53426852280},{"date":1790467200,"tvl":53418146416},{"date":1790553600,"tvl":53326470614},{"date":1790640000,"tvl":53278868314},{"date":1790726400,"tvl":53167086389},{"date":1790812800,"tvl":53141935569},{"date":1790899200,"tvl":53245759135},{"date":1790985600,"tvl":53080834290},{"date":1791072000,"tvl":53155975747},{"date":1791158400,"tvl":54130276627},{"date":1791244800,"tvl":54101916977},{"date":1791331200,"tvl":53941126022},{"date":1791417600,"tvl":52306371070},{"date":1791504000,"tvl":50907921069},{"date":1791590400,"tvl":51027334538},{"date":1791676800,"tvl":51270010748},{"date":1791763200,"tvl":51606259317},{"date":1791849600,"tvl":50173594628},{"date":1791936000,"tvl":50803415013},{"date":1792022400,"tvl":49915542151},{"date":1792108800,"tvl":50043916849},{"date":1792195200,"tvl":49468121699},{"date":1792281600,"tvl":49360549472},{"date":1792368000,"tvl":49570141581},{"date":1792454400,"tvl":49342810811},{"date":1792540800,"tvl":50223696117},{"date":1792627200,"tvl":50593630988},{"date":1792713600,"tvl":49399353586},{"date":1792800000,"tvl":49197158979},{"date":1792886400,"tvl":48957318455},{"date":1792972800,"tvl":48471249805},{"date":1793059200,"tvl":48236550996},{"date":1793145600,"tvl":46072615782},{"date":1793232000,"tvl":45840420814}]';

/// Série diária com os valores dados, na forma real: `[{"date":..,"tvl":..}]`.
String _series(List<Object?> values) {
  final points = [
    for (var i = 0; i < values.length; i++)
      '{"date":${1700000000 + i * 86400},"tvl":${values[i]}}',
  ];
  return '[${points.join(',')}]';
}

/// `count` pontos de 100 em 100+(count-1).
List<Object?> _ramp(int count) => [for (var i = 0; i < count; i++) 100 + i];

void main() {
  group('parseTvlTrend', () {
    test('série real do ETH: o último contra o de 30 posições atrás', () {
      // (45840420814 − 53278868314) ÷ 53278868314 × 100, calculado à parte.
      expect(parseTvlTrend(_ethereumTail), closeTo(-13.961346656541899, 1e-9));
    });

    test('compara com o ponto de 30 posições atrás, não o de 29 nem o de 31', () {
      // 31 pontos de 100 a 130 e um último de 200: 32 pontos, o de 30 atrás é o
      // índice 1 (101).
      final body = _series([..._ramp(31), 200]);

      expect(parseTvlTrend(body), closeTo((200 - 101) / 101 * 100, 1e-9));
    });

    test('31 pontos bastam (a base é o primeiro); 30 não', () {
      expect(parseTvlTrend(_series(_ramp(31))), closeTo(30.0, 1e-9));
      expect(
        () => parseTvlTrend(_series(_ramp(30))),
        throwsA(isA<CryptoSourceException>()),
      );
      expect(() => parseTvlTrend('[]'), throwsA(isA<CryptoSourceException>()));
    });

    test('base zero é recusada (não divide por zero)', () {
      // 31 pontos: a base (30 posições atrás) é o primeiro, que vale zero.
      final body = _series([0, ...List.filled(29, 5), 10]);

      expect(
        () => parseTvlTrend(body),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('DefiLlama TVL'),
          ),
        ),
      );
    });

    test('tvl negativo, texto ou nulo é recusado', () {
      for (final bad in ['-5', '"100"', 'null']) {
        final body = _series([..._ramp(30), bad]);
        expect(
          () => parseTvlTrend(body),
          throwsA(isA<CryptoSourceException>()),
          reason: bad,
        );
      }
    });

    test('corpo fora do formato vira erro da fonte, não TypeError', () {
      for (final body in [
        '{"error":"x"}',
        '[1,2]',
        '[{"date":1}]',
        'não é json',
      ]) {
        expect(
          () => parseTvlTrend(body),
          throwsA(
            isA<CryptoSourceException>().having(
              (e) => e.toString(),
              'mensagem',
              contains('DefiLlama TVL'),
            ),
          ),
          reason: body,
        );
      }
    });
  });

  group('DefiLlamaTvlService', () {
    (DefiLlamaTvlService, List<Uri>) service(
      http.Response Function(Uri) handler,
    ) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return handler(request.url);
      });
      return (DefiLlamaTvlService(client: client), requests);
    }

    test('ETH usa a cadeia Ethereum', () async {
      final (llama, requests) = service(
        (_) => http.Response(_ethereumTail, 200),
      );

      final value = await llama.tvlTrend('ETH');

      expect(value, closeTo(-13.961346656541899, 1e-9));
      expect(requests.single.host, 'api.llama.fi');
      expect(requests.single.path, '/v2/historicalChainTvl/Ethereum');
    });

    test('BNB usa a cadeia BSC; minúsculas funcionam', () async {
      final (llama, requests) = service(
        (_) => http.Response(_ethereumTail, 200),
      );

      await llama.tvlTrend('bnb');

      expect(requests.single.path, '/v2/historicalChainTvl/BSC');
    });

    test('moeda sem cadeia própria é erro de uso, sem pedido à rede', () async {
      final (llama, requests) = service(
        (_) => http.Response(_ethereumTail, 200),
      );

      for (final coin in ['BTC', 'ADA', 'XRP', 'SOL']) {
        await expectLater(
          llama.tvlTrend(coin),
          throwsArgumentError,
          reason: coin,
        );
      }

      expect(requests, isEmpty);
    });

    test('falha de HTTP sobe como erro da fonte, com o nome dela', () async {
      final (llama, _) = service((_) => http.Response('', 503));

      await expectLater(
        llama.tvlTrend('ETH'),
        throwsA(
          isA<CryptoSourceException>().having(
            (e) => e.toString(),
            'mensagem',
            contains('DefiLlama TVL'),
          ),
        ),
      );
    });
  });
}
