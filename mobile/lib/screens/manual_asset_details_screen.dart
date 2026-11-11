import 'package:file_picker/file_picker.dart';
import 'package:material_ui/material_ui.dart';
import 'package:open_filex/open_filex.dart';

import '../domain/number_locale.dart' show formatFixed;
import '../domain/tax_format.dart' show formatDay;
import '../l10n/app_localizations.dart';
import '../models/asset.dart';
import '../models/asset_attachment.dart';
import '../models/asset_valuation.dart';
import '../services/portfolio_repository.dart';

String _formatBytes(int bytes, String locale) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${formatFixed(bytes / 1024, 1, locale: locale)} KB';
  return '${formatFixed(bytes / (1024 * 1024), 1, locale: locale)} MB';
}

/// Mirror de `ManualAssetDetails.tsx` do desktop — histórico de avaliação +
/// anexos (escritura, ITBI, cap table). Anexo em disco vive no diretório de
/// documentos do app (`PortfolioRepository.addAssetAttachment`), mesmo
/// espírito do `app_data_dir()` do desktop; abrir usa `open_filex` (app
/// padrão do sistema) em vez do preview inline do desktop.
/// `empresaNaoListada` ganha o bloco de participação societária que `imovel`
/// não tem — ambas compartilham histórico de avaliação e anexos.
class ManualAssetDetailsScreen extends StatefulWidget {
  final Asset asset;

  const ManualAssetDetailsScreen({super.key, required this.asset});

  @override
  State<ManualAssetDetailsScreen> createState() =>
      _ManualAssetDetailsScreenState();
}

class _ManualAssetDetailsScreenState extends State<ManualAssetDetailsScreen> {
  final _repository = PortfolioRepository();

  late Asset _asset;
  late TextEditingController _sharesOwnedController;
  late TextEditingController _totalSharesController;
  late TextEditingController _companyValuationController;

  final _valuationValueController = TextEditingController();
  final _valuationNotesController = TextEditingController();
  DateTime? _valuationDate;

  final _documentTypeController = TextEditingController();

  bool _loading = true;
  bool _savingEquity = false;
  bool _savingValuation = false;
  bool _addingAttachment = false;
  List<AssetValuation> _valuations = [];
  List<AssetAttachment> _attachments = [];

  bool get _isEmpresaNaoListada =>
      _asset.assetClass == AssetClass.empresaNaoListada;

  @override
  void initState() {
    super.initState();
    _asset = widget.asset;
    _sharesOwnedController = TextEditingController(
      text: _asset.equitySharesOwned?.toString() ?? '',
    );
    _totalSharesController = TextEditingController(
      text: _asset.equityTotalShares?.toString() ?? '',
    );
    _companyValuationController = TextEditingController(
      text: _asset.equityCompanyValuation?.toString() ?? '',
    );
    for (final controller in [
      _sharesOwnedController,
      _totalSharesController,
      _companyValuationController,
    ]) {
      controller.addListener(() => setState(() {}));
    }
    _load();
  }

  Future<void> _load() async {
    final valuations = await _repository.listAssetValuations(_asset.id!);
    final attachments = await _repository.listAssetAttachments(_asset.id!);
    if (mounted) {
      setState(() {
        _valuations = valuations;
        _attachments = attachments;
        _loading = false;
      });
    }
  }

  double? _parse(String text) => double.tryParse(text.replaceAll(',', '.'));

  Future<void> _saveEquity() async {
    setState(() => _savingEquity = true);

    final sharesOwned = _parse(_sharesOwnedController.text);
    final totalShares = _parse(_totalSharesController.text);
    final companyValuation = _parse(_companyValuationController.text);

    await _repository.updateAssetEquity(
      _asset.id!,
      sharesOwned: sharesOwned,
      totalShares: totalShares,
      companyValuation: companyValuation,
    );

    setState(() {
      _asset = Asset(
        id: _asset.id,
        ticker: _asset.ticker,
        name: _asset.name,
        assetClass: _asset.assetClass,
        currency: _asset.currency,
        createdAt: _asset.createdAt,
        externalId: _asset.externalId,
        equitySharesOwned: sharesOwned,
        equityTotalShares: totalShares,
        equityCompanyValuation: companyValuation,
      );
      _savingEquity = false;
    });
  }

  Future<void> _pickValuationDate() async {
    final picked = await showDatePicker(
      context: context,
      initialDate: _valuationDate ?? DateTime.now(),
      firstDate: DateTime(2000),
      lastDate: DateTime.now(),
    );
    if (picked != null) setState(() => _valuationDate = picked);
  }

  Future<void> _addValuation() async {
    final date = _valuationDate;
    final value = _parse(_valuationValueController.text);
    if (date == null || value == null || value <= 0) return;

    setState(() => _savingValuation = true);

    await _repository.insertAssetValuation(
      assetId: _asset.id!,
      valuationDate: date,
      value: value,
      notes: _valuationNotesController.text.trim().isEmpty
          ? null
          : _valuationNotesController.text.trim(),
    );

    _valuationDate = null;
    _valuationValueController.clear();
    _valuationNotesController.clear();
    setState(() => _savingValuation = false);
    await _load();
  }

  Future<void> _deleteValuation(int id) async {
    await _repository.deleteAssetValuation(id);
    await _load();
  }

  Future<void> _pickAndAddAttachment() async {
    final result = await FilePicker.platform.pickFiles();
    final path = result?.files.single.path;
    if (path == null) return;

    setState(() => _addingAttachment = true);
    await _repository.addAssetAttachment(
      assetId: _asset.id!,
      sourcePath: path,
      documentType: _documentTypeController.text.trim().isEmpty
          ? null
          : _documentTypeController.text.trim(),
    );
    _documentTypeController.clear();
    setState(() => _addingAttachment = false);
    await _load();
  }

  Future<void> _openAttachment(AssetAttachment attachment) async {
    final path = await _repository.resolveAssetAttachmentPath(
      attachment.storedRelativePath,
    );
    final result = await OpenFilex.open(path);
    if (result.type != ResultType.done && mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(result.message)));
    }
  }

  Future<void> _deleteAttachment(AssetAttachment attachment) async {
    await _repository.deleteAssetAttachment(attachment);
    await _load();
  }

  @override
  void dispose() {
    _sharesOwnedController.dispose();
    _totalSharesController.dispose();
    _companyValuationController.dispose();
    _valuationValueController.dispose();
    _valuationNotesController.dispose();
    _documentTypeController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final sharesOwned = _parse(_sharesOwnedController.text);
    final totalShares = _parse(_totalSharesController.text);
    final companyValuation = _parse(_companyValuationController.text);
    final percentual =
        (sharesOwned != null && totalShares != null && totalShares > 0)
        ? sharesOwned / totalShares
        : null;
    final participationValue = (percentual != null && companyValuation != null)
        ? percentual * companyValuation
        : null;

    final latestValue = _valuations.isEmpty ? null : _valuations.last.value;
    final l = AppLocalizations.of(context);
    final locale = l.localeName;

    return Scaffold(
      appBar: AppBar(
        title: Text(
          _isEmpresaNaoListada ? l.madTitleCompany : l.madTitleProperty,
        ),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                if (_isEmpresaNaoListada) ...[
                  Text(
                    percentual == null
                        ? l.madEquityTitle
                        : participationValue == null
                        ? l.madEquityPct(
                            formatFixed(percentual * 100, 2, locale: locale),
                          )
                        : l.madEquityPctValue(
                            formatFixed(percentual * 100, 2, locale: locale),
                            formatFixed(participationValue, 2, locale: locale),
                            _asset.currency,
                          ),
                    style: Theme.of(context).textTheme.titleSmall,
                  ),
                  const SizedBox(height: 4),
                  Text(
                    l.madEquityNote,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: _sharesOwnedController,
                    keyboardType: const TextInputType.numberWithOptions(
                      decimal: true,
                    ),
                    decoration: InputDecoration(labelText: l.madSharesOwned),
                  ),
                  const SizedBox(height: 16),
                  TextField(
                    controller: _totalSharesController,
                    keyboardType: const TextInputType.numberWithOptions(
                      decimal: true,
                    ),
                    decoration: InputDecoration(labelText: l.madTotalShares),
                  ),
                  const SizedBox(height: 16),
                  TextField(
                    controller: _companyValuationController,
                    keyboardType: const TextInputType.numberWithOptions(
                      decimal: true,
                    ),
                    decoration: InputDecoration(
                      labelText: l.madCompanyValuation(_asset.currency),
                    ),
                  ),
                  const SizedBox(height: 12),
                  FilledButton(
                    onPressed: _savingEquity ? null : _saveEquity,
                    child: Text(l.madSaveEquity),
                  ),
                  const Divider(height: 32),
                ],
                Text(
                  latestValue == null
                      ? l.madHistory
                      : l.madHistoryLatest(
                          formatFixed(latestValue, 2, locale: locale),
                          _asset.currency,
                        ),
                  style: Theme.of(context).textTheme.titleSmall,
                ),
                const SizedBox(height: 12),
                if (_valuations.isEmpty)
                  Text(l.madNoValuations)
                else
                  for (final valuation in _valuations)
                    Card(
                      margin: const EdgeInsets.only(bottom: 8),
                      child: ListTile(
                        title: Text(
                          '${formatFixed(valuation.value, 2, locale: locale)} ${_asset.currency}',
                        ),
                        subtitle: Text(
                          formatDay(valuation.valuationDate, locale: locale) +
                              (valuation.notes != null ? ' · ${valuation.notes}' : ''),
                        ),
                        trailing: IconButton(
                          icon: const Icon(Icons.delete_outline),
                          onPressed: () => _deleteValuation(valuation.id!),
                        ),
                      ),
                    ),
                const SizedBox(height: 16),
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  title: Text(
                    _valuationDate != null
                        ? l.madDate(formatDay(_valuationDate!, locale: locale))
                        : l.madPickDate,
                  ),
                  trailing: const Icon(Icons.calendar_today),
                  onTap: _pickValuationDate,
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: _valuationValueController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: InputDecoration(
                    labelText: l.madValue(_asset.currency),
                  ),
                ),
                const SizedBox(height: 16),
                TextField(
                  controller: _valuationNotesController,
                  decoration: InputDecoration(labelText: l.madNotes),
                ),
                const SizedBox(height: 16),
                FilledButton(
                  onPressed: _savingValuation ? null : _addValuation,
                  child: Text(l.madAddValuation),
                ),
                const Divider(height: 32),
                Text(
                  l.madAttachments,
                  style: Theme.of(context).textTheme.titleSmall,
                ),
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
                        subtitle: Text(
                          '${attachment.documentType ?? "—"} · '
                          '${_formatBytes(attachment.fileSizeBytes, locale)}',
                        ),
                        trailing: IconButton(
                          icon: const Icon(Icons.delete_outline),
                          onPressed: () => _deleteAttachment(attachment),
                        ),
                      ),
                    ),
                const SizedBox(height: 16),
                TextField(
                  controller: _documentTypeController,
                  decoration: InputDecoration(
                    labelText: l.madDocType,
                    hintText: l.madDocTypeHint,
                  ),
                ),
                const SizedBox(height: 16),
                FilledButton.icon(
                  onPressed: _addingAttachment ? null : _pickAndAddAttachment,
                  icon: const Icon(Icons.attach_file),
                  label: Text(l.madAddAttachment),
                ),
              ],
            ),
    );
  }
}
