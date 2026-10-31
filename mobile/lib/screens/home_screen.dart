import 'package:flutter/material.dart';

import '../domain/fx_history.dart';
import '../domain/general_finance.dart';
import '../domain/home_layout.dart';
import '../domain/home_summary.dart';
import '../domain/money_format.dart';
import '../services/general_finance_repository.dart';
import '../services/home_layout_repository.dart';
import '../services/live_positions.dart';
import '../services/net_worth_loader.dart';
import '../services/portfolio_repository.dart';
import '../services/quote_dispatcher.dart';
import '../services/yahoo_quote_service.dart';

/// "+BRL 70,00": o lucro com sinal, na moeda base.
@visibleForTesting
String homeProfitAmountLabel(double profit, String base) =>
    '${profit >= 0 ? '+' : '-'}${formatMoney(profit.abs(), base)}';

/// "+23,3%": o lucro sobre o custo, com sinal.
@visibleForTesting
String homeProfitPercentLabel(double percent) =>
    '${percent >= 0 ? '+' : '-'}${percent.abs().toStringAsFixed(1)}%';

const _allocationColors = [
  Color(0xFF4ADE80),
  Color(0xFF60A5FA),
  Color(0xFFFBBF24),
  Color(0xFFF472B6),
  Color(0xFFA78BFA),
  Color(0xFF2DD4BF),
  Color(0xFFFB923C),
  Color(0xFF94A3B8),
];

/// Pela posição na lista ordenada, não pela classe: com 15 classes e 8 cores, a cor
/// por classe repetiria (cripto e ação BR saíam do mesmo verde).
Color _colorAt(int rank) => _allocationColors[rank % _allocationColors.length];

/// O valor grande do card numa linha só: na meia largura "BRL 28.860,00" quebrava em
/// duas linhas, então ele encolhe para caber em vez de quebrar.
Widget _bigNumber(Text text) => FittedBox(
  fit: BoxFit.scaleDown,
  alignment: Alignment.centerLeft,
  child: text,
);

/// Aba Início: cards-resumo da carteira e do patrimônio na moeda base, que o dono
/// reordena, esconde e alterna entre meia e cheia largura (modo Personalizar). O
/// layout é por aparelho, como todo o resto do mobile. Os 4 widgets usam só dados que
/// o celular já tem; retorno TWR, favoritos e carteiras múltiplas do desktop ficam de
/// fora.
class HomeScreen extends StatefulWidget {
  /// Série da moeda em reais; o padrão é o Yahoo com o fallback de inverso e de
  /// pernas via dólar. Injetável para teste, sem rede.
  final Future<FxHistory> Function(String currency)? fetchFx;

  /// Posições com preço atual; o padrão lê a carteira e cota na rede.
  final Future<List<LivePositionRow>> Function()? loadPositions;

  const HomeScreen({super.key, this.fetchFx, this.loadPositions});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  final _layoutRepository = HomeLayoutRepository();
  final _finance = GeneralFinanceRepository();
  final _portfolio = PortfolioRepository();
  final _dispatcher = QuoteDispatcher();
  final _yahoo = YahooQuoteService();

  List<HomeWidgetConfig> _layout = defaultHomeLayout();

  /// Rascunho do modo Personalizar; `null` fora dele. Só vira [_layout] ao salvar.
  List<HomeWidgetConfig>? _draft;

  bool _loading = true;
  Object? _error;
  HomeSummary? _summary;
  NetWorth? _netWorth;
  Set<String> _fxFailed = {};
  int _run = 0;

  @override
  void initState() {
    super.initState();
    _loadLayout();
    _loadData();
  }

  Future<void> _loadLayout() async {
    final layout = await _layoutRepository.load();
    if (!mounted) return;
    setState(() => _layout = layout);
  }

  Future<void> _loadData() async {
    final run = ++_run;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final base = await _finance.getBaseCurrency();
      final accounts = await _finance.listAccounts();
      final transactions = await _finance.listTransactions();
      final balances = computeBalances(transactions, toIsoDay(DateTime.now()));
      final rows = await (widget.loadPositions ?? _livePositions)();
      final net = await loadNetWorth(
        repository: _finance,
        accounts: accounts,
        balances: balances,
        base: base,
        rows: rows,
        fetchFx: widget.fetchFx ?? _fetchFx,
      );
      final summary = computeHomeSummary(rows, base, net.ratesToBrl);
      // Um refresh no meio deixa esta rodada velha.
      if (!mounted || run != _run) return;
      setState(() {
        _summary = summary;
        _netWorth = net.netWorth;
        _fxFailed = net.fxFailed;
        _loading = false;
      });
    } catch (e) {
      if (!mounted || run != _run) return;
      setState(() {
        _error = e;
        _loading = false;
      });
    }
  }

  Future<List<LivePositionRow>> _livePositions() async => loadLivePositions(
    await _portfolio.getPositions(),
    repository: _portfolio,
    dispatcher: _dispatcher,
  );

  Future<FxHistory> _fetchFx(String currency) =>
      _yahoo.fetchFxHistoryWithFallback(
        from: DateTime.now().subtract(const Duration(days: 7)),
        currency: currency,
      );

  Future<void> _saveDraft() async {
    final draft = _draft;
    if (draft == null) return;
    final messenger = ScaffoldMessenger.of(context);
    try {
      await _layoutRepository.save(draft);
    } catch (e) {
      messenger.showSnackBar(
        SnackBar(content: Text('Não consegui salvar o layout: $e')),
      );
      return;
    }
    if (!mounted) return;
    setState(() {
      _layout = draft;
      _draft = null;
    });
  }

  void _updateDraft(int index, HomeWidgetConfig Function(HomeWidgetConfig) f) {
    final draft = [..._draft!];
    draft[index] = f(draft[index]);
    setState(() => _draft = draft);
  }

  @override
  Widget build(BuildContext context) {
    final editing = _draft != null;
    return Scaffold(
      appBar: AppBar(
        title: Text(editing ? 'Personalizar Início' : 'Início'),
        actions: editing
            ? [
                TextButton(
                  key: const Key('home-cancel'),
                  onPressed: () => setState(() => _draft = null),
                  child: const Text('Cancelar'),
                ),
                TextButton(
                  key: const Key('home-save'),
                  onPressed: _saveDraft,
                  child: const Text('Salvar'),
                ),
              ]
            : [
                IconButton(
                  key: const Key('home-customize'),
                  tooltip: 'Personalizar',
                  icon: const Icon(Icons.tune),
                  onPressed: () => setState(() => _draft = [..._layout]),
                ),
              ],
      ),
      body: editing ? _editor() : _dashboard(),
    );
  }

  Widget _editor() {
    final draft = _draft!;
    return Column(
      children: [
        Align(
          alignment: Alignment.centerRight,
          child: TextButton.icon(
            key: const Key('home-reset'),
            onPressed: () => setState(() => _draft = defaultHomeLayout()),
            icon: const Icon(Icons.restore),
            label: const Text('Restaurar padrão'),
          ),
        ),
        Expanded(
          child: ReorderableListView(
            buildDefaultDragHandles: false,
            onReorderItem: (oldIndex, newIndex) => setState(
              () => _draft = reorderHomeLayout(draft, oldIndex, newIndex),
            ),
            children: [
              for (var i = 0; i < draft.length; i++)
                ListTile(
                  key: Key('home-item-${draft[i].key.id}'),
                  leading: ReorderableDragStartListener(
                    key: Key('home-handle-${draft[i].key.id}'),
                    index: i,
                    child: const Icon(Icons.drag_handle),
                  ),
                  title: Text(draft[i].key.label),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      TextButton(
                        key: Key('home-wide-${draft[i].key.id}'),
                        onPressed: () => _updateDraft(
                          i,
                          (c) => c.copyWith(wide: !c.wide),
                        ),
                        child: Text(draft[i].wide ? 'Cheia' : 'Meia'),
                      ),
                      Switch(
                        key: Key('home-visible-${draft[i].key.id}'),
                        value: draft[i].visible,
                        onChanged: (visible) =>
                            _updateDraft(i, (c) => c.copyWith(visible: visible)),
                      ),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }

  Widget _dashboard() {
    final rows = homeRows(_layout);
    return RefreshIndicator(
      onRefresh: _loadData,
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (_error != null)
            Card(
              key: const Key('home-error'),
              color: Theme.of(context).colorScheme.errorContainer,
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: Text(
                  'Não consegui carregar as cotações agora. '
                  'Puxe para atualizar. ($_error)',
                ),
              ),
            ),
          if (rows.isEmpty)
            const Padding(
              padding: EdgeInsets.all(32),
              child: Text(
                'Todos os cards estão escondidos. Toque em Personalizar para mostrá-los.',
                textAlign: TextAlign.center,
              ),
            ),
          for (final row in rows)
            Padding(
              padding: const EdgeInsets.only(bottom: 12),
              child: IntrinsicHeight(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (var i = 0; i < row.length; i++) ...[
                      if (i > 0) const SizedBox(width: 12),
                      Expanded(child: _card(row[i])),
                    ],
                    // meia largura sem par continua com a largura de meia
                    if (row.length == 1 && !row.first.wide) ...[
                      const SizedBox(width: 12),
                      const Expanded(child: SizedBox.shrink()),
                    ],
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }

  Widget _card(HomeWidgetConfig config) {
    final Widget content;
    switch (config.key) {
      case HomeWidgetKey.totalValue:
        content = _totalValue();
      case HomeWidgetKey.profit:
        content = _profit();
      case HomeWidgetKey.allocation:
        content = _allocation();
      case HomeWidgetKey.netWorth:
        content = _netWorthCard();
    }
    return Card(
      key: Key('home-card-${config.key.id}'),
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              config.key.label,
              style: Theme.of(context).textTheme.labelLarge,
            ),
            const SizedBox(height: 8),
            content,
          ],
        ),
      ),
    );
  }

  /// Enquanto carrega pela primeira vez, um indicador; com erro e sem dado, um traço.
  Widget _placeholder() {
    if (_loading) {
      return const SizedBox(
        height: 20,
        width: 20,
        child: CircularProgressIndicator(strokeWidth: 2),
      );
    }
    return const Text('—');
  }

  Widget _totalValue() {
    final summary = _summary;
    if (summary == null) return _placeholder();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _bigNumber(
          Text(
            formatMoney(summary.totalValue, summary.base),
            key: const Key('home-total-value-text'),
            style: Theme.of(context).textTheme.headlineSmall,
          ),
        ),
        if (summary.itemsMissing > 0)
          Text(
            '${summary.itemsMissing} fora da soma (sem cotação ou câmbio)',
            key: const Key('home-total-value-missing'),
            style: Theme.of(context).textTheme.bodySmall,
          ),
      ],
    );
  }

  Widget _profit() {
    final summary = _summary;
    if (summary == null) return _placeholder();
    final profit = summary.profit;
    final percent = summary.profitPercent;
    final scheme = Theme.of(context).colorScheme;
    final color = profit == null
        ? null
        : (profit >= 0 ? scheme.primary : scheme.error);
    // O percentual é o número grande (cabe na meia largura); o valor em reais vai
    // embaixo. Sem custo para dividir (percentual nulo), o valor em reais é o grande.
    final amount = profit == null
        ? null
        : homeProfitAmountLabel(profit, summary.base);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _bigNumber(
          Text(
            percent != null ? homeProfitPercentLabel(percent) : (amount ?? '—'),
            key: const Key('home-profit-text'),
            style: Theme.of(
              context,
            ).textTheme.headlineSmall?.copyWith(color: color),
          ),
        ),
        if (percent != null && amount != null)
          _bigNumber(
            Text(
              amount,
              key: const Key('home-profit-amount'),
              style: Theme.of(
                context,
              ).textTheme.bodyMedium?.copyWith(color: color),
            ),
          ),
        if (summary.profitExcluded > 0)
          Text(
            '${summary.profitExcluded} fora do cálculo',
            key: const Key('home-profit-excluded'),
            style: Theme.of(context).textTheme.bodySmall,
          ),
      ],
    );
  }

  Widget _allocation() {
    final summary = _summary;
    if (summary == null) return _placeholder();
    if (summary.allocation.isEmpty) return const Text('Sem posições abertas.');
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        ClipRRect(
          borderRadius: BorderRadius.circular(6),
          child: Row(
            children: [
              for (final (i, slice) in summary.allocation.indexed)
                Expanded(
                  flex: (slice.share * 1000).round().clamp(1, 1000),
                  child: Container(height: 14, color: _colorAt(i)),
                ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        for (final (i, slice) in summary.allocation.indexed)
          Padding(
            key: Key('home-allocation-row-${slice.assetClass.value}'),
            padding: const EdgeInsets.symmetric(vertical: 2),
            child: Row(
              children: [
                Container(
                  width: 10,
                  height: 10,
                  decoration: BoxDecoration(
                    color: _colorAt(i),
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(child: Text(slice.assetClass.label)),
                Text('${(slice.share * 100).toStringAsFixed(1)}%'),
              ],
            ),
          ),
      ],
    );
  }

  Widget _netWorthCard() {
    final net = _netWorth;
    if (net == null) return _placeholder();
    Widget line(String label, double value, {String? key}) => Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        children: [
          Expanded(child: Text(label)),
          Text(formatMoney(value, net.base), key: key == null ? null : Key(key)),
        ],
      ),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _bigNumber(
          Text(
            formatMoney(net.total, net.base),
            key: const Key('home-net-worth-text'),
            style: Theme.of(context).textTheme.headlineSmall,
          ),
        ),
        const SizedBox(height: 8),
        line('Caixa', net.cash),
        line('Carteira', net.portfolio),
        if (net.loansReceivable != 0) line('Empréstimos', net.loansReceivable),
        if (net.debt != 0) line('Dívidas', -net.debt),
        if (net.itemsMissingFx > 0)
          Text(
            '${net.itemsMissingFx} item(ns) fora da soma por falta de câmbio ou cotação.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        if (_fxFailed.isNotEmpty)
          Text(
            'Câmbio indisponível agora para ${(_fxFailed.toList()..sort()).join(', ')}.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
      ],
    );
  }
}
