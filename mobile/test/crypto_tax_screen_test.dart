import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/crypto_tax.dart';
import 'package:anchor_mobile/screens/crypto_tax_screen.dart';

void main() {
  CryptoMonth month({
    bool exempt = false,
    double tax = 0,
    double carriedIn = 0,
    double due = 0,
    double carriedOut = 0,
  }) => CryptoMonth(
    period: '2026-02',
    salesTotal: 40000,
    exempt: exempt,
    gain: 30000,
    rate: 0.15,
    tax: tax,
    carriedIn: carriedIn,
    due: due,
    carriedOut: carriedOut,
    dueDate: DateTime.utc(2026, 3, 31),
    darfCode: 4600,
    sales: const [],
  );

  test('reais no formato brasileiro', () {
    expect(formatBrl(0), 'R\$ 0,00');
    expect(formatBrl(6), 'R\$ 6,00');
    expect(formatBrl(1234.5), 'R\$ 1.234,50');
    expect(formatBrl(35000), 'R\$ 35.000,00');
    expect(formatBrl(5500000.499), 'R\$ 5.500.000,50');
    expect(formatBrl(-10000), '-R\$ 10.000,00');
    expect(formatBrl(-0.001), 'R\$ 0,00', reason: 'sem sinal em zero');
  });

  test('datas e períodos', () {
    expect(formatDay(DateTime.utc(2026, 3, 5)), '05/03/2026');
    expect(formatPeriod('2026-02'), '02/2026');
  });

  test('a situação do mês diz isento, DARF, acumula ou sem imposto', () {
    expect(
      cryptoMonthStatus(month(exempt: true)),
      'Isento: vendas até R\$ 35.000,00 no mês',
    );
    expect(
      cryptoMonthStatus(month(tax: 4500, due: 4500)),
      'DARF 4600 de R\$ 4.500,00 até 31/03/2026',
    );
    expect(
      cryptoMonthStatus(month(tax: 6, carriedOut: 6)),
      'Imposto de R\$ 6,00 abaixo do mínimo de R\$ 10,00: soma ao mês seguinte',
    );
    expect(
      cryptoMonthStatus(month()),
      'Sem imposto: o mês fechou sem ganho',
    );
  });

  test('os avisos nomeiam a moeda, a data e o período', () {
    expect(
      cryptoWarningText(
        CryptoTaxWarning.costUnknown(ticker: 'BTC', date: DateTime(2026, 2, 10)),
      ),
      contains('BTC em 10/02/2026'),
    );
    expect(
      cryptoWarningText(const CryptoTaxWarning.salesNonBrl(period: '2026-02')),
      contains('02/2026'),
    );
  });
}
