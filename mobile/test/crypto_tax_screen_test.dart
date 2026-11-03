import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/tax_format.dart';

// Os formatadores que as telas de IR de fundos e de exterior ainda usam em pt. Os textos do
// IR de cripto estão em `crypto_tax_labels_test.dart`.
void main() {
  test('reais no formato brasileiro', () {
    expect(formatBrl(0, locale: 'pt'), 'R\$ 0,00');
    expect(formatBrl(6, locale: 'pt'), 'R\$ 6,00');
    expect(formatBrl(1234.5, locale: 'pt'), 'R\$ 1.234,50');
    expect(formatBrl(35000, locale: 'pt'), 'R\$ 35.000,00');
    expect(formatBrl(5500000.499, locale: 'pt'), 'R\$ 5.500.000,50');
    expect(formatBrl(-10000, locale: 'pt'), '-R\$ 10.000,00');
    expect(formatBrl(-0.001, locale: 'pt'), 'R\$ 0,00', reason: 'sem sinal em zero');
  });

  test('datas e períodos', () {
    expect(formatDay(DateTime.utc(2026, 3, 5), locale: 'pt'), '05/03/2026');
    expect(formatPeriod('2026-02'), '02/2026');
  });
}
