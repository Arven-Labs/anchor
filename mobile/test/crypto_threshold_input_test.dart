import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/crypto_threshold_input.dart';

// Entrada dos limites no celular, sem tela nem banco. A regra de verdade
// (verde diferente de vermelho, finitos) continua no repositório; aqui é só o que
// o diálogo precisa para habilitar o Salvar e dizer a direção.
void main() {
  group('parseBoundary', () {
    test('aceita vírgula ou ponto como separador decimal', () {
      expect(parseBoundary('1,5'), 1.5);
      expect(parseBoundary('1.5'), 1.5);
      expect(parseBoundary('0,25'), 0.25);
    });

    test('aceita sinal, inteiro e espaços nas pontas', () {
      expect(parseBoundary('-0,25'), -0.25);
      expect(parseBoundary('+2'), 2.0);
      expect(parseBoundary(' 3 '), 3.0);
      expect(parseBoundary('75'), 75.0);
      expect(parseBoundary(',5'), 0.5);
    });

    test('recusa vazio, só espaço e texto', () {
      expect(parseBoundary(''), isNull);
      expect(parseBoundary('   '), isNull);
      expect(parseBoundary('abc'), isNull);
      expect(parseBoundary('-'), isNull);
    });

    test('recusa mais de um separador e milhar', () {
      expect(parseBoundary('1,2,3'), isNull);
      expect(parseBoundary('1.000,5'), isNull);
      expect(parseBoundary('1..5'), isNull);
    });

    test('recusa o que o double.tryParse aceitaria mas não é um número digitado', () {
      expect(parseBoundary('NaN'), isNull);
      expect(parseBoundary('Infinity'), isNull);
      expect(parseBoundary('-Infinity'), isNull);
      expect(parseBoundary('1e3'), isNull);
      expect(parseBoundary('0x10'), isNull);
    });

    test('nunca devolve um valor não finito', () {
      // Tantos dígitos que o double estoura para infinito.
      final huge = '9' * 400;
      expect(parseBoundary(huge), isNull);
    });
  });

  group('boundaryDraftError', () {
    test('sem erro com dois números diferentes', () {
      expect(boundaryDraftError('0', '7'), isNull);
      expect(boundaryDraftError('2,5', '-1'), isNull);
    });

    test('pede dois números quando um deles não é número', () {
      expect(boundaryDraftError('', '7'), 'Digite dois números.');
      expect(boundaryDraftError('0', 'x'), 'Digite dois números.');
    });

    test('pede números diferentes quando são iguais', () {
      expect(boundaryDraftError('5', '5'), 'Use dois números diferentes.');
      expect(boundaryDraftError('5,0', '5'), 'Use dois números diferentes.');
    });
  });

  group('draftHigherIsBetter', () {
    test('verde acima do vermelho: maior é melhor', () {
      expect(draftHigherIsBetter('2', '0'), isTrue);
    });

    test('verde abaixo do vermelho: menor é melhor', () {
      expect(draftHigherIsBetter('0', '7'), isFalse);
    });

    test('sem direção quando o rascunho não vale', () {
      expect(draftHigherIsBetter('', '7'), isNull);
      expect(draftHigherIsBetter('5', '5'), isNull);
    });
  });

  group('describeBands', () {
    test('maior é melhor: verde a partir do limite, vermelho até o outro', () {
      expect(describeBands(2.0, 0.0), 'Verde ≥ 2 · Vermelho ≤ 0');
    });

    test('menor é melhor: verde até o limite, vermelho a partir do outro', () {
      expect(describeBands(0.0, 7.0), 'Verde ≤ 0 · Vermelho ≥ 7');
    });

    test('usa vírgula decimal', () {
      expect(describeBands(0.5, 4.0), 'Verde ≤ 0,5 · Vermelho ≥ 4');
    });
  });

  group('formatBoundary', () {
    test('inteiro sem casas', () {
      expect(formatBoundary(7.0), '7');
      expect(formatBoundary(-1.0), '-1');
      expect(formatBoundary(75.0), '75');
    });

    test('vírgula decimal e sem zeros à direita', () {
      expect(formatBoundary(0.5), '0,5');
      expect(formatBoundary(1.25), '1,25');
      expect(formatBoundary(3.10), '3,1');
    });

    test('nunca mostra -0', () {
      expect(formatBoundary(-0.0), '0');
      expect(formatBoundary(0.0), '0');
    });

    test('valor pequeno não vira zero', () {
      expect(formatBoundary(0.0005), '0,0005');
    });

    test('número grande sem notação científica', () {
      expect(formatBoundary(1e15), '1000000000000000');
      expect(formatBoundary(1e15).contains('e'), isFalse);
    });

    test('ida e volta: o que é mostrado volta como o mesmo número', () {
      for (final value in [0.0, 7.0, -1.0, 0.5, 1.25, 55.0, 0.0005, 12345.678]) {
        expect(parseBoundary(formatBoundary(value)), value, reason: '$value');
      }
    });
  });
}
