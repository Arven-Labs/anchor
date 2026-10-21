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

  test('série vazia não tem taxa', () {
    final empty = FxHistory(const []);

    expect(empty.isEmpty, isTrue);
    expect(empty.rateOn(DateTime(2026, 1, 6)), isNull);
  });
}
