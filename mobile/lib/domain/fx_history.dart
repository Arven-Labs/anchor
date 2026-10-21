/// Série diária de câmbio (quanto vale 1 unidade da moeda de origem em reais).
/// Porta de `FxHistory::rate_on`/`nearest` do desktop
/// (`desktop/src-tauri/src/domain/currency_conversion.rs`): a taxa é a do dia
/// **mais próximo** da data pedida; em empate vale a anterior.
class FxHistory {
  /// Ordenada por dia, sem datas repetidas.
  final List<(DateTime, double)> _series;

  FxHistory(Iterable<(DateTime, double)> series)
    : _series = (series.map((e) => (_day(e.$1), e.$2)).toList()
        ..sort((a, b) => a.$1.compareTo(b.$1)));

  /// Só o dia importa: tira a hora e o fuso pra comparar datas.
  static DateTime _day(DateTime date) =>
      DateTime.utc(date.year, date.month, date.day);

  bool get isEmpty => _series.isEmpty;

  /// Taxa do dia mais próximo de `date`; `null` com a série vazia.
  double? rateOn(DateTime date) {
    if (_series.isEmpty) return null;
    final day = _day(date);

    // Primeiro índice com data >= `day`.
    var low = 0;
    var high = _series.length;
    while (low < high) {
      final mid = (low + high) ~/ 2;
      if (_series[mid].$1.isBefore(day)) {
        low = mid + 1;
      } else {
        high = mid;
      }
    }

    final before = low > 0 ? _series[low - 1] : null;
    final next = low < _series.length ? _series[low] : null;
    if (before == null) return next!.$2;
    if (next == null) return before.$2;
    final toBefore = day.difference(before.$1);
    final toNext = next.$1.difference(day);
    return toNext < toBefore ? next.$2 : before.$2;
  }
}
