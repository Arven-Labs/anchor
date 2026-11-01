import 'crypto_score.dart';
import 'number_locale.dart';

// Regras do gráfico do histórico do score, sem tela. Espelham
// `desktop/src/stock-lookup/CryptoScoreHistoryChart.tsx`.

/// As réguas do gráfico são os limiares do veredito: acima da primeira a tese
/// está intacta, entre as duas é neutro.
const historyIntactLine = verdictIntactMinPct;
const historyNeutralLine = verdictNeutralMinPct;

/// O rótulo de cada período vive em `l10n/crypto_labels.dart` (só "Tudo/All" muda de idioma).
enum HistoryPeriod {
  days30(30),
  days90(90),
  months12(365),
  all(null);

  /// Quantos dias para trás, contados do último ponto; `null` é a série inteira.
  final int? days;

  const HistoryPeriod(this.days);
}

/// Fração de verdes sobre o **total** do conjunto da moeda (a mesma base do
/// veredito, por isso 40 e 70 são as faixas dele). Total zero dá 0, nunca `NaN`.
double greenPercent(ScoreHistoryPoint point) =>
    point.total == 0 ? 0.0 : point.green / point.total * 100.0;

/// Recorte da série já carregada. Conta a partir do **último ponto**, não de
/// hoje, para uma moeda sem coleta recente não ficar vazia. Janela com menos de
/// dois pontos não desenha linha, então mostra a série inteira.
List<ScoreHistoryPoint> filterPeriod(
  List<ScoreHistoryPoint> points,
  HistoryPeriod period,
) {
  final days = period.days;
  if (days == null || points.isEmpty) return points;
  final latest = _day(points.last.date);
  if (latest == null) return points;
  final cut = [
    for (final point in points)
      if (_isWithin(point.date, latest, days)) point,
  ];
  return cut.length < 2 ? points : cut;
}

/// Os botões de período só valem quando a série passa de 30 dias.
bool spansMoreThan30Days(List<ScoreHistoryPoint> points) {
  if (points.length < 2) return false;
  final first = _day(points.first.date);
  final last = _day(points.last.date);
  if (first == null || last == null) return false;
  return last.difference(first).inDays > 30;
}

/// `2026-10-29` vira `29/10/26` em `pt` e `10/29/26` nos outros idiomas. Data que não
/// tem esse formato volta como veio.
String formatHistoryDate(String isoDate, {required String locale}) {
  final parts = isoDate.split('-');
  if (parts.length != 3 || parts[0].length != 4) return isoDate;
  final year = parts[0].substring(2);
  return isPortuguese(locale)
      ? '${parts[2]}/${parts[1]}/$year'
      : '${parts[1]}/${parts[2]}/$year';
}

/// `YYYY-MM-DD` em UTC, ou `null` se ilegível (o ponto então não é cortado).
DateTime? _day(String isoDate) => DateTime.tryParse('${isoDate}T00:00:00Z');

bool _isWithin(String isoDate, DateTime latest, int days) {
  final day = _day(isoDate);
  if (day == null) return true;
  return latest.difference(day).inDays <= days;
}
