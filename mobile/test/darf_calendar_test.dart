import 'package:flutter_test/flutter_test.dart';

import 'package:anchor_mobile/domain/darf_calendar.dart';

void main() {
  DateTime d(int year, int month, int day) => DateTime.utc(year, month, day);

  test('Páscoa em anos conhecidos', () {
    expect(easter(2024), d(2024, 3, 31));
    expect(easter(2025), d(2025, 4, 20));
    expect(easter(2026), d(2026, 4, 5));
  });

  test('exemplo da Receita: ganho de fevereiro/2024 vence no fim de março, sem Sexta-feira Santa', () {
    // 31/03/2024 é domingo e 29/03 é Sexta-feira Santa → 28/03.
    expect(darfDueDate(2024, 2), d(2024, 3, 28));
  });

  test('pula fim de semana', () {
    // Fevereiro/2026 termina num sábado (28/02) → sexta 27/02.
    expect(darfDueDate(2026, 1), d(2026, 2, 27));
  });

  test('ganho de novembro vence em 30/12, porque 31/12 não tem expediente bancário', () {
    expect(darfDueDate(2025, 11), d(2025, 12, 30));
  });

  test('ganho de dezembro vence em janeiro do ano seguinte', () {
    // Janeiro/2027 termina num domingo (31/01) → sexta 29/01.
    expect(darfDueDate(2026, 12), d(2027, 1, 29));
  });

  test('feriados fixos, Carnaval e Corpus Christi não são dia útil', () {
    expect(isBusinessDay(d(2026, 4, 21)), isFalse, reason: 'Tiradentes');
    expect(isBusinessDay(d(2026, 11, 20)), isFalse, reason: 'Consciência Negra');
    expect(isBusinessDay(d(2026, 2, 16)), isFalse, reason: 'segunda de Carnaval');
    expect(isBusinessDay(d(2026, 6, 4)), isFalse, reason: 'Corpus Christi');
    expect(isBusinessDay(d(2026, 4, 3)), isFalse, reason: 'Sexta-feira Santa');
    expect(isBusinessDay(d(2026, 3, 2)), isTrue, reason: 'segunda comum');
  });
}
