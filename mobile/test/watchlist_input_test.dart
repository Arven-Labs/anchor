import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/finance_error.dart';
import 'package:anchor_mobile/domain/watchlist_input.dart';

// Regras de entrada de watchlist: nome, preço-alvo e anotação (tabela de testes 1 a 6).
Matcher _argumentError(FinanceErrorCode code) =>
    throwsA(isA<FinanceArgumentError>().having((e) => e.code, 'code', code));

void main() {
  group('nome da lista', () {
    test('tira os espaços das pontas', () {
      expect(normalizeWatchlistName('  Dividendos  '), 'Dividendos');
    });

    test('vazio ou só espaços é recusado', () {
      expect(() => normalizeWatchlistName(''), _argumentError(FinanceErrorCode.watchlistNameRequired));
      expect(() => normalizeWatchlistName('   '), _argumentError(FinanceErrorCode.watchlistNameRequired));
    });
  });

  group('preço-alvo', () {
    test('em branco fica sem alvo', () {
      expect(validateTargetPrice(null), isNull);
    });

    test('positivo passa como veio', () {
      expect(validateTargetPrice(30.5), 30.5);
      expect(validateTargetPrice(0.01), 0.01);
    });

    test('zero, negativo, NaN e infinito são recusados', () {
      for (final bad in [0.0, -3.0, double.nan, double.infinity, double.negativeInfinity]) {
        expect(() => validateTargetPrice(bad), _argumentError(FinanceErrorCode.targetPriceMustBePositive), reason: '$bad');
      }
    });
  });

  group('anotação', () {
    test('em branco vira null e o resto perde os espaços das pontas', () {
      expect(normalizeWatchlistNotes(null), isNull);
      expect(normalizeWatchlistNotes('  '), isNull);
      expect(normalizeWatchlistNotes(' olhar balanço '), 'olhar balanço');
    });
  });
}
