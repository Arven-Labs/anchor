import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/number_locale.dart';

// `formatFixed` troca só o separador decimal do `toStringAsFixed` (ADR 0001). Não agrupa
// milhar: é o que o Portfolio já fazia, e o agrupamento é do `formatMoney`.
void main() {
  group('formatFixed', () {
    test('pt: vírgula decimal', () {
      expect(formatFixed(3.14159, 2, locale: 'pt'), '3,14');
      expect(formatFixed(2.5, 2, locale: 'pt'), '2,50');
      expect(formatFixed(23.3333, 1, locale: 'pt'), '23,3');
    });

    test('en: ponto decimal', () {
      expect(formatFixed(3.14159, 2, locale: 'en'), '3.14');
      expect(formatFixed(2.5, 2, locale: 'en'), '2.50');
      expect(formatFixed(23.3333, 1, locale: 'en'), '23.3');
    });

    test('sem casas decimais não tem separador nenhum', () {
      for (final locale in ['pt', 'en']) {
        expect(formatFixed(10, 0, locale: locale), '10', reason: locale);
        expect(formatFixed(10.4, 0, locale: locale), '10', reason: locale);
      }
    });

    test('não agrupa milhar, em nenhum idioma', () {
      expect(formatFixed(1234567.891, 2, locale: 'pt'), '1234567,89');
      expect(formatFixed(1234567.891, 2, locale: 'en'), '1234567.89');
    });

    test('mantém o sinal e o arredondamento do toStringAsFixed', () {
      for (final value in [-1.005, -0.5, 0.125, 70000.5, 1e-7]) {
        for (final decimals in [0, 1, 2, 3]) {
          expect(
            formatFixed(value, decimals, locale: 'en'),
            value.toStringAsFixed(decimals),
            reason: '$value / $decimals',
          );
          expect(
            formatFixed(value, decimals, locale: 'pt'),
            value.toStringAsFixed(decimals).replaceAll('.', ','),
            reason: '$value / $decimals',
          );
        }
      }
    });

    test('locale desconhecido cai em inglês; pt_BR vale como pt', () {
      expect(formatFixed(2.5, 2, locale: 'fr'), '2.50');
      expect(formatFixed(2.5, 2, locale: 'pt_BR'), '2,50');
    });
  });
}
