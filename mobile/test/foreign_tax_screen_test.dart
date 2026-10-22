import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/foreign_tax.dart';
import 'package:anchor_mobile/domain/ptax_series.dart';
import 'package:anchor_mobile/screens/foreign_tax_screen.dart';

void main() {
  ForeignYear year({
    double lossCarriedIn = 0,
    double taxBeforeCredit = 0,
    double creditUsed = 0,
    double taxDue = 0,
    double lossCarriedOut = 0,
  }) => ForeignYear(
    year: 2025,
    capitalGain: 0,
    dividends: 0,
    withheld: creditUsed,
    lossCarriedIn: lossCarriedIn,
    taxableBase: 0,
    rate: 0.15,
    taxBeforeCredit: taxBeforeCredit,
    creditUsed: creditUsed,
    taxDue: taxDue,
    lossCarriedOut: lossCarriedOut,
    sales: const [],
    dividendItems: const [],
  );

  test('situação do ano: imposto estimado', () {
    expect(
      foreignYearStatus(
        year(taxBeforeCredit: 420, creditUsed: 60, taxDue: 360),
      ),
      'Imposto estimado de R\$ 360,00 na declaração',
    );
  });

  test('situação do ano: retido no exterior cobre o imposto', () {
    expect(
      foreignYearStatus(year(taxBeforeCredit: 90, creditUsed: 90)),
      contains('o retido no exterior cobre os R\$ 90,00 devidos'),
    );
  });

  test('situação do ano: prejuízo do ano abate os seguintes', () {
    expect(
      foreignYearStatus(year(lossCarriedOut: 600)),
      contains('prejuízo de R\$ 600,00'),
    );
  });

  test('situação do ano: resultado coube no prejuízo anterior', () {
    expect(
      foreignYearStatus(year(lossCarriedIn: 500, lossCarriedOut: 100)),
      contains('coube no prejuízo de anos anteriores'),
    );
  });

  test('avisos em português', () {
    expect(
      foreignWarningText(
        ForeignTaxWarning.costUnknown(
          ticker: 'AAPL',
          date: DateTime(2025, 6, 15),
        ),
      ),
      contains('AAPL em 15/06/2025'),
    );
    expect(
      foreignWarningText(const ForeignTaxWarning.fxMissing(year: 2027)),
      contains('de 2027 sem câmbio'),
    );
  });

  test('o aviso de câmbio aproximado fala do ano e da PTAX', () {
    final text = foreignWarningText(
      const ForeignTaxWarning.fxApproximate(year: 2025),
    );

    expect(text, contains('2025'));
    expect(text, contains('PTAX do Banco Central'));
    expect(text, contains('aproximados'));
  });

  test('a nota diz que a conversão é pela PTAX, com o mercado de reserva', () {
    expect(foreignIntroNote, contains('PTAX do Banco Central'));
    expect(foreignIntroNote, contains('câmbio de mercado'));
    expect(foreignIntroNote, contains('sem DARF mensal'));
  });

  ForeignHolding holding(int year, double quantity, double cost) =>
      ForeignHolding(
        year: year,
        assetId: 1,
        ticker: 'AAPL',
        name: 'Apple',
        currency: 'USD',
        quantity: quantity,
        costBrl: cost,
        fxApproximate: false,
        incomplete: false,
      );

  test(
    'a descrição do informe traz as duas posições e a base da conversão',
    () {
      final text = foreignHoldingDescription(
        year: 2025,
        ticker: 'AAPL',
        name: 'Apple',
        previous: holding(2024, 10, 5000),
        current: holding(2025, 20.5, 12200),
      );

      expect(text, contains('AAPL — Apple.'));
      expect(
        text,
        contains(
          'Situação em 31/12/2024: 10 un., custo de aquisição R\$ 5.000,00.',
        ),
      );
      expect(
        text,
        contains(
          'Situação em 31/12/2025: 20,5 un., custo de aquisição R\$ 12.200,00.',
        ),
      );
      expect(text, contains('cotação de fechamento (venda) do Banco Central'));
    },
  );

  test('a descrição sem posição em uma das datas diz isso', () {
    final text = foreignHoldingDescription(
      year: 2025,
      ticker: 'AAPL',
      name: 'Apple',
      previous: holding(2024, 10, 5000),
    );

    expect(
      text,
      contains(
        'Situação em 31/12/2025: sem posição, custo de aquisição R\$ 0,00.',
      ),
    );
  });

  test('a quantidade perde os zeros à direita', () {
    expect(formatHoldingQuantity(10), '10');
    expect(formatHoldingQuantity(0.12345678), '0,12345678');
    expect(formatHoldingQuantity(1.5), '1,5');
  });

  test('a nota e a legenda do informe avisam dos limites', () {
    expect(foreignHoldingsNote, contains('não valor de mercado'));
    expect(foreignHoldingsNote, contains('país'));
    expect(foreignHoldingsLegend, contains('aproximado'));
    expect(foreignHoldingsLegend, contains('incompleto'));
  });

  test('a taxa de cada fato diz de onde veio', () {
    expect(foreignRateLabel(6.2086, RateSource.ptax), 'Câmbio 6,2086 (PTAX)');
    expect(foreignRateLabel(6, RateSource.market), 'Câmbio 6,0000 (mercado)');
  });
}
