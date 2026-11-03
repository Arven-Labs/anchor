import 'package:flutter/material.dart';

import '../domain/currency_catalog.dart';
import '../domain/fx_history.dart';
import '../domain/general_finance.dart';
import '../domain/money_format.dart';
import '../domain/number_locale.dart';
import '../l10n/app_localizations.dart';
import '../l10n/finance_labels.dart';
import '../models/bank_account.dart';
import '../services/general_finance_repository.dart';
import '../services/live_positions.dart';
import '../services/net_worth_loader.dart';
import '../services/portfolio_repository.dart';
import '../services/quote_dispatcher.dart';
import '../services/yahoo_quote_service.dart';
import 'financas_categories_screen.dart';
import 'financas_liabilities_screen.dart';
import 'financas_loans_screen.dart';
import 'financas_transactions_screen.dart';

/// Aviso de câmbio que não veio: os itens dessas moedas ficam fora da soma.
@visibleForTesting
String fxUnavailableBanner(AppLocalizations l, Set<String> failed) =>
    l.finFxBanner((failed.toList()..sort()).join(', '));

/// Linha do resumo quando algo ficou de fora (conta sem câmbio ou posição sem
/// cotação); `null` quando está tudo dentro.
@visibleForTesting
String? missingItemsNote(AppLocalizations l, int count) =>
    count == 0 ? null : l.finMissingItems(count);

/// Finanças Gerais: contas em qualquer moeda, o saldo de cada uma (derivado
/// dos lançamentos) e o patrimônio na moeda base — caixa das contas mais valor
/// de mercado da carteira. Standalone, como o resto do mobile (sem sync com o
/// desktop). Transferências entre contas, dívidas (`FinancasLiabilitiesScreen`),
/// empréstimos concedidos (`FinancasLoansScreen`) e categorias com limite
/// mensal (`FinancasCategoriesScreen`) saem pelos botões da barra. A empresa
/// não listada entra no patrimônio pela última avaliação manual.
class FinancasScreen extends StatefulWidget {
  /// Série da moeda em reais; o padrão é o Yahoo com o fallback de inverso e
  /// de pernas via dólar. Injetável para teste, sem rede.
  final Future<FxHistory> Function(String currency)? fetchFx;

  /// Posições com preço atual; o padrão lê a carteira e cota na rede.
  final Future<List<LivePositionRow>> Function()? loadPositions;

  const FinancasScreen({super.key, this.fetchFx, this.loadPositions});

  @override
  State<FinancasScreen> createState() => _FinancasScreenState();
}

class _FinancasScreenState extends State<FinancasScreen> {
  final _repository = GeneralFinanceRepository();
  final _portfolio = PortfolioRepository();
  final _dispatcher = QuoteDispatcher();
  final _yahoo = YahooQuoteService();

  bool _loading = true;
  List<BankAccount> _accounts = [];
  Map<int, double> _balances = {};
  String _base = defaultBaseCurrency;

  // O resumo vem depois da lista: cota a carteira e o câmbio, que é rede.
  bool _summaryLoading = true;
  NetWorth? _netWorth;
  Map<String, double> _rates = {};
  Set<String> _fxFailed = {};
  int _summaryRun = 0;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final accounts = await _repository.listAccounts();
    final transactions = await _repository.listTransactions();
    final base = await _repository.getBaseCurrency();
    final balances = computeBalances(transactions, toIsoDay(DateTime.now()));
    if (!mounted) return;
    setState(() {
      _accounts = accounts;
      _balances = balances;
      _base = base;
      _loading = false;
    });
    await _loadSummary();
  }

  Future<void> _loadSummary() async {
    final run = ++_summaryRun;
    setState(() => _summaryLoading = true);

    final rows = await (widget.loadPositions ?? _livePositions)();
    final summary = await loadNetWorth(
      repository: _repository,
      accounts: _accounts,
      balances: _balances,
      base: _base,
      rows: rows,
      fetchFx: widget.fetchFx ?? _fetchFx,
    );

    // Uma troca de base ou um refresh no meio deixa esta rodada velha.
    if (!mounted || run != _summaryRun) return;
    setState(() {
      _netWorth = summary.netWorth;
      _rates = summary.ratesToBrl;
      _fxFailed = summary.fxFailed;
      _summaryLoading = false;
    });
  }

  Future<List<LivePositionRow>> _livePositions() async => loadLivePositions(
    await _portfolio.getPositions(),
    repository: _portfolio,
    dispatcher: _dispatcher,
  );

  Future<FxHistory> _fetchFx(String currency) => _yahoo
      .fetchFxHistoryWithFallback(
        from: DateTime.now().subtract(const Duration(days: 7)),
        currency: currency,
      );

  Future<void> _changeBase(String code) async {
    await _repository.setBaseCurrency(code);
    setState(() => _base = code);
    await _loadSummary();
  }

  /// Cria (`existing == null`) ou edita. A moeda fica travada na conta que já
  /// tem lançamentos: o histórico seria relido em outra moeda.
  Future<void> _editAccount([BankAccount? existing]) async {
    final l = AppLocalizations.of(context);
    final nomeController = TextEditingController(text: existing?.nome);
    final titularController = TextEditingController(text: existing?.titular);
    var currency = existing?.currency ?? defaultBaseCurrency;
    final currencyLocked =
        existing != null && await _repository.accountInUse(existing.id!);
    if (!mounted) return;

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: Text(existing == null ? l.finNewAccount : l.finEditAccount),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                controller: nomeController,
                decoration: InputDecoration(
                  labelText: l.finFieldName,
                  hintText: l.finAccountNameHint,
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: titularController,
                decoration: InputDecoration(labelText: l.finFieldHolder),
              ),
              const SizedBox(height: 12),
              DropdownButtonFormField<String>(
                initialValue: currency,
                decoration: InputDecoration(
                  labelText: l.finFieldCurrency,
                  helperText: currencyLocked ? l.finCurrencyLocked : null,
                ),
                items: [
                  for (final code in currencyCatalog)
                    DropdownMenuItem(
                      value: code,
                      child: Text('$code — ${currencyName(l, code)}'),
                    ),
                ],
                onChanged: currencyLocked
                    ? null
                    : (value) =>
                          setDialogState(() => currency = value ?? currency),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l.actionCancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l.actionSave),
            ),
          ],
        ),
      ),
    );

    final nome = nomeController.text.trim();
    final titular = titularController.text.trim();
    nomeController.dispose();
    titularController.dispose();
    if (saved != true || nome.isEmpty || titular.isEmpty || !mounted) return;

    final messenger = ScaffoldMessenger.of(context);
    final account = BankAccount(
      id: existing?.id,
      nome: nome,
      titular: titular,
      currency: currency,
      createdAt: existing?.createdAt ?? DateTime.now(),
    );
    try {
      if (existing == null) {
        await _repository.insertAccount(account);
      } else {
        await _repository.updateAccount(account);
      }
    } on StateError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(financeErrorText(l, e))));
      return;
    }
    await _load();
  }

  Future<void> _deleteAccount(BankAccount account) async {
    final l = AppLocalizations.of(context);
    final inUse = await _repository.accountInUse(account.id!);
    if (!mounted) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.finDeleteTitle(account.nome)),
        content: Text(inUse ? l.finDeleteWithTx : l.finDeleteNoTx),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l.actionCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l.actionDelete),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    await _repository.deleteAccount(account.id!);
    await _load();
  }

  /// Transferência entre duas contas. Na mesma moeda o valor é um só; em
  /// moedas diferentes o usuário digita o que saiu e o que chegou, como no
  /// extrato, e a taxa efetiva aparece só como informação.
  Future<void> _transfer() async {
    final messenger = ScaffoldMessenger.of(context);
    final l = AppLocalizations.of(context);
    if (_accounts.length < 2) {
      messenger.showSnackBar(
        SnackBar(content: Text(l.finTransferNeedTwo)),
      );
      return;
    }
    final outController = TextEditingController();
    final inController = TextEditingController();
    final notesController = TextEditingController();
    BankAccount from = _accounts.first;
    BankAccount to = _accounts[1];
    var date = DateTime.now();

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) {
          final cross = from.currency != to.currency;
          final out = parseAmount(outController.text);
          final incoming = parseAmount(inController.text);
          return AlertDialog(
            title: Text(l.finTransfer),
            content: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  DropdownButtonFormField<BankAccount>(
                    key: const Key('financas-transfer-from'),
                    initialValue: from,
                    isExpanded: true,
                    decoration: InputDecoration(labelText: l.finFrom),
                    items: [
                      for (final a in _accounts)
                        DropdownMenuItem(value: a, child: Text(a.label)),
                    ],
                    onChanged: (value) =>
                        setDialogState(() => from = value ?? from),
                  ),
                  const SizedBox(height: 12),
                  DropdownButtonFormField<BankAccount>(
                    key: const Key('financas-transfer-to'),
                    initialValue: to,
                    isExpanded: true,
                    decoration: InputDecoration(labelText: l.finTo),
                    items: [
                      for (final a in _accounts)
                        DropdownMenuItem(value: a, child: Text(a.label)),
                    ],
                    onChanged: (value) =>
                        setDialogState(() => to = value ?? to),
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    key: const Key('financas-transfer-out'),
                    controller: outController,
                    keyboardType: const TextInputType.numberWithOptions(
                      decimal: true,
                    ),
                    onChanged: (_) => setDialogState(() {}),
                    decoration: InputDecoration(
                      labelText: cross
                          ? l.finAmountSent(from.currency)
                          : l.finAmount(from.currency),
                    ),
                  ),
                  if (cross) ...[
                    const SizedBox(height: 12),
                    TextField(
                      key: const Key('financas-transfer-in'),
                      controller: inController,
                      keyboardType: const TextInputType.numberWithOptions(
                        decimal: true,
                      ),
                      onChanged: (_) => setDialogState(() {}),
                      decoration: InputDecoration(
                        labelText: l.finAmountReceived(to.currency),
                        helperText: out != null && incoming != null
                            ? l.finEffectiveRate(
                                from.currency,
                                formatFixed(incoming / out, 4, locale: l.localeName),
                                to.currency,
                              )
                            : l.finSpreadIncluded,
                      ),
                    ),
                  ],
                  const SizedBox(height: 12),
                  OutlinedButton.icon(
                    icon: const Icon(Icons.calendar_today_outlined),
                    label: Text(
                      formatReadingDate(toIsoDay(date), locale: l.localeName),
                    ),
                    onPressed: () async {
                      final picked = await showDatePicker(
                        context: context,
                        initialDate: date,
                        firstDate: DateTime(2000),
                        lastDate: DateTime(2100),
                      );
                      if (picked != null) setDialogState(() => date = picked);
                    },
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: notesController,
                    decoration: InputDecoration(labelText: l.finNotes),
                  ),
                ],
              ),
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(context).pop(false),
                child: Text(l.actionCancel),
              ),
              FilledButton(
                onPressed: () => Navigator.of(context).pop(true),
                child: Text(l.finTransfer),
              ),
            ],
          );
        },
      ),
    );

    final valorSaida = parseAmount(outController.text);
    final valorEntrada = from.currency == to.currency
        ? valorSaida
        : parseAmount(inController.text);
    final notes = notesController.text.trim();
    // Sem `dispose()`: o diálogo ainda está na animação de saída e o campo
    // reconstruiria com o controlador morto (assert em debug). Sem ouvintes,
    // o coletor os recolhe.
    if (saved != true || !mounted) return;

    if (from.id == to.id) {
      messenger.showSnackBar(
        SnackBar(content: Text(l.finSameAccount)),
      );
      return;
    }
    if (valorSaida == null || valorEntrada == null) {
      messenger.showSnackBar(
        SnackBar(content: Text(l.finAmountsRequired)),
      );
      return;
    }
    await _repository.createTransfer(
      fromAccountId: from.id!,
      toAccountId: to.id!,
      valorSaida: valorSaida,
      valorEntrada: valorEntrada,
      date: toIsoDay(date),
      notes: notes.isEmpty ? null : notes,
    );
    await _load();
  }

  Future<void> _openLoans() async {
    await Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => FinancasLoansScreen(accounts: _accounts)),
    );
    await _load();
  }

  Future<void> _openLiabilities() async {
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => FinancasLiabilitiesScreen(accounts: _accounts),
      ),
    );
    await _load();
  }

  Future<void> _openCategories() async {
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => FinancasCategoriesScreen(
          base: _base,
          ratesToBrl: _rates,
          accounts: _accounts,
        ),
      ),
    );
  }

  Future<void> _open(BankAccount account) async {
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => FinancasTransactionsScreen(account: account),
      ),
    );
    await _load();
  }

  Widget _summaryCard() {
    final l = AppLocalizations.of(context);
    final net = _netWorth;
    final textTheme = Theme.of(context).textTheme;
    final missingNote = net == null ? null : missingItemsNote(l, net.itemsMissingFx);
    return Card(
      margin: const EdgeInsets.only(bottom: 16),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(l.finNetWorth, style: textTheme.titleSmall),
                ),
                // Atualizando por baixo (troca de base, refresh): o número
                // antigo fica na tela até o novo chegar.
                if (_summaryLoading && net != null)
                  const Padding(
                    padding: EdgeInsets.only(right: 8),
                    child: SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  ),
                DropdownButton<String>(
                  key: const Key('financas-base-dropdown'),
                  value: _base,
                  underline: const SizedBox.shrink(),
                  items: [
                    for (final code in currencyCatalog)
                      DropdownMenuItem(value: code, child: Text(code)),
                  ],
                  onChanged: (code) {
                    if (code != null && code != _base) _changeBase(code);
                  },
                ),
              ],
            ),
            if (net == null)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 8),
                child: Text(l.finCalculating),
              )
            else ...[
              Text(
                formatMoney(net.total, net.base, locale: l.localeName),
                key: const Key('financas-net-worth'),
                style: textTheme.headlineSmall,
              ),
              const SizedBox(height: 8),
              Text(
                l.finCash(formatMoney(net.cash, net.base, locale: l.localeName)),
                key: const Key('financas-cash'),
              ),
              Text(
                l.finPortfolio(formatMoney(net.portfolio, net.base, locale: l.localeName)),
                key: const Key('financas-portfolio'),
              ),
              if (net.loansReceivable > 0)
                Text(
                  l.finLoansReceivable(
                    formatMoney(net.loansReceivable, net.base, locale: l.localeName),
                  ),
                  key: const Key('financas-loans'),
                ),
              if (net.debt > 0)
                Text(
                  l.finDebtsLine(formatMoney(net.debt, net.base, locale: l.localeName)),
                  key: const Key('financas-debt'),
                ),
              if (missingNote != null)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(
                    missingNote,
                    key: const Key('financas-missing'),
                    style: textTheme.bodySmall,
                  ),
                ),
              if (_fxFailed.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Text(
                    fxUnavailableBanner(l, _fxFailed),
                    key: const Key('financas-fx-banner'),
                    style: textTheme.bodySmall,
                  ),
                ),
            ],
          ],
        ),
      ),
    );
  }

  /// "≈ BRL 350,00" sob o saldo de uma conta em outra moeda; "sem câmbio"
  /// quando não há taxa; nada enquanto o resumo ainda calcula ou quando a
  /// conta já está na base.
  Widget? _equivalent(BankAccount account, double balance) {
    if (account.currency == _base || _netWorth == null) return null;
    final l = AppLocalizations.of(context);
    final converted = convertToBase(balance, account.currency, _base, _rates);
    return Text(
      converted == null
          ? l.finNoRate('${account.currency}/$_base')
          : '≈ ${formatMoney(converted, _base, locale: l.localeName)}',
      key: Key('financas-equivalent-${account.id}'),
      style: Theme.of(context).textTheme.bodySmall,
    );
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(
        title: Text(l.finTitle),
        actions: [
          IconButton(
            tooltip: l.finTransfer,
            icon: const Icon(Icons.swap_horiz),
            onPressed: _transfer,
          ),
          IconButton(
            tooltip: l.finLoans,
            icon: const Icon(Icons.handshake_outlined),
            onPressed: _openLoans,
          ),
          IconButton(
            tooltip: l.finDebts,
            icon: const Icon(Icons.request_quote_outlined),
            onPressed: _openLiabilities,
          ),
          IconButton(
            tooltip: l.finCategories,
            icon: const Icon(Icons.category_outlined),
            onPressed: _openCategories,
          ),
          IconButton(
            tooltip: l.finRefresh,
            icon: const Icon(Icons.refresh),
            onPressed: _load,
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => _editAccount(),
        icon: const Icon(Icons.add),
        label: Text(l.finFabAccount),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                _summaryCard(),
                if (_accounts.isEmpty) Center(child: Text(l.finEmptyAccounts)),
                for (final account in _accounts)
                  _accountCard(account, _balances[account.id] ?? 0),
              ],
            ),
    );
  }

  Widget _accountCard(BankAccount account, double balance) {
    final l = AppLocalizations.of(context);
    final equivalent = _equivalent(account, balance);
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: ListTile(
        title: Text(account.nome),
        subtitle: Text(account.titular),
        onTap: () => _open(account),
        trailing: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text(
                  formatMoney(balance, account.currency, locale: l.localeName),
                  key: Key('financas-balance-${account.id}'),
                ),
                ?equivalent,
              ],
            ),
            PopupMenuButton<String>(
              onSelected: (action) => action == 'edit'
                  ? _editAccount(account)
                  : _deleteAccount(account),
              itemBuilder: (_) => [
                PopupMenuItem(value: 'edit', child: Text(l.actionEdit)),
                PopupMenuItem(value: 'delete', child: Text(l.actionDelete)),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
