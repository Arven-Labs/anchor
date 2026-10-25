import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/fx_history.dart';

void main() {
  final history = FxHistory([
    // Fora de ordem de propósito: a série se ordena sozinha.
    (DateTime.utc(2026, 1, 10), 5.5),
    (DateTime.utc(2026, 1, 2), 5.0),
    (DateTime.utc(2026, 1, 6), 5.2),
  ]);

  test('dia exato devolve a taxa do dia', () {
    expect(history.rateOn(DateTime(2026, 1, 6)), 5.2);
  });

  test('dia sem taxa pega o dia mais próximo', () {
    expect(history.rateOn(DateTime(2026, 1, 3)), 5.0, reason: '1 dia do 02, 3 do 06');
    expect(history.rateOn(DateTime(2026, 1, 9)), 5.5, reason: '1 dia do 10, 3 do 06');
  });

  test('empate entre dois dias vale o anterior', () {
    expect(history.rateOn(DateTime(2026, 1, 4)), 5.0, reason: '2 dias do 02 e do 06');
  });

  test('antes e depois da série vale a ponta', () {
    expect(history.rateOn(DateTime(2025, 12, 1)), 5.0);
    expect(history.rateOn(DateTime(2026, 3, 1)), 5.5);
  });

  test('só o dia importa, não a hora nem o fuso', () {
    expect(history.rateOn(DateTime(2026, 1, 6, 23, 59)), 5.2);
    expect(history.rateOn(DateTime.utc(2026, 1, 6, 0, 1)), 5.2);
  });

  test('inverted devolve 1/taxa e descarta taxa zero', () {
    final inverse = FxHistory([
      (DateTime.utc(2026, 1, 2), 0.25),
      (DateTime.utc(2026, 1, 3), 0.0),
      (DateTime.utc(2026, 1, 6), 0.5),
    ]).inverted();

    expect(inverse.rateOn(DateTime.utc(2026, 1, 2)), 4.0);
    expect(inverse.rateOn(DateTime.utc(2026, 1, 6)), 2.0);
    // O dia 03 saiu; o mais próximo é o 02 (1 dia) e não o 06 (3 dias).
    expect(inverse.rateOn(DateTime.utc(2026, 1, 3)), 4.0);
  });

  test('times cruza duas séries no dia mais próximo da segunda', () {
    final pygUsd = FxHistory([
      (DateTime.utc(2026, 1, 2), 0.000125),
      (DateTime.utc(2026, 1, 6), 0.000130),
    ]);
    final usdBrl = FxHistory([
      (DateTime.utc(2026, 1, 2), 5.0),
      (DateTime.utc(2026, 1, 5), 6.0),
    ]);

    final cross = pygUsd.times(usdBrl);

    // 02/01: 0,000125 × 5,00 = 0,000625. 06/01: o dia 05 é o mais próximo,
    // 0,000130 × 6,00 = 0,00078.
    expect(cross.rateOn(DateTime.utc(2026, 1, 2)), closeTo(0.000625, 1e-12));
    expect(cross.rateOn(DateTime.utc(2026, 1, 6)), closeTo(0.00078, 1e-12));
  });

  test('times com a segunda série vazia é vazia', () {
    final one = FxHistory([(DateTime.utc(2026, 1, 2), 5.0)]);

    expect(one.times(FxHistory(const [])).isEmpty, isTrue);
  });

  test('série vazia não tem taxa', () {
    final empty = FxHistory(const []);

    expect(empty.isEmpty, isTrue);
    expect(empty.rateOn(DateTime(2026, 1, 6)), isNull);
  });
}
