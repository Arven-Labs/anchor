import 'package:flutter/material.dart';

import '../domain/currency_catalog.dart';
import '../domain/fx_history.dart';
import '../domain/general_finance.dart';
import '../domain/money_format.dart';
import '../models/bank_account.dart';
import '../services/fx_rates_loader.dart';
import '../services/general_finance_repository.dart';
import '../services/live_positions.dart';
import '../services/portfolio_repository.dart';
import '../services/quote_dispatcher.dart';
import '../services/yahoo_quote_service.dart';
import 'financas_categories_screen.dart';
import 'financas_liabilities_screen.dart';
import 'financas_loans_screen.dart';
import 'financas_transactions_screen.dart';

/// Aviso de câmbio que não veio: os itens dessas moedas ficam fora da soma.
@visibleForTesting
String fxUnavailableBanner(Set<String> failed) =>
    'Câmbio indisponível agora para ${(failed.toList()..sort()).join(', ')}. '
    'Os itens dessas moedas ficam fora do patrimônio.';

/// Linha do resumo quando algo ficou de fora (conta sem câmbio ou posição sem
/// cotação); `null` quando está tudo dentro.
@visibleForTesting
String? missingItemsNote(int count) {
  if (count == 0) return null;
  return count == 1
      ? '1 item fora da soma por falta de câmbio ou cotação.'
      : '$count itens fora da soma por falta de câmbio ou cotação.';
}

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
    final cashItems = [
      for (final account in _accounts)
        ValuedItem(account.currency, _balances[account.id] ?? 0),
    ];
    final portfolioItems = portfolioValuedItems(rows);
    final currencyOf = {for (final a in _accounts) a.id!: a.currency};
    final today = toIsoDay(DateTime.now());
    final debtItems = [
      for (final l in await _repository.listLiabilities())
        ValuedItem(
          currencyOf[l.liability.bankAccountId] ?? defaultBaseCurrency,
          l.balanceOn(today),
        ),
    ];
    final now = DateTime.now();
    final loanItems = [
      for (final l in await _repository.listLoans())
        if (l.loan.includeInNetWorth)
          ValuedItem(l.loan.currency, l.balanceOn(now)),
    ];
    final fx = await loadRatesToBrl(
      {
        _base,
        for (final item in [
          ...cashItems,
          ...portfolioItems,
          ...debtItems,
          ...loanItems,
        ])
          item.currency,
      },
      fetch: widget.fetchFx ?? _fetchFx,
    );
    final net = computeNetWorth(
      base: _base,
      cashItems: cashItems,
      portfolioItems: portfolioItems,
      debtItems: debtItems,
      loanItems: loanItems,
      ratesToBrl: fx.ratesToBrl,
    );

    // Uma troca de base ou um refresh no meio deixa esta rodada velha.
    if (!mounted || run != _summaryRun) return;
    setState(() {
      _netWorth = net;
      _rates = fx.ratesToBrl;
      _fxFailed = fx.failed;
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
          title: Text(existing == null ? 'Nova conta' : 'Editar conta'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                controller: nomeController,
                decoration: const InputDecoration(
                  labelText: 'Nome',
                  hintText: 'Nubank',
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: titularController,
                decoration: const InputDecoration(labelText: 'Titular'),
              ),
              const SizedBox(height: 12),
              DropdownButtonFormField<String>(
                initialValue: currency,
                decoration: InputDecoration(
                  labelText: 'Moeda',
                  helperText: currencyLocked
                      ? 'Não muda numa conta que já tem lançamentos.'
                      : null,
                ),
                items: [
                  for (final (code, name) in currencyCatalog)
                    DropdownMenuItem(value: code, child: Text('$code — $name')),
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
              child: const Text('Cancelar'),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: const Text('Salvar'),
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
      messenger.showSnackBar(SnackBar(content: Text(e.message)));
      return;
    }
    await _load();
  }

  Future<void> _deleteAccount(BankAccount account) async {
    final inUse = await _repository.accountInUse(account.id!);
    if (!mounted) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Excluir ${account.nome}?'),
        content: Text(
          inUse
              ? 'Os lançamentos desta conta também serão apagados.'
              : 'A conta não tem lançamentos.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancelar'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Excluir'),
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
    if (_accounts.length < 2) {
      messenger.showSnackBar(
        const SnackBar(content: Text('Cadastre duas contas para transferir.')),
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
            title: const Text('Transferir'),
            content: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  DropdownButtonFormField<BankAccount>(
                    key: const Key('financas-transfer-from'),
                    initialValue: from,
                    isExpanded: true,
                    decoration: const InputDecoration(labelText: 'De'),
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
                    decoration: const InputDecoration(labelText: 'Para'),
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
                          ? 'Saiu (${from.currency})'
                          : 'Valor (${from.currency})',
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
                        labelText: 'Chegou (${to.currency})',
                        helperText: out != null && incoming != null
                            ? 'Taxa efetiva: 1 ${from.currency} = '
                                  '${(incoming / out).toStringAsFixed(4)} '
                                  '${to.currency}'
                            : 'Spread e tarifa já embutidos nos valores.',
                      ),
                    ),
                  ],
                  const SizedBox(height: 12),
                  OutlinedButton.icon(
                    icon: const Icon(Icons.calendar_today_outlined),
                    label: Text(formatIsoDay(toIsoDay(date))),
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
                    decoration: const InputDecoration(labelText: 'Observação'),
                  ),
                ],
              ),
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(context).pop(false),
                child: const Text('Cancelar'),
              ),
              FilledButton(
                onPressed: () => Navigator.of(context).pop(true),
                child: const Text('Transferir'),
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
        const SnackBar(content: Text('Escolha duas contas diferentes.')),
      );
      return;
    }
    if (valorSaida == null || valorEntrada == null) {
      messenger.showSnackBar(
        const SnackBar(content: Text('Informe os valores, maiores que zero.')),
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
    final net = _netWorth;
    final textTheme = Theme.of(context).textTheme;
    final missingNote = net == null ? null : missingItemsNote(net.itemsMissingFx);
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
                  child: Text('Patrimônio', style: textTheme.titleSmall),
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
                    for (final (code, _) in currencyCatalog)
                      DropdownMenuItem(value: code, child: Text(code)),
                  ],
                  onChanged: (code) {
                    if (code != null && code != _base) _changeBase(code);
                  },
                ),
              ],
            ),
            if (net == null)
              const Padding(
                padding: EdgeInsets.symmetric(vertical: 8),
                child: Text('Calculando…'),
              )
            else ...[
              Text(
                formatMoney(net.total, net.base),
                key: const Key('financas-net-worth'),
                style: textTheme.headlineSmall,
              ),
              const SizedBox(height: 8),
              Text(
                'Caixa ${formatMoney(net.cash, net.base)}',
                key: const Key('financas-cash'),
              ),
              Text(
                'Carteira ${formatMoney(net.portfolio, net.base)}',
                key: const Key('financas-portfolio'),
              ),
              if (net.loansReceivable > 0)
                Text(
                  'Empréstimos a receber ${formatMoney(net.loansReceivable, net.base)}',
                  key: const Key('financas-loans'),
                ),
              if (net.debt > 0)
                Text(
                  'Dívidas −${formatMoney(net.debt, net.base)}',
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
                    fxUnavailableBanner(_fxFailed),
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
    final converted = convertToBase(balance, account.currency, _base, _rates);
    return Text(
      converted == null
          ? 'sem câmbio ${account.currency}/$_base'
          : '≈ ${formatMoney(converted, _base)}',
      key: Key('financas-equivalent-${account.id}'),
      style: Theme.of(context).textTheme.bodySmall,
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Finanças'),
        actions: [
          IconButton(
            tooltip: 'Transferir',
            icon: const Icon(Icons.swap_horiz),
            onPressed: _transfer,
          ),
          IconButton(
            tooltip: 'Empréstimos',
            icon: const Icon(Icons.handshake_outlined),
            onPressed: _openLoans,
          ),
          IconButton(
            tooltip: 'Dívidas',
            icon: const Icon(Icons.request_quote_outlined),
            onPressed: _openLiabilities,
          ),
          IconButton(
            tooltip: 'Categorias',
            icon: const Icon(Icons.category_outlined),
            onPressed: _openCategories,
          ),
          IconButton(
            tooltip: 'Atualizar',
            icon: const Icon(Icons.refresh),
            onPressed: _load,
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => _editAccount(),
        icon: const Icon(Icons.add),
        label: const Text('Conta'),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                _summaryCard(),
                if (_accounts.isEmpty)
                  const Center(child: Text('Nenhuma conta cadastrada ainda.')),
                for (final account in _accounts)
                  _accountCard(account, _balances[account.id] ?? 0),
              ],
            ),
    );
  }

  Widget _accountCard(BankAccount account, double balance) {
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
                  formatMoney(balance, account.currency),
                  key: Key('financas-balance-${account.id}'),
                ),
                ?equivalent,
              ],
            ),
            PopupMenuButton<String>(
              onSelected: (action) => action == 'edit'
                  ? _editAccount(account)
                  : _deleteAccount(account),
              itemBuilder: (_) => const [
                PopupMenuItem(value: 'edit', child: Text('Editar')),
                PopupMenuItem(value: 'delete', child: Text('Excluir')),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
