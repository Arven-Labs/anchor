import '../domain/currency_catalog.dart';
import '../domain/fx_history.dart';
import '../domain/general_finance.dart';
import '../models/bank_account.dart';
import 'fx_rates_loader.dart';
import 'general_finance_repository.dart';
import 'live_positions.dart';

/// O patrimônio na moeda base e as taxas usadas para chegar nele, para quem
/// precisa converter outros valores com o mesmo câmbio (`ratesToBrl`) e avisar
/// as moedas sem taxa (`fxFailed`).
class NetWorthSummary {
  final NetWorth netWorth;
  final Map<String, double> ratesToBrl;
  final Set<String> fxFailed;

  const NetWorthSummary({
    required this.netWorth,
    required this.ratesToBrl,
    required this.fxFailed,
  });
}

/// Caixa das contas mais valor de mercado da carteira, mais empréstimos
/// concedidos, menos o saldo devedor das dívidas, tudo na moeda `base`.
/// Compartilhado por Finanças e pela Home. Quem chama já leu as contas e os
/// saldos (`balances` por id de conta) e cotou a carteira (`rows`).
Future<NetWorthSummary> loadNetWorth({
  required GeneralFinanceRepository repository,
  required List<BankAccount> accounts,
  required Map<int, double> balances,
  required String base,
  required List<LivePositionRow> rows,
  required Future<FxHistory> Function(String currency) fetchFx,
}) async {
  final cashItems = [
    for (final account in accounts)
      ValuedItem(account.currency, balances[account.id] ?? 0),
  ];
  final portfolioItems = portfolioValuedItems(rows);
  final currencyOf = {for (final a in accounts) a.id!: a.currency};
  final today = toIsoDay(DateTime.now());
  final debtItems = [
    for (final l in await repository.listLiabilities())
      ValuedItem(
        currencyOf[l.liability.bankAccountId] ?? defaultBaseCurrency,
        l.balanceOn(today),
      ),
  ];
  final now = DateTime.now();
  final loanItems = [
    for (final l in await repository.listLoans())
      if (l.loan.includeInNetWorth)
        ValuedItem(l.loan.currency, l.balanceOn(now)),
  ];
  final fx = await loadRatesToBrl(
    {
      base,
      for (final item in [
        ...cashItems,
        ...portfolioItems,
        ...debtItems,
        ...loanItems,
      ])
        item.currency,
    },
    fetch: fetchFx,
  );
  final net = computeNetWorth(
    base: base,
    cashItems: cashItems,
    portfolioItems: portfolioItems,
    debtItems: debtItems,
    loanItems: loanItems,
    ratesToBrl: fx.ratesToBrl,
  );
  return NetWorthSummary(
    netWorth: net,
    ratesToBrl: fx.ratesToBrl,
    fxFailed: fx.failed,
  );
}
