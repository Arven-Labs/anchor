import 'package:material_ui/material_ui.dart';

import '../domain/general_finance.dart' show toIsoDay;
import '../l10n/app_localizations.dart';
import '../models/asset.dart';
import '../models/thesis.dart';
import '../services/thesis_repository.dart';
import 'thesis_edit_screen.dart';

/// As teses do ticker buscado, dentro da busca de cotação (equivalente ao
/// `AssetThesesSidebar` do desktop). Lista só as ligadas ao ativo desse ticker **e** classe;
/// com o ticker fora da carteira a lista é vazia e a tese nova sai Global, sem criar ativo
/// (um ativo novo viraria card "0 unidades" na Carteira, que não filtra posição zerada).
class AssetThesesSection extends StatefulWidget {
  final String ticker;
  final AssetClass assetClass;

  /// Repositório; o padrão usa o banco do app. Injetável para teste (relógio fixo).
  final ThesisRepository? repository;

  const AssetThesesSection({super.key, required this.ticker, required this.assetClass, this.repository});

  @override
  State<AssetThesesSection> createState() => _AssetThesesSectionState();
}

class _AssetThesesSectionState extends State<AssetThesesSection> {
  late final ThesisRepository _repository = widget.repository ?? ThesisRepository();

  Asset? _asset;
  List<ThesisEntry> _entries = [];

  /// Antes da 1ª leitura `_asset` é `null` por falta de resposta, não por o ativo não existir:
  /// sem este sinal, "Nova tese" salvaria Global para um ativo que está na carteira.
  bool _loaded = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final asset = await _repository.findAsset(ticker: widget.ticker, assetClass: widget.assetClass);
    final entries = await _repository.listFor(ticker: widget.ticker, assetClass: widget.assetClass);
    if (!mounted) return;
    setState(() {
      _asset = asset;
      _entries = entries;
      _loaded = true;
    });
  }

  Future<void> _open({ThesisEntry? entry}) async {
    final changed = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => ThesisEditScreen(
          repository: _repository,
          entry: entry,
          forTicker: widget.ticker,
          forAsset: _asset,
        ),
      ),
    );
    if (changed == true) await _load();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Column(
      key: const Key('asset-theses-section'),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(child: Text(l.thesesTitle, style: Theme.of(context).textTheme.titleMedium)),
            OutlinedButton(
              key: const Key('asset-theses-new-button'),
              onPressed: _loaded ? () => _open() : null,
              child: Text(l.thesesNew),
            ),
          ],
        ),
        const SizedBox(height: 8),
        if (!_loaded)
          const SizedBox.shrink()
        else if (_entries.isEmpty)
          Text(l.assetThesesEmpty(widget.ticker))
        else
          for (final entry in _entries)
            Card(
              child: ListTile(
                key: Key('asset-thesis-tile-${entry.thesis.id}'),
                title: Text(entry.thesis.title),
                subtitle: Text(toIsoDay(entry.thesis.updatedAt)),
                onTap: () => _open(entry: entry),
              ),
            ),
      ],
    );
  }
}
