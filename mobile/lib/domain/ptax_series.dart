/// Série PTAX do Banco Central de uma moeda (fechamento, em reais por unidade),
/// para o IR de exterior (Lei 14.754/2023, art. 15). Porta de
/// `desktop/src-tauri/src/domain/ptax.rs`: a PTAX só existe em dia útil, então um
/// fato de fim de semana ou feriado usa a última cotação anterior (prática usual,
/// não regra lida na lei), até `ptaxMaxLookbackDays`; mais longe que isso a série
/// está com buraco e vale mais o câmbio de mercado.
library;

/// Maior distância (em dias) entre o fato e a PTAX anterior aceita.
const ptaxMaxLookbackDays = 10;

/// Qual lado da cotação: a conversão geral usa a venda; o imposto pago no
/// exterior, a compra.
enum RateKind { sell, buy }

/// De onde veio a taxa de uma conversão.
enum RateSource { ptax, market }

class PtaxQuote {
  final double buy;
  final double sell;

  const PtaxQuote({required this.buy, required this.sell});
}

class PtaxSeries {
  /// Ordenada por dia (UTC), sem dias repetidos.
  final List<(DateTime, PtaxQuote)> _series;

  PtaxSeries(Iterable<(DateTime, PtaxQuote)> series)
    : _series = (series.map((e) => (_day(e.$1), e.$2)).toList()
        ..sort((a, b) => a.$1.compareTo(b.$1)));

  /// Só o dia importa: tira a hora e o fuso pra comparar datas.
  static DateTime _day(DateTime date) =>
      DateTime.utc(date.year, date.month, date.day);

  bool get isEmpty => _series.isEmpty;

  /// PTAX do dia ou, sem cotação nele, a última anterior dentro do teto;
  /// `null` com a série vazia, antes da primeira cotação ou com buraco.
  double? rate(DateTime date, RateKind kind) {
    final day = _day(date);

    // Primeiro índice com data > `day`; o anterior é a última cotação até `day`.
    var low = 0;
    var high = _series.length;
    while (low < high) {
      final mid = (low + high) ~/ 2;
      if (_series[mid].$1.isAfter(day)) {
        high = mid;
      } else {
        low = mid + 1;
      }
    }
    if (low == 0) return null;

    final (quoteDay, quote) = _series[low - 1];
    if (day.difference(quoteDay).inDays > ptaxMaxLookbackDays) return null;
    final value = kind == RateKind.sell ? quote.sell : quote.buy;
    return value.isFinite && value > 0 ? value : null;
  }
}
