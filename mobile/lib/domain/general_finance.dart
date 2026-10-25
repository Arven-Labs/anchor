import '../models/general_transaction.dart';

/// Porta de `signed_valor` (`desktop/src-tauri/src/commands/bank_account.rs`):
/// despesa sai da conta, receita entra. `valor` é sempre a grandeza positiva.
double signedValor(GeneralTransactionType type, double valor) =>
    type == GeneralTransactionType.despesa ? -valor : valor;

/// Saldo de cada conta (`id` → saldo na moeda da conta) a partir dos
/// lançamentos, ignorando os de data depois de `today` (`YYYY-MM-DD`, comparado
/// como texto) — porta de `compute_balances` do desktop. O saldo nunca é
/// gravado.
Map<int, double> computeBalances(
  Iterable<GeneralTransaction> transactions,
  String today,
) {
  final balances = <int, double>{};
  for (final tx in transactions) {
    if (tx.date.compareTo(today) > 0) continue;
    balances.update(
      tx.bankAccountId,
      (sum) => sum + signedValor(tx.type, tx.valor),
      ifAbsent: () => signedValor(tx.type, tx.valor),
    );
  }
  return balances;
}

/// `value` de `from` para `base`, dadas as taxas de cada moeda em reais
/// (`ratesToBrl['USD'] == 5.0` quer dizer US$ 1 = R$ 5). O real vale 1 sem
/// precisar estar no mapa. Sem taxa utilizável de um dos lados (ausente, zero,
/// negativa ou não finita) devolve `null`.
double? convertToBase(
  double value,
  String from,
  String base,
  Map<String, double> ratesToBrl,
) {
  if (from == base) return value;
  final fromRate = _rateToBrl(from, ratesToBrl);
  final baseRate = _rateToBrl(base, ratesToBrl);
  if (fromRate == null || baseRate == null) return null;
  return value * fromRate / baseRate;
}

double? _rateToBrl(String currency, Map<String, double> ratesToBrl) {
  if (currency == 'BRL') return 1.0;
  final rate = ratesToBrl[currency];
  return rate != null && rate.isFinite && rate > 0 ? rate : null;
}

/// Um valor na moeda dele; `value == null` é "sem preço" (posição sem
/// cotação) e conta como item de fora, como a falta de câmbio.
class ValuedItem {
  final String currency;
  final double? value;

  const ValuedItem(this.currency, this.value);
}

/// Patrimônio na moeda `base`: caixa das contas mais valor de mercado da
/// carteira. Item sem preço ou sem câmbio fica fora da soma e entra em
/// `itemsMissingFx` — espelha `NetWorthSummary.items_missing_fx` do desktop.
class NetWorth {
  final String base;
  final double cash;
  final double portfolio;
  final int itemsMissingFx;

  const NetWorth({
    required this.base,
    required this.cash,
    required this.portfolio,
    required this.itemsMissingFx,
  });

  double get total => cash + portfolio;
}

NetWorth computeNetWorth({
  required String base,
  required Iterable<ValuedItem> cashItems,
  required Iterable<ValuedItem> portfolioItems,
  required Map<String, double> ratesToBrl,
}) {
  var missing = 0;
  double sum(Iterable<ValuedItem> items) {
    var total = 0.0;
    for (final item in items) {
      final value = item.value;
      final converted = value == null
          ? null
          : convertToBase(value, item.currency, base, ratesToBrl);
      if (converted == null) {
        missing++;
      } else {
        total += converted;
      }
    }
    return total;
  }

  final cash = sum(cashItems);
  final portfolio = sum(portfolioItems);
  return NetWorth(
    base: base,
    cash: cash,
    portfolio: portfolio,
    itemsMissingFx: missing,
  );
}
