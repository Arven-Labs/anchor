import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:open_filex/open_filex.dart';

import '../domain/file_size.dart';
import '../domain/finance_error.dart';
import '../l10n/app_localizations.dart';
import '../l10n/finance_labels.dart';
import '../models/asset.dart';
import '../models/thesis.dart';
import '../models/thesis_attachment.dart';
import '../services/portfolio_repository.dart';
import '../services/thesis_attachment_repository.dart';
import '../services/thesis_repository.dart';

/// Escolhe um arquivo no aparelho; devolve o caminho, ou `null` se o usuário cancelou.
Future<String?> _pickFileWithPlatform() async => (await FilePicker.platform.pickFiles())?.files.single.path;

/// Abre o arquivo no app padrão do sistema; devolve a mensagem de erro, ou `null` se abriu.
Future<String?> _openFileWithPlatform(String path) async {
  final result = await OpenFilex.open(path);
  return result.type == ResultType.done ? null : result.message;
}

/// Cria ou edita uma tese (título, ativo opcional, texto em Markdown). Devolve `true` ao
/// fechar quando algo mudou (salvou ou apagou), para a lista recarregar.
class ThesisEditScreen extends StatefulWidget {
  final ThesisRepository repository;

  /// `null` = tese nova.
  final ThesisEntry? entry;

  /// Anexos, seletor de arquivo e abertura de arquivo; os padrões usam o disco e os plugins
  /// do aparelho. Injetáveis para teste (os plugins não rodam no `flutter_test`).
  final ThesisAttachmentRepository? attachments;
  final Future<String?> Function()? pickFile;
  final Future<String?> Function(String path)? openFile;

  /// Vindo da busca de cotação: o ticker fica fixo (sem seletor de ativo). [forAsset] é o ativo
  /// cadastrado com esse ticker e classe; `null` quando o ticker não está na carteira, caso em
  /// que a tese é salva Global e **nenhum ativo é criado** (um ativo novo viraria card na Carteira).
  final String? forTicker;
  final Asset? forAsset;

  const ThesisEditScreen({
    super.key,
    required this.repository,
    this.entry,
    this.attachments,
    this.pickFile,
    this.openFile,
    this.forTicker,
    this.forAsset,
  });

  @override
  State<ThesisEditScreen> createState() => _ThesisEditScreenState();
}

class _ThesisEditScreenState extends State<ThesisEditScreen> {
  late final TextEditingController _title = TextEditingController(text: widget.entry?.thesis.title ?? '');
  late final TextEditingController _content = TextEditingController(
    text: widget.entry?.thesis.contentMarkdown ?? '',
  );
  late final ThesisAttachmentRepository _attachmentRepository = widget.attachments ?? ThesisAttachmentRepository();
  int? _assetId;
  List<Asset> _assets = [];
  List<ThesisAttachment> _attachments = [];
  bool _addingAttachment = false;
  Object? _error;

  @override
  void initState() {
    super.initState();
    _assetId = widget.entry?.asset?.id;
    _loadAssets();
    _loadAttachments();
  }

  Future<void> _loadAttachments() async {
    final entry = widget.entry;
    if (entry == null) return;
    final attachments = await _attachmentRepository.list(entry.thesis.id);
    if (mounted) setState(() => _attachments = attachments);
  }

  Future<void> _addAttachment() async {
    final entry = widget.entry;
    if (entry == null) return;
    final path = await (widget.pickFile ?? _pickFileWithPlatform)();
    if (path == null) return;
    setState(() {
      _addingAttachment = true;
      _error = null;
    });
    try {
      await _attachmentRepository.add(entry.thesis.id, path);
    } on FinanceArgumentError catch (e) {
      if (mounted) setState(() => _error = e);
    } finally {
      if (mounted) setState(() => _addingAttachment = false);
    }
    await _loadAttachments();
  }

  Future<void> _openAttachment(ThesisAttachment attachment) async {
    final path = await _attachmentRepository.absolutePath(attachment);
    final message = await (widget.openFile ?? _openFileWithPlatform)(path);
    if (message != null && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(message)));
    }
  }

  Future<void> _deleteAttachment(ThesisAttachment attachment) async {
    try {
      await _attachmentRepository.delete(attachment);
    } on FinanceStateError catch (e) {
      if (mounted) setState(() => _error = e);
    }
    await _loadAttachments();
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

  /// Com o ticker fixo (busca de cotação), o ativo é o da busca, ou nenhum; senão, o escolhido.
  int? get _effectiveAssetId => widget.forTicker != null ? widget.forAsset?.id : _assetId;

  Future<void> _save() async {
    final entry = widget.entry;
    final assetId = _effectiveAssetId;
    try {
      if (entry == null) {
        await widget.repository.create(title: _title.text, content: _content.text, assetId: assetId);
      } else {
        await widget.repository.update(entry.thesis.id, title: _title.text, content: _content.text, assetId: assetId);
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
    try {
      await widget.repository.delete(entry.thesis.id);
    } on FinanceStateError catch (e) {
      // Um arquivo anexado não saiu do aparelho: a tese continua e o erro aparece aqui.
      if (mounted) setState(() => _error = e);
      return;
    }
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
          if (widget.forTicker != null)
            Text(
              widget.forAsset != null
                  ? l.thesisAssetFixed(widget.forTicker!)
                  : l.thesisAssetNotInPortfolio(widget.forTicker!),
              key: const Key('thesis-fixed-asset'),
              style: Theme.of(context).textTheme.bodyMedium,
            )
          else
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
          // O anexo precisa do id da tese: em tese nova, salva primeiro e depois anexa.
          if (!adding) ...[
            const Divider(height: 32),
            Column(
              key: const Key('thesis-attachments-section'),
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(l.madAttachments, style: Theme.of(context).textTheme.titleSmall),
                const SizedBox(height: 12),
                if (_attachments.isEmpty)
                  Text(l.madNoAttachments)
                else
                  for (final attachment in _attachments)
                    Card(
                      margin: const EdgeInsets.only(bottom: 8),
                      child: ListTile(
                        onTap: () => _openAttachment(attachment),
                        title: Text(attachment.originalFileName),
                        subtitle: Text(formatFileSize(attachment.fileSizeBytes, l.localeName)),
                        trailing: IconButton(
                          key: Key('thesis-attachment-delete-${attachment.id}'),
                          icon: const Icon(Icons.delete_outline),
                          tooltip: l.actionDelete,
                          onPressed: () => _deleteAttachment(attachment),
                        ),
                      ),
                    ),
                const SizedBox(height: 12),
                FilledButton.icon(
                  key: const Key('thesis-add-attachment-button'),
                  onPressed: _addingAttachment ? null : _addAttachment,
                  icon: const Icon(Icons.attach_file),
                  label: Text(l.madAddAttachment),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}
