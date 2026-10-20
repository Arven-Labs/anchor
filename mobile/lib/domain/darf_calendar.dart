// Vencimento do DARF de renda variável: último dia útil do mês seguinte ao da
// apuração. Porta de `desktop/src-tauri/src/domain/tax/calendar.rs`. "Dia
// útil" = dia em que banco abre: fora fins de semana, feriados nacionais e os
// dias sem expediente bancário (Carnaval, Corpus Christi, 31/12). Feriados
// estaduais/municipais ficam de fora — o app não sabe onde o dono mora; na
// dúvida, vence um dia útil "antes" em vez de depois, nunca ao contrário.
//
// Todas as datas são UTC à meia-noite: somar/subtrair dias não sofre com
// horário de verão.

/// Domingo de Páscoa (algoritmo anônimo gregoriano — Meeus/Jones/Butcher).
DateTime easter(int year) {
  final a = year % 19;
  final b = year ~/ 100;
  final c = year % 100;
  final d = b ~/ 4;
  final e = b % 4;
  final f = (b + 8) ~/ 25;
  final g = (b - f + 1) ~/ 3;
  final h = (19 * a + b - d - g + 15) % 30;
  final i = c ~/ 4;
  final k = c % 4;
  final l = (32 + 2 * e + 2 * i - h - k) % 7;
  final m = (a + 11 * h + 22 * l) ~/ 451;
  final month = (h + l - 7 * m + 114) ~/ 31;
  final day = ((h + l - 7 * m + 114) % 31) + 1;
  return DateTime.utc(year, month, day);
}

const _fixedBankHolidays = [
  (1, 1), // Confraternização Universal
  (4, 21), // Tiradentes
  (5, 1), // Dia do Trabalho
  (9, 7), // Independência
  (10, 12), // Nossa Senhora Aparecida
  (11, 2), // Finados
  (11, 15), // Proclamação da República
  (11, 20), // Consciência Negra (Lei 14.759/2023)
  (12, 25), // Natal
  (12, 31), // sem expediente bancário
];

bool _sameDay(DateTime a, DateTime b) =>
    a.year == b.year && a.month == b.month && a.day == b.day;

bool isBankHoliday(DateTime date) {
  if (_fixedBankHolidays.contains((date.month, date.day))) return true;
  final sunday = easter(date.year);
  final movable = [
    sunday.subtract(const Duration(days: 48)), // segunda de Carnaval
    sunday.subtract(const Duration(days: 47)), // terça de Carnaval
    sunday.subtract(const Duration(days: 2)), // Sexta-feira Santa
    sunday.add(const Duration(days: 60)), // Corpus Christi
  ];
  return movable.any((holiday) => _sameDay(holiday, date));
}

bool isBusinessDay(DateTime date) =>
    date.weekday != DateTime.saturday &&
    date.weekday != DateTime.sunday &&
    !isBankHoliday(date);

/// Último dia útil do mês seguinte a (`year`, `month`), em UTC.
DateTime darfDueDate(int year, int month) {
  // Dia 0 do mês seguinte ao seguinte = último dia do mês seguinte.
  var day = DateTime.utc(year, month + 2, 0);
  while (!isBusinessDay(day)) {
    day = day.subtract(const Duration(days: 1));
  }
  return day;
}
