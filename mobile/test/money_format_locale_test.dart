import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/general_finance.dart';
import 'package:anchor_mobile/domain/money_format.dart';

// `formatMoney` segue o idioma (ADR 0001) e o `locale` é obrigatório: esquecer é erro de
// compilação, não texto em português por padrão.
void main() {
  group('formatMoney com locale', () {
    test('pt: milhar com ponto e decimal com vírgula', () {
      expect(formatMoney(1234.56, 'USD', locale: 'pt'), 'USD 1.234,56');
      expect(formatMoney(-1234567.891, 'BRL', locale: 'pt'), 'BRL -1.234.567,89');
    });

    test('en: milhar com vírgula e decimal com ponto', () {
      expect(formatMoney(1234.56, 'USD', locale: 'en'), 'USD 1,234.56');
      expect(formatMoney(-1234567.891, 'BRL', locale: 'en'), 'BRL -1,234,567.89');
    });

    test('zero negativo sai como zero nos dois idiomas', () {
      for (final value in [-0.0, -0.004]) {
        expect(formatMoney(value, 'USD', locale: 'pt'), 'USD 0,00', reason: '$value');
        expect(formatMoney(value, 'USD', locale: 'en'), 'USD 0.00', reason: '$value');
      }
    });

    test('arredonda para cima na casa certa e ganha o milhar', () {
      expect(formatMoney(999.999, 'EUR', locale: 'pt'), 'EUR 1.000,00');
      expect(formatMoney(999.999, 'EUR', locale: 'en'), 'EUR 1,000.00');
    });

    test('três dígitos não ganham separador', () {
      expect(formatMoney(999.5, 'USD', locale: 'en'), 'USD 999.50');
      expect(formatMoney(100000, 'USD', locale: 'en'), 'USD 100,000.00');
    });

    test('locale desconhecido cai em inglês', () {
      expect(formatMoney(1234.56, 'USD', locale: 'fr'), 'USD 1,234.56');
    });
  });

  group('formatReadingDate', () {
    test('pt: dia/mês/ano; en: mês/dia/ano', () {
      expect(formatReadingDate('2026-10-29', locale: 'pt'), '29/10/2026');
      expect(formatReadingDate('2026-10-29', locale: 'en'), '10/29/2026');
    });

    test('texto fora do formato volta como veio', () {
      for (final locale in ['pt', 'en']) {
        expect(formatReadingDate('hoje', locale: locale), 'hoje');
        expect(formatReadingDate('9/3/2026', locale: locale), '9/3/2026');
        expect(formatReadingDate('', locale: locale), '');
      }
    });
  });
}
