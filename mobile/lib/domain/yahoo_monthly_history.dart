import 'dart:convert';

import '../services/quote_exceptions.dart';

/// Fechamento mensal de um ativo e as datas dos desdobramentos, vindos de uma chamada só ao
/// gráfico do Yahoo (`interval=1mo&events=split`). O preço já vem ajustado a desdobramentos,
/// por isso a rentabilidade tira da conta o ativo que teve um depois da 1ª transação dele.
class MonthlyPriceHistory {
  /// `'AAAA-MM'` → fechamento do mês (o último ponto é o preço mais recente).
  final Map<String, double> closes;

  /// Dia (UTC) de cada desdobramento.
  final List<DateTime> splits;

  const MonthlyPriceHistory({required this.closes, required this.splits});
}

/// Lê a resposta do gráfico mensal. O mês é o da data do ponto em UTC; fechamento vazio
/// (mês sem pregão) é descartado. Resposta quebrada ou sem nenhum fechamento é "não
/// encontrado" para [symbol].
MonthlyPriceHistory parseYahooMonthlyHistory(String body, String symbol) {
  try {
    final decoded = jsonDecode(body) as Map<String, dynamic>;
    final result = (decoded['chart']['result'] as List).first as Map<String, dynamic>;
    final timestamps = (result['timestamp'] as List).cast<num>();
    final quote = (result['indicators']['quote'] as List).first as Map<String, dynamic>;
    final rawCloses = quote['close'] as List;

    final closes = <String, double>{};
    for (var i = 0; i < timestamps.length && i < rawCloses.length; i++) {
      final close = rawCloses[i];
      if (close is! num) continue;
      final date = DateTime.fromMillisecondsSinceEpoch(timestamps[i].toInt() * 1000, isUtc: true);
      closes['${date.year.toString().padLeft(4, '0')}-${date.month.toString()}'] = close.toDouble();
    }
    if (closes.isEmpty) throw QuoteNotFoundException(symbol);

    final splits = <DateTime>[];
    final rawSplits = (result['events'] as Map<String, dynamic>?)?['splits'] as Map<String, dynamic>?;
    for (final split in rawSplits?.values ?? const <dynamic>[]) {
      final seconds = (split as Map<String, dynamic>)['date'] as num;
      final date = DateTime.fromMillisecondsSinceEpoch(seconds.toInt() * 1000, isUtc: true);
      splits.add(DateTime.utc(date.year, date.month, date.day));
    }
    splits.sort();

    return MonthlyPriceHistory(closes: closes, splits: splits);
  } on QuoteNotFoundException {
    rethrow;
  } catch (_) {
    throw QuoteNotFoundException(symbol);
  }
}
