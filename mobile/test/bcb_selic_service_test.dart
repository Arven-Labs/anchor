import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:anchor_mobile/domain/porquinho_yield.dart';
import 'package:anchor_mobile/services/bcb_selic_service.dart';
import 'package:anchor_mobile/services/quote_exceptions.dart';

// Meta Selic diária do BCB (SGS 432). Porta de `desktop/src-tauri/src/finance_core/bcb_sgs.rs`:
// o BCB recusa (HTTP 406) uma janela longa de série diária, por isso a busca vai em janelas de 9 anos.
void main() {
  DateTime d(int y, int m, int day) => DateTime.utc(y, m, day);

  // Formato real da API: data em dd/MM/yyyy e valor como texto.
  const body = '[{"data":"12/09/2025","valor":"14.00"},{"data":"15/09/2025","valor":"14.00"},'
      '{"data":"17/09/2025","valor":"14.25"}]';

  group('parseSelic', () {
    test('lê data dd/MM/yyyy e valor em texto, em ordem de data', () {
      final points = BcbSelicService.parseSelic(body);

      expect(points.map((p) => p.date), [d(2025, 9, 12), d(2025, 9, 15), d(2025, 9, 17)]);
      expect(points.map((p) => p.meta), [14.0, 14.0, 14.25]);
    });

    test('ordena o que vier fora de ordem', () {
      final points = BcbSelicService.parseSelic('[{"data":"17/09/2025","valor":"14.25"},{"data":"12/09/2025","valor":"14.00"}]');

      expect(points.first.date, d(2025, 9, 12));
    });

    test('descarta linha sem data válida ou com valor que não é número positivo', () {
      final points = BcbSelicService.parseSelic(
        '[{"data":"31/02/2025","valor":"14.00"},{"data":"12/09/2025","valor":"abc"},'
        '{"data":"13/09/2025","valor":"0"},{"data":"15/09/2025","valor":"14.00"}]',
      );

      expect(points.map((p) => p.date), [d(2025, 9, 15)]);
    });

    test('corpo que não é uma lista JSON (HTML com status 200) é fonte indisponível', () {
      for (final bad in ['<html>erro</html>', '{"erro":"x"}', '']) {
        expect(() => BcbSelicService.parseSelic(bad), throwsA(isA<QuoteUnavailableException>()));
      }
    });
  });

  group('janelas e URL', () {
    test('o trecho é cortado em janelas de 9 anos que terminam em 31/12', () {
      final windows = BcbSelicService.windows(d(2016, 3, 1), d(2026, 11, 8));

      expect(windows, [(d(2016, 3, 1), d(2024, 12, 31)), (d(2025, 1, 1), d(2026, 11, 8))]);
    });

    test('trecho curto cabe numa janela só', () {
      expect(BcbSelicService.windows(d(2025, 1, 2), d(2025, 6, 1)), [(d(2025, 1, 2), d(2025, 6, 1))]);
    });

    test('a URL pede a série 432 com datas dd/MM/yyyy', () {
      final uri = BcbSelicService.buildUri(d(2025, 1, 2), d(2025, 12, 31));

      expect(
        uri.toString(),
        'https://api.bcb.gov.br/dados/serie/bcdata.sgs.432/dados?formato=json&dataInicial=02/01/2025&dataFinal=31/12/2025',
      );
    });
  });

  group('fetchSince', () {
    test('junta as janelas, sem repetir dia, e devolve em ordem', () async {
      final requested = <String>[];
      final service = BcbSelicService(
        client: MockClient((request) async {
          requested.add(request.url.queryParameters['dataInicial']!);
          return http.Response(
            request.url.queryParameters['dataInicial'] == '01/03/2016'
                ? '[{"data":"02/03/2016","valor":"14.25"},{"data":"31/12/2024","valor":"12.25"}]'
                : '[{"data":"31/12/2024","valor":"12.25"},{"data":"02/01/2025","valor":"12.25"}]',
            200,
          );
        }),
      );

      final points = await service.fetchSince(d(2016, 3, 1), today: d(2026, 11, 8));

      expect(requested, ['01/03/2016', '01/01/2025']);
      expect(points.map((p) => p.date), [d(2016, 3, 2), d(2024, 12, 31), d(2025, 1, 2)]);
    });

    test('HTTP 406, 429 ou 500 é fonte indisponível', () async {
      for (final status in [406, 429, 500]) {
        final service = BcbSelicService(client: MockClient((_) async => http.Response('', status)));

        await expectLater(
          service.fetchSince(d(2025, 1, 2), today: d(2025, 6, 1)),
          throwsA(isA<QuoteUnavailableException>()),
        );
      }
    });

    test('série vazia é fonte indisponível (sem meta, a estimativa não tem como seguir)', () async {
      final service = BcbSelicService(client: MockClient((_) async => http.Response('[]', 200)));

      await expectLater(
        service.fetchSince(d(2025, 1, 2), today: d(2025, 6, 1)),
        throwsA(isA<QuoteUnavailableException>()),
      );
    });

    test('o resultado alimenta o modelo: SelicPoint', () async {
      final service = BcbSelicService(client: MockClient((_) async => http.Response(body, 200)));

      final points = await service.fetchSince(d(2025, 9, 1), today: d(2025, 9, 30));

      expect(points.first, isA<SelicPoint>());
    });
  });
}
