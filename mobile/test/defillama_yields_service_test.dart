import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/services/crypto_source_http.dart';
import 'package:anchor_mobile/services/defillama_yields_service.dart';

// Os 8 últimos pontos reais de `yields.llama.fi/chart/<pool stETH>` (2026-10-31).
// A série completa tem 1.643 pontos (254 KB). O dado tem uma armadilha de
// propósito: o Lido reportou um APR de 15,077 % em 2026-10-30 (o próprio
// `eth-api.lido.fi` confirma; a média de 7 dias dele é 4,16 %), então os 2 últimos
// pontos valem 15,06732 e os anteriores ~2,25. O ponto de 2026-10-24 (2,781) fica
// fora da janela de 7: com 8 pontos a mediana daria outro valor.
const _stethTail =
    r'{"status":"success","data":[{"timestamp":"2026-10-24T23:01:32.839Z","tvlUsd":22592177208,"apy":2.781,"apyBase":2.781,"apyReward":null,"il7d":null,"apyBase7d":null,"pricePerShare":null},{"timestamp":"2026-10-25T23:01:13.950Z","tvlUsd":22597360550,"apy":2.255,"apyBase":2.255,"apyReward":null,"il7d":null,"apyBase7d":null,"pricePerShare":null},{"timestamp":"2026-10-26T22:01:34.873Z","tvlUsd":22285687237,"apy":2.24,"apyBase":2.24,"apyReward":null,"il7d":null,"apyBase7d":null,"pricePerShare":null},{"timestamp":"2026-10-27T23:01:29.472Z","tvlUsd":22713877377,"apy":2.27155,"apyBase":2.27155,"apyReward":null,"il7d":null,"apyBase7d":null,"pricePerShare":null},{"timestamp":"2026-10-28T23:01:11.562Z","tvlUsd":22865561468,"apy":2.25205,"apyBase":2.25205,"apyReward":null,"il7d":null,"apyBase7d":null,"pricePerShare":null},{"timestamp":"2026-10-29T23:01:43.312Z","tvlUsd":22007976369,"apy":2.24386,"apyBase":2.24386,"apyReward":null,"il7d":null,"apyBase7d":null,"pricePerShare":null},{"timestamp":"2026-10-30T23:01:39.843Z","tvlUsd":21647398125,"apy":15.06732,"apyBase":15.06732,"apyReward":null,"il7d":null,"apyBase7d":null,"pricePerShare":null},{"timestamp":"2026-10-31T12:01:50.401Z","tvlUsd":21786980193,"apy":15.06732,"apyBase":15.06732,"apyReward":null,"il7d":null,"apyBase7d":null,"pricePerShare":null}]}';

/// Dia do último ponto real.
final _today = DateTime.utc(2026, 10, 31);

/// Série no formato real, um ponto por dia terminando em [last]; cada item de
/// [apys] é o `apy` do ponto (número, `null` ou o que o teste quiser).
String _chart(List<Object?> apys, {String last = '2026-10-30'}) {
  final lastDay = DateTime.parse('${last}T00:00:00Z');
  return jsonEncode({
    'status': 'success',
    'data': [
      for (var i = 0; i < apys.length; i++)
        {
          'timestamp':
              '${lastDay.subtract(Duration(days: apys.length - 1 - i)).toIso8601String().substring(0, 10)}T23:01:00.000Z',
          'tvlUsd': 1000000,
          'apy': apys[i],
          'apyBase': apys[i],
          'apyReward': null,
        },
    ],
  });
}

Matcher _sourceError() => throwsA(
  isA<CryptoSourceException>().having(
    (e) => e.toString(),
    'mensagem',
    startsWith('DefiLlama yields'),
  ),
);

void main() {
  group('parseStethApy', () {
    test('série real: a mediana dos 7 últimos ignora o pico de 15 % do Lido', () {
      // 2,255 / 2,24 / 2,27155 / 2,25205 / 2,24386 / 15,06732 / 15,06732: a
      // mediana é 2,255. O último ponto sozinho daria 15,06732.
      expect(parseStethApy(_stethTail, today: _today), closeTo(2.255, 1e-12));
    });

    test('é a mediana, não a média (um pico não puxa o valor)', () {
      final body = _chart([1, 1, 1, 1, 1, 100, 100]);

      expect(parseStethApy(body, today: _today), 1.0);
    });

    test('a janela é o fim da série: 7 pontos, nem 8 nem os primeiros', () {
      // Os 7 últimos são 2, 2, 2, 10, 10, 10, 10 (mediana 10). Com o 0 e 8
      // pontos a mediana seria 6; com os 7 primeiros (0, 2, 2, 2, 10, 10, 10), 2.
      final body = _chart([0, 2, 2, 2, 10, 10, 10, 10]);

      expect(parseStethApy(body, today: _today), 10.0);
    });

    test('série longa: só o fim conta', () {
      final body = _chart([...List.filled(30, 1), ...List.filled(7, 5)]);

      expect(parseStethApy(body, today: _today), 5.0);
    });

    test('a ordem dos 7 não importa; o que vale é a posição na série', () {
      expect(
        parseStethApy(_chart([7, 1, 6, 2, 5, 3, 4]), today: _today),
        4.0,
      );
    });

    test('menos de 7 pontos é erro', () {
      expect(
        () => parseStethApy(_chart([2, 2, 2, 2, 2, 2]), today: _today),
        _sourceError(),
      );
      expect(
        () => parseStethApy(_chart([]), today: _today),
        _sourceError(),
      );
    });

    test('apy nulo, texto ou negativo em qualquer dos 7 últimos é erro', () {
      // Em 1.643 pontos reais não há nenhum `apy` nulo; se aparecer, a série não
      // é confiável e o indicador falha em vez de pular o ponto.
      for (final bad in <Object?>[null, '2.2', -1]) {
        for (var position = 0; position < 7; position++) {
          final apys = <Object?>[2, 2, 2, 2, 2, 2, 2]..[position] = bad;
          expect(
            () => parseStethApy(_chart(apys), today: _today),
            _sourceError(),
            reason: '$bad na posição $position',
          );
        }
      }
    });

    test('apy nulo antes da janela de 7 não atrapalha', () {
      final body = _chart([null, 2, 2, 2, 2, 2, 2, 2]);

      expect(parseStethApy(body, today: _today), 2.0);
    });

    test('dado velho: último ponto com 7 dias passa, com 8 não', () {
      final apys = [2, 2, 2, 2, 2, 2, 2];

      expect(parseStethApy(_chart(apys, last: '2026-10-24'), today: _today), 2.0);
      expect(
        () => parseStethApy(_chart(apys, last: '2026-10-23'), today: _today),
        _sourceError(),
      );
    });

    test('corpo fora do formato vira erro da fonte, não TypeError', () {
      for (final body in [
        'não é json',
        '{}',
        '[]',
        '{"status":"error"}',
        '{"data":{}}',
        '{"data":[1,2,3,4,5,6,7]}',
        '{"data":[{"apy":2},{"apy":2},{"apy":2},{"apy":2},{"apy":2},{"apy":2},{"apy":2}]}',
      ]) {
        expect(
          () => parseStethApy(body, today: _today),
          _sourceError(),
          reason: body,
        );
      }
    });
  });

  group('realStakingYield', () {
    test('APY menos inflação bruta, com os dois números reais de 2026-10-30', () {
      // Inflação bruta = 2992,561249519 × 365 ÷ 122158026,10744938 × 100.
      expect(realStakingYield(2.255, 0.8941572575130418),
          closeTo(1.360842742486958, 1e-12));
    });

    test('yield real negativo é válido (APY menor que a inflação)', () {
      expect(realStakingYield(0.5, 0.8), closeTo(-0.3, 1e-12));
    });
  });

  group('DefiLlamaYieldsService', () {
    (DefiLlamaYieldsService, List<Uri>) service(
      http.Response Function(Uri) handler, {
      DateTime? now,
    }) {
      final requests = <Uri>[];
      final client = MockClient((request) async {
        requests.add(request.url);
        return handler(request.url);
      });
      return (
        DefiLlamaYieldsService(client: client, now: () => now ?? _today),
        requests,
      );
    }

    test('pede o pool stETH da Lido e devolve a mediana', () async {
      final (yields, requests) = service((_) => http.Response(_stethTail, 200));

      final value = await yields.stethApy();

      expect(value, closeTo(2.255, 1e-12));
      expect(requests.single.host, 'yields.llama.fi');
      expect(
        requests.single.path,
        '/chart/747c1d2a-c668-4682-b9f9-296708a3dd90',
      );
    });

    test('dado velho depois do download vira erro (usa o relógio injetado)', () async {
      final (yields, _) = service(
        (_) => http.Response(_stethTail, 200),
        now: DateTime.utc(2026, 11, 8), // 8 dias depois de 2026-10-31
      );

      await expectLater(yields.stethApy(), _sourceError());
    });

    test('falha de HTTP sobe como erro da fonte, com o nome dela', () async {
      for (final status in [429, 503, 404]) {
        final (yields, _) = service((_) => http.Response('', status));

        await expectLater(yields.stethApy(), _sourceError(), reason: '$status');
      }
    });
  });
}
