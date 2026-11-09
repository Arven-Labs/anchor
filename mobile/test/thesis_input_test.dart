import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/finance_error.dart';
import 'package:anchor_mobile/domain/thesis_input.dart';

// Regras de entrada das teses (tabela de testes TS1 a TS3).
void main() {
  group('título', () {
    test('TS1: tira os espaços das pontas', () {
      expect(normalizeThesisTitle('  Juros 2027  '), 'Juros 2027');
    });

    test('TS2: vazio ou só espaços é recusado', () {
      for (final bad in ['', '   ', '\n\t']) {
        expect(
          () => normalizeThesisTitle(bad),
          throwsA(isA<FinanceArgumentError>().having((e) => e.code, 'code', FinanceErrorCode.thesisTitleRequired)),
          reason: bad.replaceAll('\n', r'\n').replaceAll('\t', r'\t'),
        );
      }
    });
  });

  group('texto', () {
    test('TS3: ausente vira vazio e o resto sai idêntico, com quebras de linha e espaços', () {
      expect(normalizeThesisContent(null), '');
      expect(normalizeThesisContent(''), '');
      const markdown = '# Tese\n\n- ponto 1  \n- ponto 2\n';
      expect(normalizeThesisContent(markdown), markdown);
    });
  });
}
