import 'package:material_ui/material_ui.dart';

import '../domain/finance_error.dart';
import '../domain/number_locale.dart';
import '../l10n/app_localizations.dart';
import '../l10n/finance_labels.dart';
import '../models/asset.dart';
import '../models/position.dart';
import '../models/watchlist.dart';
import '../services/live_positions.dart';
import '../services/portfolio_repository.dart';
import '../services/quote_dispatcher.dart';
import '../services/watchlist_repository.dart';
import 'portfolio_screen.dart' show livePriceLabel;

/// Favoritos e watchlists nomeadas (Fase 10.4 do desktop): cada item leva preço-alvo e
/// anotação, e o preço atual aparece ao lado. Só exibição: sem comparação com o alvo.
class WatchlistScreen extends StatefulWidget {
  /// Preço atual dos ativos dados; o padrão cota na rede pela fonte da classe de cada um
  /// (a mesma da Carteira). Injetável para teste, sem rede.
  final Future<List<LivePositionRow>> Function(List<Asset> assets)? loadPrices;

  const WatchlistScreen({super.key, this.loadPrices});

  @override
  State<WatchlistScreen> createState() => _WatchlistScreenState();
}

class _WatchlistScreenState extends State<WatchlistScreen> {
  final _repository = WatchlistRepository();
  final _portfolio = PortfolioRepository();
  final _dispatcher = QuoteDispatcher();
  final _nameController = TextEditingController();

  bool _loading = true;
  List<Watchlist> _watchlists = [];
  int? _selectedId;
  List<WatchlistEntry> _entries = [];
  List<Asset> _favorites = [];
  List<Asset> _allAssets = [];
  Map<int, LivePositionRow> _prices = {};
  bool _pricesLoading = false;
  bool _confirmingDelete = false;
  Object? _error;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  Future<List<LivePositionRow>> _fetchPrices(List<Asset> assets) {
    final custom = widget.loadPrices;
    if (custom != null) return custom(assets);
    return loadLivePositions(
      [for (final a in assets) Position(asset: a, netQuantity: 0, averageBuyPrice: null)],
      repository: _portfolio,
      dispatcher: _dispatcher,
    );
  }

  /// Relê o banco e depois busca os preços (a rede pode demorar: a lista aparece antes).
  Future<void> _load({int? select}) async {
    final watchlists = await _repository.listWatchlists();
    final favorites = await _repository.listFavorites();
    final assets = await _portfolio.listAssets();
    var selected = select ?? _selectedId;
    if (selected == null || !watchlists.any((w) => w.id == selected)) {
      selected = watchlists.isEmpty ? null : watchlists.first.id;
    }
    final entries = selected == null ? <WatchlistEntry>[] : await _repository.listItems(selected);
    if (!mounted) return;
    setState(() {
      _watchlists = watchlists;
      _selectedId = selected;
      _entries = entries;
      _favorites = favorites;
      _allAssets = assets;
      _loading = false;
      _pricesLoading = true;
    });

    final byId = <int, Asset>{
      for (final a in favorites) a.id!: a,
      for (final e in entries) e.asset.id!: e.asset,
    };
    var prices = <int, LivePositionRow>{};
    if (byId.isNotEmpty) {
      try {
        final rows = await _fetchPrices(byId.values.toList());
        prices = {for (final r in rows) r.position.asset.id!: r};
      } catch (_) {}
    }
    if (!mounted) return;
    setState(() {
      _prices = prices;
      _pricesLoading = false;
    });
  }

  Future<void> _guard(Future<void> Function() action) async {
    try {
      await action();
      if (mounted) setState(() => _error = null);
    } on FinanceArgumentError catch (e) {
      if (mounted) setState(() => _error = e);
    } on FinanceStateError catch (e) {
      if (mounted) setState(() => _error = e);
    }
  }

  Future<void> _create() => _guard(() async {
    final id = await _repository.createWatchlist(_nameController.text);
    _nameController.clear();
    _confirmingDelete = false;
    await _load(select: id);
  });

  Future<void> _delete() async {
    final id = _selectedId;
    if (id == null) return;
    if (!_confirmingDelete) {
      setState(() => _confirmingDelete = true);
      return;
    }
    await _repository.deleteWatchlist(id);
    _confirmingDelete = false;
    _selectedId = null;
    await _load();
  }

  Future<void> _removeFavorite(Asset asset) async {
    await _repository.toggleFavorite(asset.id!);
    await _load();
  }

  Future<void> _removeItem(WatchlistEntry entry) async {
    await _repository.removeItem(entry.item.id);
    await _load();
  }

  Future<void> _openItemDialog({WatchlistEntry? editing}) async {
    final watchlistId = _selectedId;
    if (watchlistId == null) return;
    final saved = await showDialog<bool>(
      context: context,
      builder: (_) => _ItemDialog(
        assets: _allAssets,
        editing: editing,
        onSubmit: (assetId, target, notes) async {
          if (editing != null) {
            await _repository.updateItem(editing.item.id, targetPrice: target, notes: notes);
          } else {
            await _repository.addItem(
              watchlistId: watchlistId,
              assetId: assetId!,
              targetPrice: target,
              notes: notes,
            );
          }
        },
      ),
    );
    if (saved == true) await _load();
  }

  String _priceText(AppLocalizations l, Asset asset) {
    if (_pricesLoading) return l.portfolioCurrentPrice('…');
    final row = _prices[asset.id];
    return l.portfolioCurrentPrice(
      livePriceLabel(
        l,
        row?.livePrice,
        unavailable: row?.priceUnavailable ?? false,
        currency: asset.currency,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.watchlistsTitle)),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                _favoritesCard(l),
                const SizedBox(height: 16),
                _watchlistsCard(l),
              ],
            ),
    );
  }

  Widget _favoritesCard(AppLocalizations l) => Card(
    key: const Key('favorites-block'),
    child: Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('★ ${l.watchlistsFavorites}', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          if (_favorites.isEmpty)
            Text(l.watchlistsFavoritesEmpty)
          else
            for (final asset in _favorites)
              ListTile(
                contentPadding: EdgeInsets.zero,
                title: Text(asset.ticker),
                subtitle: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [Text(asset.name), Text(_priceText(l, asset))],
                ),
                trailing: IconButton(
                  key: Key('favorite-remove-${asset.ticker}'),
                  icon: const Icon(Icons.star),
                  tooltip: l.favoriteRemove,
                  onPressed: () => _removeFavorite(asset),
                ),
              ),
        ],
      ),
    ),
  );

  Widget _watchlistsCard(AppLocalizations l) => Card(
    child: Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(l.watchlistsTitle, style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          if (_watchlists.isEmpty)
            Text(l.watchlistsEmpty)
          else
            Row(
              children: [
                Expanded(
                  child: DropdownButtonFormField<int>(
                    key: ValueKey('watchlist-select-$_selectedId-${_watchlists.length}'),
                    initialValue: _selectedId,
                    decoration: InputDecoration(labelText: l.watchlistsSelect),
                    items: [
                      for (final w in _watchlists) DropdownMenuItem(value: w.id, child: Text(w.name)),
                    ],
                    onChanged: (id) {
                      if (id == null) return;
                      _confirmingDelete = false;
                      _load(select: id);
                    },
                  ),
                ),
                const SizedBox(width: 8),
                OutlinedButton(
                  key: const Key('watchlist-delete-button'),
                  onPressed: _delete,
                  child: Text(_confirmingDelete ? l.watchlistsConfirmDelete : l.actionDelete),
                ),
              ],
            ),
          const SizedBox(height: 16),
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: TextField(
                  key: const Key('watchlist-name-field'),
                  controller: _nameController,
                  decoration: InputDecoration(labelText: l.watchlistsNewName),
                ),
              ),
              const SizedBox(width: 8),
              FilledButton(
                key: const Key('watchlist-create-button'),
                onPressed: _create,
                child: Text(l.watchlistsCreate),
              ),
            ],
          ),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                financeErrorText(l, _error!),
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          if (_selectedId != null) ...[
            const SizedBox(height: 16),
            FilledButton.tonal(
              key: const Key('watchlist-add-item-button'),
              onPressed: () => _openItemDialog(),
              child: Text(l.watchlistsAddAsset),
            ),
            const SizedBox(height: 8),
            for (final entry in _entries) _entryTile(l, entry),
          ],
        ],
      ),
    ),
  );

  Widget _entryTile(AppLocalizations l, WatchlistEntry entry) {
    final target = entry.item.targetPrice;
    final notes = entry.item.notes;
    return ListTile(
      contentPadding: EdgeInsets.zero,
      title: Text(entry.asset.ticker),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(entry.asset.name),
          Text(_priceText(l, entry.asset)),
          Text(
            target == null
                ? l.watchlistsNoTarget
                : l.watchlistsTarget(formatFixed(target, 2, locale: l.localeName)),
          ),
          if (notes != null) Text(notes),
        ],
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            key: Key('watchlist-item-edit-${entry.item.id}'),
            icon: const Icon(Icons.edit),
            tooltip: l.actionEdit,
            onPressed: () => _openItemDialog(editing: entry),
          ),
          IconButton(
            key: Key('watchlist-item-remove-${entry.item.id}'),
            icon: const Icon(Icons.delete_outline),
            tooltip: l.watchlistsRemove,
            onPressed: () => _removeItem(entry),
          ),
        ],
      ),
    );
  }
}

/// Diálogo de incluir (ativo + alvo + anotação) ou editar (alvo + anotação) um item. O erro
/// de validação aparece aqui dentro, sem fechar.
class _ItemDialog extends StatefulWidget {
  final List<Asset> assets;
  final WatchlistEntry? editing;
  final Future<void> Function(int? assetId, double? target, String? notes) onSubmit;

  const _ItemDialog({required this.assets, required this.editing, required this.onSubmit});

  @override
  State<_ItemDialog> createState() => _ItemDialogState();
}

class _ItemDialogState extends State<_ItemDialog> {
  late final TextEditingController _target;
  late final TextEditingController _notes;
  int? _assetId;
  Object? _error;

  @override
  void initState() {
    super.initState();
    final item = widget.editing?.item;
    _target = TextEditingController(
      text: item?.targetPrice == null ? '' : formatFixed(item!.targetPrice!, 2, locale: 'en'),
    );
    _notes = TextEditingController(text: item?.notes ?? '');
  }

  @override
  void dispose() {
    _target.dispose();
    _notes.dispose();
    super.dispose();
  }

  /// Em branco = sem alvo; texto que não é número vira NaN, que a regra do domínio recusa.
  double? _parseTarget() {
    final text = _target.text.trim();
    if (text.isEmpty) return null;
    return double.tryParse(text.replaceAll(',', '.')) ?? double.nan;
  }

  Future<void> _submit() async {
    try {
      await widget.onSubmit(_assetId, _parseTarget(), _notes.text);
      if (mounted) Navigator.of(context).pop(true);
    } on FinanceArgumentError catch (e) {
      if (mounted) setState(() => _error = e);
    } on FinanceStateError catch (e) {
      if (mounted) setState(() => _error = e);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final adding = widget.editing == null;
    final noAssets = adding && widget.assets.isEmpty;
    return AlertDialog(
      title: Text(adding ? l.watchlistsAddAsset : widget.editing!.asset.ticker),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (noAssets)
              Text(l.watchlistsNoAssets)
            else ...[
              if (adding)
                DropdownButtonFormField<int>(
                  key: const Key('watchlist-asset-field'),
                  initialValue: _assetId,
                  isExpanded: true,
                  decoration: InputDecoration(labelText: l.watchlistsAssetField),
                  items: [
                    for (final a in widget.assets)
                      DropdownMenuItem(
                        value: a.id,
                        child: Text('${a.ticker} — ${a.name}', overflow: TextOverflow.ellipsis),
                      ),
                  ],
                  onChanged: (id) => setState(() => _assetId = id),
                ),
              TextField(
                key: const Key('watchlist-target-field'),
                controller: _target,
                keyboardType: const TextInputType.numberWithOptions(decimal: true),
                decoration: InputDecoration(labelText: l.watchlistsTargetField),
              ),
              TextField(
                key: const Key('watchlist-notes-field'),
                controller: _notes,
                decoration: InputDecoration(labelText: l.watchlistsNotesField),
              ),
            ],
            if (_error != null)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  financeErrorText(l, _error!),
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ),
          ],
        ),
      ),
      actions: [
        TextButton(onPressed: () => Navigator.of(context).pop(false), child: Text(l.actionCancel)),
        if (!noAssets)
          FilledButton(
            key: const Key('watchlist-item-confirm-button'),
            onPressed: adding && _assetId == null ? null : _submit,
            child: Text(l.actionSave),
          ),
      ],
    );
  }
}
