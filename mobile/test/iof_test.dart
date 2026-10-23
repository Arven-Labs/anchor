import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/iof.dart';

void main() {
  test('a alíquota cai de 96% a zero no 30º dia', () {
    expect(iofRateForDays(0), 0.96, reason: 'no mesmo dia vale a do 1º');
    expect(iofRateForDays(1), 0.96);
    expect(iofRateForDays(10), 0.66);
    expect(iofRateForDays(20), 0.33);
    expect(iofRateForDays(29), 0.03);
    expect(iofRateForDays(30), 0.0);
    expect(iofRateForDays(400), 0.0);
  });

  test('a tabela nunca sobe', () {
    for (var day = 1; day < 30; day++) {
      expect(
        iofRateForDays(day + 1),
        lessThanOrEqualTo(iofRateForDays(day)),
        reason: 'dia $day',
      );
    }
  });
}
