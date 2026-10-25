import '../domain/fx_history.dart';

/// Taxas de hoje de cada moeda em reais (`ratesToBrl['USD'] == 5.0`: US$ 1 =
/// R$ 5) e as moedas que não deu para cotar agora.
class FxRatesResult {
  final Map<String, double> ratesToBrl;

  /// Moedas sem taxa: o Yahoo não tem o par nem por rota cruzada, ou a fonte
  /// não respondeu (limite, timeout, rede fora). O patrimônio conta os itens
  /// delas como de fora.
  final Set<String> failed;

  const FxRatesResult(this.ratesToBrl, this.failed);
}

/// Busca a taxa de cada moeda para o real. `fetch` devolve a série da moeda
/// em reais (na prática `YahooQuoteService.fetchFxHistoryWithFallback`, com o
/// fallback de inverso e de pernas via dólar); a taxa de hoje é a do dia mais
/// próximo da série. O real não é buscado.
Future<FxRatesResult> loadRatesToBrl(
  Iterable<String> currencies, {
  required Future<FxHistory> Function(String currency) fetch,
  DateTime? today,
}) async {
  final day = today ?? DateTime.now();
  final codes = {
    for (final c in currencies)
      if (c != 'BRL') c,
  };
  final rates = <String, double>{};
  final failed = <String>{};
  await Future.wait(
    codes.map((code) async {
      try {
        final rate = (await fetch(code)).rateOn(day);
        if (rate != null && rate.isFinite && rate > 0) {
          rates[code] = rate;
        } else {
          failed.add(code);
        }
      } catch (_) {
        failed.add(code);
      }
    }),
  );
  return FxRatesResult(rates, failed);
}
