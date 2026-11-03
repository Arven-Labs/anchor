import 'package:flutter/material.dart';

import '../domain/fund_compensation.dart' show LossCredit, RecordNote;
import '../domain/fund_redemption.dart';
import '../domain/fund_tax.dart';
import '../domain/tax_format.dart' show formatBrl, formatDay;
import '../l10n/app_localizations.dart';
import '../l10n/fund_tax_labels.dart';
import '../models/fund.dart';
import '../services/fund_quota_sync.dart';
import '../services/portfolio_repository.dart';

/// IR e IOF de fundos (ver `buildFundReports`): cota por data, estimativa do
/// próximo come-cotas e resgates. Estimativa informativa: o imposto sai na fonte
/// e o app não guarda pagamento.
class FundTaxScreen extends StatefulWidget {
  /// Injetável para teste; o padrão lê a CVM de verdade.
  final FundQuotaSync? sync;

  const FundTaxScreen({super.key, this.sync});

  @override
  State<FundTaxScreen> createState() => _FundTaxScreenState();
}

class _FundTaxScreenState extends State<FundTaxScreen> {
  final _repository = PortfolioRepository();
  late final FundQuotaSync _sync = widget.sync ?? FundQuotaSync();

  bool _loading = true;
  bool _refreshing = false;
  String? _progress;
  List<FundReport> _reports = [];
  List<LossCredit> _lossCredits = [];
  List<RecordNote> _recordNotes = [];
  Map<int, String> _subclasses = {};
  Map<int, CvmStatus> _statuses = {};

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final assets = await _repository.listAssets();
    final transactions = await _repository.listAllTransactions();
    final terms = await _repository.listFundTerms();
    final quotas = await _repository.listAllFundQuotas();
    final subclasses = await _repository.listFundSubclasses();
    final administrators = await _repository.listFundAdministrators();
    final structures = await _repository.listFundStructures();
    final estoqueOptions = await _repository.listFundEstoqueOptions();
    if (!mounted) return;
    final result = buildFundReports(
      assets: assets,
      transactions: transactions,
      terms: terms,
      quotas: quotas,
      today: DateTime.now(),
      administrators: administrators,
      structures: structures,
      estoqueOptions: estoqueOptions,
    );
    setState(() {
      _reports = result.reports;
      _lossCredits = result.lossCredits;
      _recordNotes = result.recordNotes;
      _subclasses = subclasses;
      _loading = false;
    });
  }

  void _say(String message) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(message)));
  }

  /// A frase da recusa de uma configuração, no idioma da tela.
  String _settingError(FormatException e) =>
      e is FundSettingException ? fundSettingErrorText(AppLocalizations.of(context), e) : e.message;

  /// Grava o CNPJ e a subclasse; trocar um deles apaga as cotas já lidas da CVM.
  Future<void> _setCvm(FundReport report, {String? cnpj, String? subclass}) async {
    try {
      await _repository.setFundCvm(
        report.asset.id!,
        cnpj: cnpj ?? report.asset.cnpj,
        // Outro CNPJ é outra série: a subclasse antiga não vale mais.
        subclass: cnpj != null ? null : subclass ?? _subclasses[report.asset.id],
      );
    } on FormatException catch (e) {
      _say(_settingError(e));
      return;
    }
    _statuses.remove(report.asset.id);
    await _load();
  }

  Future<void> _setAdministrator(FundReport report, String administrator) async {
    try {
      await _repository.setFundAdministrator(report.asset.id!, administrator);
    } on FormatException catch (e) {
      _say(_settingError(e));
      return;
    }
    await _load();
  }

  Future<void> _setStructure(FundReport report, FundStructure structure) async {
    try {
      await _repository.setFundStructure(report.asset.id!, structure);
    } on FormatException catch (e) {
      _say(_settingError(e));
      return;
    }
    await _load();
  }

  Future<void> _setEstoque(FundReport report, EstoqueOption? option) async {
    try {
      await _repository.setFundEstoque(report.asset.id!, option);
    } on FormatException catch (e) {
      _say(_settingError(e));
      return;
    }
    await _load();
  }

  Future<void> _clearSubclass(FundReport report) async {
    await _repository.setFundCvm(report.asset.id!, cnpj: report.asset.cnpj);
    _statuses.remove(report.asset.id);
    await _load();
  }

  Future<void> _refreshCvm() async {
    final l = AppLocalizations.of(context);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.fundRefreshCvm),
        content: Text(l.fundCvmDownloadWarning),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l.actionCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l.fundDownload),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    setState(() {
      _refreshing = true;
      _progress = l.fundProgressPreparing;
    });
    try {
      final statuses = await _sync.refresh(
        onProgress: (done, total) {
          if (mounted) setState(() => _progress = l.fundProgressReading(done, total));
        },
      );
      _statuses = statuses;
    } catch (e) {
      _say(l.fundCvmReadFailed('$e'));
    }
    if (!mounted) return;
    setState(() {
      _refreshing = false;
      _progress = null;
    });
    await _load();
  }

  Future<void> _setTerm(FundReport report, FundTerm term) async {
    await _repository.setFundTerm(report.asset.id!, term);
    await _load();
  }

  Future<void> _addQuota(FundReport report) async {
    final result = await showDialog<({DateTime date, double value})>(
      context: context,
      builder: (_) => _QuotaDialog(ticker: report.asset.ticker),
    );
    if (result == null) return;
    await _repository.setFundQuota(
      assetId: report.asset.id!,
      date: result.date,
      value: result.value,
    );
    await _load();
  }

  Future<void> _deleteQuota(FundQuota quota) async {
    await _repository.deleteFundQuota(quota.id!);
    await _load();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.fundTaxTitle)),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                Text(
                  l.fundTaxDisclaimer,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                const SizedBox(height: 12),
                if (_reports.any((r) => r.asset.cnpj != null)) ...[
                  FilledButton.tonalIcon(
                    onPressed: _refreshing ? null : _refreshCvm,
                    icon: const Icon(Icons.cloud_download),
                    label: Text(l.fundRefreshCvm),
                  ),
                  if (_progress != null) ...[
                    const SizedBox(height: 8),
                    const LinearProgressIndicator(),
                    const SizedBox(height: 4),
                    Text(
                      _progress!,
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ],
                  const SizedBox(height: 12),
                ],
                if (_lossCredits.isNotEmpty) ...[
                  Text(
                    l.fundLossCreditsHeader,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  for (final credit in _lossCredits) Text(lossCreditText(l, credit)),
                  const SizedBox(height: 12),
                ],
                for (final note in _recordNotes) ...[
                  Text(
                    recordNoteText(l, note),
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Colors.orange,
                    ),
                  ),
                  const SizedBox(height: 12),
                ],
                if (_reports.isEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 24),
                    child: Center(child: Text(l.fundNone)),
                  )
                else
                  for (final report in _reports)
                    _FundCard(
                      report: report,
                      subclass: _subclasses[report.asset.id],
                      cvmStatus: _statuses[report.asset.id],
                      onTerm: (term) => _setTerm(report, term),
                      onStructure: (value) => _setStructure(report, value),
                      onEstoque: (value) => _setEstoque(report, value),
                      onAdministrator: (text) =>
                          _setAdministrator(report, text),
                      onCnpj: (cnpj) => _setCvm(report, cnpj: cnpj),
                      onSubclass: (subclass) => subclass == null
                          ? _clearSubclass(report)
                          : _setCvm(report, subclass: subclass),
                      onAddQuota: () => _addQuota(report),
                      onDeleteQuota: _deleteQuota,
                    ),
              ],
            ),
    );
  }
}

class _FundCard extends StatelessWidget {
  final FundReport report;
  final String? subclass;
  final CvmStatus? cvmStatus;
  final ValueChanged<FundTerm> onTerm;
  final ValueChanged<FundStructure> onStructure;
  final ValueChanged<EstoqueOption?> onEstoque;
  final ValueChanged<String> onAdministrator;
  final ValueChanged<String> onCnpj;
  final ValueChanged<String?> onSubclass;
  final VoidCallback onAddQuota;
  final ValueChanged<FundQuota> onDeleteQuota;

  const _FundCard({
    required this.report,
    required this.subclass,
    required this.cvmStatus,
    required this.onTerm,
    required this.onStructure,
    required this.onEstoque,
    required this.onAdministrator,
    required this.onCnpj,
    required this.onSubclass,
    required this.onAddQuota,
    required this.onDeleteQuota,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final error = Theme.of(context).colorScheme.error;
    final l = AppLocalizations.of(context);
    final locale = l.localeName;
    String brl(double value) => formatBrl(value, locale: locale);
    final estimate = report.estimate;
    final basis = comeCotasBasisText(l, estimate);
    final estoque = estoqueText(l, report);
    final refundable = refundableText(l, report);
    final creditText = comeCotasCreditText(l, estimate);
    final cvmDates = [
      for (final q in report.quotas)
        if (q.source == 'cvm') q.date,
    ];
    final lastCvm = cvmDates.isEmpty
        ? null
        : cvmDates.reduce((a, b) => a.isAfter(b) ? a : b);
    final cvmText = cvmStatusText(l, cvmStatus, lastCvm);
    final options = cvmStatus?.kind == CvmStatusKind.needsSubclass
        ? cvmStatus!.options
        : [?subclass];

    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(report.asset.ticker, style: textTheme.titleMedium),
            Text(report.asset.name, style: textTheme.bodySmall),
            const SizedBox(height: 12),
            DropdownButtonFormField<FundTerm>(
              initialValue: report.term,
              decoration: InputDecoration(labelText: l.fundTermLabel),
              items: FundTerm.values
                  .map(
                    (t) => DropdownMenuItem(value: t, child: Text(fundTermName(l, t))),
                  )
                  .toList(),
              onChanged: (value) {
                if (value != null) onTerm(value);
              },
            ),
            if (report.term != null) ...[
              const SizedBox(height: 12),
              DropdownButtonFormField<FundStructure>(
                key: ValueKey('structure-${report.structure}'),
                initialValue: report.structure,
                decoration: InputDecoration(labelText: l.fundStructureLabel),
                items: FundStructure.values
                    .map(
                      (s) => DropdownMenuItem(
                        value: s,
                        child: Text(fundStructureName(l, s)),
                      ),
                    )
                    .toList(),
                onChanged: (value) {
                  if (value != null) onStructure(value);
                },
              ),
              if (report.structure != FundStructure.open &&
                  report.estoqueStatus != EstoqueStatus.notApplicable) ...[
                const SizedBox(height: 12),
                DropdownButtonFormField<EstoqueOption?>(
                  key: ValueKey('estoque-${report.estoqueOption}'),
                  initialValue: report.estoqueOption,
                  decoration: InputDecoration(labelText: l.fundEstoqueLabel),
                  items: [
                    DropdownMenuItem<EstoqueOption?>(
                      value: null,
                      child: Text(l.fundEstoqueNotInformed),
                    ),
                    for (final option in EstoqueOption.values)
                      DropdownMenuItem<EstoqueOption?>(
                        value: option,
                        child: Text(estoqueOptionName(l, option)),
                      ),
                  ],
                  onChanged: onEstoque,
                ),
              ],
            ],
            const SizedBox(height: 12),
            _AdministratorField(
              // Remonta quando o texto gravado ou o tipo mudam: o campo é estado local.
              key: ValueKey('administrator-${report.administrator}-${report.term}'),
              administrator: report.administrator,
              enabled: report.term != null,
              onSave: onAdministrator,
            ),
            const SizedBox(height: 12),
            _CvmCnpjField(cnpj: report.asset.cnpj, onSave: onCnpj),
            if (options.isNotEmpty) ...[
              const SizedBox(height: 12),
              DropdownButtonFormField<String?>(
                key: ValueKey('subclass-$subclass-${options.length}'),
                initialValue: subclass,
                decoration: InputDecoration(labelText: l.fundSubclassLabel),
                items: [
                  DropdownMenuItem<String?>(
                    value: null,
                    child: Text(l.fundSubclassNone),
                  ),
                  for (final option in options)
                    DropdownMenuItem<String?>(value: option, child: Text(option)),
                ],
                onChanged: onSubclass,
              ),
            ],
            if (cvmText.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 6),
                child: Text(
                  cvmText,
                  style: cvmStatus?.kind == CvmStatusKind.error ||
                          cvmStatus?.kind == CvmStatusKind.notFound
                      ? TextStyle(color: error)
                      : textTheme.bodySmall,
                ),
              ),
            const SizedBox(height: 12),
            Text(
              comeCotasStatusText(l, estimate),
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
            if (basis.isNotEmpty) Text(basis, style: textTheme.bodySmall),
            if (estoque.isNotEmpty) Text(estoque, style: textTheme.bodySmall),
            if (refundable.isNotEmpty) Text(refundable, style: textTheme.bodySmall),
            if (creditText.isNotEmpty) Text(creditText, style: textTheme.bodySmall),
            for (final warning in fundWarnings(l, report))
              Padding(
                padding: const EdgeInsets.only(top: 6),
                child: Text(warning, style: TextStyle(color: error)),
              ),
            ExpansionTile(
              tilePadding: EdgeInsets.zero,
              title: Text(
                l.fundQuotasTitle(report.quotas.length),
                style: textTheme.bodyMedium,
              ),
              children: [
                for (final quota in report.quotas)
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    title: Text(
                      '${formatDay(quota.date, locale: locale)} — ${brl(quota.value)}'
                      '${quota.source == 'cvm' ? ' · CVM' : ''}',
                    ),
                    trailing: IconButton(
                      icon: const Icon(Icons.delete_outline),
                      tooltip: l.fundDeleteQuota,
                      onPressed: () => onDeleteQuota(quota),
                    ),
                  ),
                Align(
                  alignment: Alignment.centerLeft,
                  child: TextButton.icon(
                    onPressed: onAddQuota,
                    icon: const Icon(Icons.add),
                    label: Text(l.fundAddQuota),
                  ),
                ),
              ],
            ),
            if (report.deductions.isNotEmpty)
              ExpansionTile(
                tilePadding: EdgeInsets.zero,
                title: Text(
                  l.fundDeductionsTitle(report.deductions.length),
                  style: textTheme.bodyMedium,
                ),
                children: [
                  for (final tx in report.deductions)
                    ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      title: Text(formatDay(tx.date, locale: locale)),
                      subtitle: Text(
                        l.fundDeductionLine(
                          '${tx.quantity}',
                          brl(tx.unitPrice ?? 0),
                          brl(tx.totalValue),
                          tx.lossCreditUsed != null
                              ? l.fundDeductionCredit(brl(tx.lossCreditUsed!))
                              : '',
                        ),
                      ),
                    ),
                ],
              ),
            if (report.redemptions.isNotEmpty)
              ExpansionTile(
                tilePadding: EdgeInsets.zero,
                title: Text(
                  l.fundRedemptionsTitle(report.redemptions.length),
                  style: textTheme.bodyMedium,
                ),
                children: [
                  for (final redemption in report.redemptions.reversed)
                    _RedemptionTile(redemption: redemption),
                ],
              ),
          ],
        ),
      ),
    );
  }
}

class _RedemptionTile extends StatelessWidget {
  final RedemptionEstimate redemption;

  const _RedemptionTile({required this.redemption});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final locale = l.localeName;
    String brl(double value) => formatBrl(value, locale: locale);
    final date = formatDay(redemption.date, locale: locale);
    return ListTile(
      dense: true,
      contentPadding: EdgeInsets.zero,
      title: Text(
        redemption.isAmortization
            ? l.fundRedemptionAmortization(date, brl(redemption.quotaValue))
            : l.fundRedemptionTitle(
                date,
                '${redemption.quotas}',
                brl(redemption.quotaValue),
              ),
      ),
      subtitle: Text(
        [
          l.fundRedemptionDetail(
            brl(redemption.gross),
            brl(redemption.iof),
            brl(redemption.ir),
            brl(redemption.net),
          ),
          if (redemption.comeCotasCredit > 0.005)
            l.fundRedemptionComeCotasCredit(brl(redemption.comeCotasCredit)),
          ...redemptionCompensationLines(l, redemption),
        ].join('\n'),
      ),
    );
  }
}

/// Administradora (ou intermediário) do fundo, para agrupar quem compensa prejuízo com
/// quem. O texto mora num controller criado na montagem; o cartão remonta (`key`)
/// quando o gravado muda.
class _AdministratorField extends StatefulWidget {
  final String? administrator;
  final bool enabled;
  final ValueChanged<String> onSave;

  const _AdministratorField({
    super.key,
    required this.administrator,
    required this.enabled,
    required this.onSave,
  });

  @override
  State<_AdministratorField> createState() => _AdministratorFieldState();
}

class _AdministratorFieldState extends State<_AdministratorField> {
  late final _controller = TextEditingController(
    text: widget.administrator ?? '',
  );

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return TextField(
      controller: _controller,
      enabled: widget.enabled,
      decoration: InputDecoration(
        labelText: l.fundAdministratorLabel,
        hintText: l.fundAdministratorHint,
        helperText: !widget.enabled
            ? l.fundAdministratorHelpNoTerm
            : widget.administrator == null
            ? l.fundAdministratorHelpNone
            : l.fundAdministratorHelpSet,
        helperMaxLines: 2,
        suffixIcon: IconButton(
          icon: const Icon(Icons.check),
          tooltip: l.fundSaveAdministrator,
          onPressed: widget.enabled
              ? () => widget.onSave(_controller.text)
              : null,
        ),
      ),
      onSubmitted: widget.enabled ? widget.onSave : null,
    );
  }
}

/// CNPJ do fundo para a leitura da CVM. O texto mora num controller criado na
/// montagem; o cartão remonta (`key`) quando o CNPJ gravado muda, senão o campo
/// ficaria com o valor antigo.
class _CvmCnpjField extends StatefulWidget {
  final String? cnpj;
  final ValueChanged<String> onSave;

  const _CvmCnpjField({required this.cnpj, required this.onSave});

  @override
  State<_CvmCnpjField> createState() => _CvmCnpjFieldState();
}

class _CvmCnpjFieldState extends State<_CvmCnpjField> {
  late final _controller = TextEditingController(text: widget.cnpj ?? '');

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return TextField(
      key: ValueKey('cnpj-${widget.cnpj}'),
      controller: _controller,
      keyboardType: TextInputType.number,
      decoration: InputDecoration(
        labelText: l.fundCnpjLabel,
        hintText: '00.000.000/0000-00',
        suffixIcon: IconButton(
          icon: const Icon(Icons.check),
          tooltip: l.fundSaveCnpj,
          onPressed: () => widget.onSave(_controller.text),
        ),
      ),
      onSubmitted: widget.onSave,
    );
  }
}

/// Data e valor de uma cota nova.
class _QuotaDialog extends StatefulWidget {
  final String ticker;

  const _QuotaDialog({required this.ticker});

  @override
  State<_QuotaDialog> createState() => _QuotaDialogState();
}

class _QuotaDialogState extends State<_QuotaDialog> {
  final _valueController = TextEditingController();
  DateTime _date = DateTime.now();

  @override
  void dispose() {
    _valueController.dispose();
    super.dispose();
  }

  Future<void> _pickDate() async {
    final picked = await showDatePicker(
      context: context,
      initialDate: _date,
      firstDate: DateTime(2000),
      lastDate: DateTime.now(),
    );
    if (picked != null) setState(() => _date = picked);
  }

  void _confirm() {
    final value = double.tryParse(_valueController.text.replaceAll(',', '.'));
    if (value == null || !value.isFinite || value <= 0) return;
    Navigator.of(context).pop((date: _date, value: value));
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return AlertDialog(
      title: Text(l.fundQuotaDialogTitle(widget.ticker)),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            controller: _valueController,
            autofocus: true,
            keyboardType: const TextInputType.numberWithOptions(decimal: true),
            decoration: InputDecoration(labelText: l.fundQuotaValueLabel),
          ),
          ListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l.fundDate(formatDay(_date, locale: l.localeName))),
            trailing: const Icon(Icons.calendar_today),
            onTap: _pickDate,
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l.actionCancel),
        ),
        FilledButton(onPressed: _confirm, child: Text(l.actionSave)),
      ],
    );
  }
}
