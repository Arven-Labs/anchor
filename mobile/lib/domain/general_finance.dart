import '../models/general_transaction.dart';
import 'finance_error.dart';
import 'number_locale.dart';

/// `YYYY-MM-DD` → `dd/MM/yyyy` em `pt` e `MM/dd/yyyy` nos outros idiomas (ADR 0001);
/// texto que não tem esse formato volta como veio.
String formatReadingDate(String iso, {required String locale}) {
  if (iso.length != 10) return iso;
  final year = iso.substring(0, 4);
  final month = iso.substring(5, 7);
  final day = iso.substring(8, 10);
  return isPortuguese(locale) ? '$day/$month/$year' : '$month/$day/$year';
}

/// `DateTime` → `YYYY-MM-DD`, como o lançamento grava a data.
String toIsoDay(DateTime date) =>
    '${date.year.toString().padLeft(4, '0')}-'
    '${date.month.toString().padLeft(2, '0')}-'
    '${date.day.toString().padLeft(2, '0')}';

/// Valor digitado (aceita vírgula) → número positivo e finito, ou `null`.
double? parseAmount(String text) {
  final value = double.tryParse(text.trim().replaceAll(',', '.'));
  return value != null && value.isFinite && value > 0 ? value : null;
}

/// Porta de `signed_valor` (`desktop/src-tauri/src/commands/bank_account.rs`):
/// despesa, parcela de dívida, empréstimo concedido e saída de transferência
/// saem da conta; receita, recebimento de empréstimo e entrada de
/// transferência entram. `valor` é sempre a grandeza positiva.
double signedValor(GeneralTransactionType type, double valor) =>
    type == GeneralTransactionType.despesa ||
        type == GeneralTransactionType.parcelaDivida ||
        type == GeneralTransactionType.emprestimoConcedido ||
        type == GeneralTransactionType.transferenciaSaida
    ? -valor
    : valor;

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

/// Resultado da conciliação de saldo: o que o banco mostra, o que os lançamentos
/// dão na mesma data e a diferença (`bankBalance − ledgerBalance`), tudo em
/// centavos.
class ReconciliationPreview {
  final String date;
  final double bankBalance;
  final double ledgerBalance;
  final double difference;

  const ReconciliationPreview({
    required this.date,
    required this.bankBalance,
    required this.ledgerBalance,
    required this.difference,
  });
}

double _round2(double value) => (value * 100).roundToDouble() / 100;

/// Porta de `preview_in` (`desktop/src-tauri/src/commands/balance_reconciliation.rs`):
/// compara o saldo que o banco mostra em [date] com o saldo da conta
/// [accountId] naquele dia (lançamentos de data posterior ficam de fora).
/// [today] (`YYYY-MM-DD`) é o limite: data futura é recusada.
ReconciliationPreview reconciliationPreview({
  required Iterable<GeneralTransaction> transactions,
  required int accountId,
  required double bankBalance,
  required String date,
  required String today,
}) {
  if (!bankBalance.isFinite || bankBalance < 0) {
    throw FinanceArgumentError(FinanceErrorCode.balanceMustBeNonNegative);
  }
  // `DateTime.tryParse` aceita mês 13 e dia 30 de fevereiro (normaliza para a data
  // seguinte); só vale a data que volta igual ao ser formatada.
  final parsed = date.length == 10 ? DateTime.tryParse(date) : null;
  if (parsed == null || toIsoDay(parsed) != date) {
    throw FinanceArgumentError(FinanceErrorCode.invalidDate, {'date': date});
  }
  if (date.compareTo(today) > 0) {
    throw FinanceArgumentError(FinanceErrorCode.dateInFuture, {'date': date});
  }
  final ledger = _round2(
    computeBalances(transactions.where((tx) => tx.bankAccountId == accountId), date)[accountId] ?? 0,
  );
  final bank = _round2(bankBalance);
  return ReconciliationPreview(
    date: date,
    bankBalance: bank,
    ledgerBalance: ledger,
    difference: _round2(bank - ledger),
  );
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

/// Gasto de uma categoria no mês `monthPrefix` (`YYYY-MM`), na moeda `base`:
/// só `despesa` da categoria, cada uma convertida da moeda da conta. Despesa
/// sem câmbio fica fora da soma e vem contada. Porta de
/// `compute_spent_this_month` do desktop, com uma divergência: lá cada despesa
/// usa a taxa do dia dela; aqui o app só carrega as taxas recentes, então todas
/// usam a taxa atual de `ratesToBrl`.
({double spent, int missingFx}) spentThisMonth(
  Iterable<GeneralTransaction> transactions, {
  required int categoryId,
  required String monthPrefix,
  required Map<int, String> accountCurrency,
  required String base,
  required Map<String, double> ratesToBrl,
}) {
  var spent = 0.0;
  var missing = 0;
  for (final tx in transactions) {
    if (tx.type != GeneralTransactionType.despesa ||
        tx.categoryId != categoryId ||
        !tx.date.startsWith(monthPrefix)) {
      continue;
    }
    final converted = convertToBase(
      tx.valor,
      accountCurrency[tx.bankAccountId] ?? 'BRL',
      base,
      ratesToBrl,
    );
    if (converted == null) {
      missing++;
    } else {
      spent += converted;
    }
  }
  return (spent: spent, missingFx: missing);
}

/// Um valor na moeda dele; `value == null` é "sem preço" (posição sem
/// cotação) e conta como item de fora, como a falta de câmbio.
class ValuedItem {
  final String currency;
  final double? value;

  const ValuedItem(this.currency, this.value);
}

/// Patrimônio na moeda `base`: caixa das contas mais valor de mercado da
/// carteira, menos o saldo devedor das dívidas. Item sem preço ou sem câmbio
/// fica fora da soma e entra em `itemsMissingFx` — espelha
/// `NetWorthSummary.items_missing_fx` do desktop.
class NetWorth {
  final String base;
  final double cash;
  final double portfolio;

  /// Saldo devedor das dívidas, positivo; já subtraído em `total`.
  final double debt;

  /// Saldo a receber dos empréstimos concedidos marcados para entrar no
  /// patrimônio; já somado em `total`.
  final double loansReceivable;
  final int itemsMissingFx;

  const NetWorth({
    required this.base,
    required this.cash,
    required this.portfolio,
    this.debt = 0,
    this.loansReceivable = 0,
    required this.itemsMissingFx,
  });

  double get total => cash + portfolio + loansReceivable - debt;
}

NetWorth computeNetWorth({
  required String base,
  required Iterable<ValuedItem> cashItems,
  required Iterable<ValuedItem> portfolioItems,
  Iterable<ValuedItem> debtItems = const [],
  Iterable<ValuedItem> loanItems = const [],
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
  final debt = sum(debtItems);
  final loansReceivable = sum(loanItems);
  return NetWorth(
    base: base,
    cash: cash,
    portfolio: portfolio,
    debt: debt,
    loansReceivable: loansReceivable,
    itemsMissingFx: missing,
  );
}
