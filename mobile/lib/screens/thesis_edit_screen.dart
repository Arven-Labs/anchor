import 'package:flutter/material.dart';

import '../domain/finance_error.dart';
import '../l10n/app_localizations.dart';
import '../l10n/finance_labels.dart';
import '../models/asset.dart';
import '../models/thesis.dart';
import '../services/portfolio_repository.dart';
import '../services/thesis_repository.dart';

/// Cria ou edita uma tese (título, ativo opcional, texto em Markdown). Devolve `true` ao
/// fechar quando algo mudou (salvou ou apagou), para a lista recarregar.
class ThesisEditScreen extends StatefulWidget {
  final ThesisRepository repository;

  /// `null` = tese nova.
  final ThesisEntry? entry;

  const ThesisEditScreen({super.key, required this.repository, this.entry});

  @override
  State<ThesisEditScreen> createState() => _ThesisEditScreenState();
}

class _ThesisEditScreenState extends State<ThesisEditScreen> {
  late final TextEditingController _title = TextEditingController(text: widget.entry?.thesis.title ?? '');
  late final TextEditingController _content = TextEditingController(
    text: widget.entry?.thesis.contentMarkdown ?? '',
  );
  int? _assetId;
  List<Asset> _assets = [];
  Object? _error;

  @override
  void initState() {
    super.initState();
    _assetId = widget.entry?.asset?.id;
    _loadAssets();
  }

  @override
  void dispose() {
    _title.dispose();
    _content.dispose();
    super.dispose();
  }

  Future<void> _loadAssets() async {
    final assets = await PortfolioRepository().listAssets();
    if (mounted) setState(() => _assets = assets);
  }

  Future<void> _save() async {
    final entry = widget.entry;
    try {
      if (entry == null) {
        await widget.repository.create(title: _title.text, content: _content.text, assetId: _assetId);
      } else {
        await widget.repository.update(entry.thesis.id, title: _title.text, content: _content.text, assetId: _assetId);
      }
    } on FinanceArgumentError catch (e) {
      if (mounted) setState(() => _error = e);
      return;
    } on FinanceStateError catch (e) {
      if (mounted) setState(() => _error = e);
      return;
    }
    if (mounted) Navigator.of(context).pop(true);
  }

  Future<void> _delete() async {
    final entry = widget.entry;
    if (entry == null) return;
    final l = AppLocalizations.of(context);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.thesisDeleteTitle),
        content: Text(l.thesisDeleteBody),
        actions: [
          TextButton(onPressed: () => Navigator.of(context).pop(false), child: Text(l.actionCancel)),
          FilledButton(
            key: const Key('thesis-delete-confirm-button'),
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l.actionDelete),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    await widget.repository.delete(entry.thesis.id);
    if (mounted) Navigator.of(context).pop(true);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final adding = widget.entry == null;
    return Scaffold(
      appBar: AppBar(
        title: Text(adding ? l.thesesNew : l.thesisEditTitle),
        actions: [
          if (!adding)
            IconButton(
              key: const Key('thesis-delete-button'),
              icon: const Icon(Icons.delete_outline),
              tooltip: l.actionDelete,
              onPressed: _delete,
            ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          TextField(
            key: const Key('thesis-title-field'),
            controller: _title,
            decoration: InputDecoration(labelText: l.thesisFieldTitle),
          ),
          const SizedBox(height: 16),
          // `initialValue` só vale na construção: reconstruir o campo quando a lista de ativos
          // chega faz uma tese já ligada a um ativo mostrar o ativo, não "Global".
          KeyedSubtree(
            key: ValueKey(_assets.length),
            child: DropdownButtonFormField<int?>(
              key: const Key('thesis-asset-field'),
              initialValue: _assets.any((a) => a.id == _assetId) ? _assetId : null,
              isExpanded: true,
              decoration: InputDecoration(labelText: l.thesisFieldAsset),
              items: [
                DropdownMenuItem<int?>(value: null, child: Text(l.thesisAssetGlobal)),
                for (final a in _assets)
                  DropdownMenuItem<int?>(
                    value: a.id,
                    child: Text('${a.ticker} — ${a.name}', overflow: TextOverflow.ellipsis),
                  ),
              ],
              onChanged: (id) => setState(() => _assetId = id),
            ),
          ),
          const SizedBox(height: 16),
          TextField(
            key: const Key('thesis-content-field'),
            controller: _content,
            minLines: 8,
            maxLines: null,
            keyboardType: TextInputType.multiline,
            decoration: InputDecoration(labelText: l.thesisFieldContent, alignLabelWithHint: true),
          ),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: Text(financeErrorText(l, _error!), style: TextStyle(color: Theme.of(context).colorScheme.error)),
            ),
          const SizedBox(height: 16),
          FilledButton(
            key: const Key('thesis-save-button'),
            onPressed: _save,
            child: Text(l.actionSave),
          ),
        ],
      ),
    );
  }
}
