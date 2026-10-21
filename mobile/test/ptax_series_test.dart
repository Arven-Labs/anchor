import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/ptax_series.dart';

void main() {
  PtaxSeries series() => PtaxSeries([
    // fora de ordem de propósito: a série se ordena sozinha
    (DateTime.utc(2025, 1, 3), const PtaxQuote(buy: 6.1557, sell: 6.1563)),
    (DateTime.utc(2025, 1, 2), const PtaxQuote(buy: 6.208, sell: 6.2086)),
  ]);

  test('o dia tem a própria compra e venda', () {
    final s = series();

    expect(s.rate(DateTime(2025, 1, 2), RateKind.sell), 6.2086);
    expect(s.rate(DateTime(2025, 1, 2), RateKind.buy), 6.208);
  });

  test('só o dia importa, não a hora nem o fuso', () {
    expect(series().rate(DateTime(2025, 1, 2, 23, 59), RateKind.sell), 6.2086);
    expect(
      series().rate(DateTime.utc(2025, 1, 2, 1), RateKind.sell),
      6.2086,
    );
  });

  test('fim de semana usa a última cotação antes dele', () {
    // 04 e 05/01/2025 são sábado e domingo: vale a sexta, 03/01
    expect(series().rate(DateTime(2025, 1, 5), RateKind.sell), 6.1563);
  });

  test('antes da primeira cotação não há PTAX', () {
    expect(series().rate(DateTime(2025, 1, 1), RateKind.sell), isNull);
  });

  test('buraco maior que o teto não tem PTAX; no teto ainda vale', () {
    final s = series();

    expect(s.rate(DateTime(2025, 1, 13), RateKind.sell), 6.1563);
    expect(s.rate(DateTime(2025, 1, 14), RateKind.sell), isNull);
  });

  test('série vazia não tem PTAX', () {
    expect(PtaxSeries([]).isEmpty, isTrue);
    expect(PtaxSeries([]).rate(DateTime(2025, 1, 2), RateKind.sell), isNull);
  });
}
