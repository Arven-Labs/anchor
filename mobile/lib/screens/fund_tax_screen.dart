import 'package:flutter/material.dart';

import '../domain/come_cotas.dart';
import '../domain/fund_compensation.dart' show LossCredit;
import '../domain/fund_redemption.dart';
import '../domain/fund_tax.dart';
import '../models/fund.dart';
import '../services/fund_quota_sync.dart';
import '../services/portfolio_repository.dart';
import 'crypto_tax_screen.dart' show formatBrl, formatDay;

/// Situação do come-cotas de um fundo numa frase.
@visibleForTesting
String comeCotasStatusText(ComeCotasEstimate estimate) {
  switch (estimate.status) {
    case EstimateStatus.noPosition:
      return 'Sem cotas na carteira.';
    case EstimateStatus.noTerm:
      return 'Escolha o tipo do fundo para estimar o come-cotas e o IR do resgate.';
    case EstimateStatus.noQuota:
      return 'Lance o valor da cota para estimar o come-cotas.';
    case EstimateStatus.exempt:
      return 'Fundo de ações ou entidade de investimento: sem come-cotas.';
    case EstimateStatus.estimated:
      return 'Próximo come-cotas em ${formatDay(estimate.referenceDate)}: '
          '${formatBrl(estimate.tax)} '
          '(${estimate.quotasDeducted.toStringAsFixed(4)} cotas)';
  }
}

/// Como a base do ganho foi escolhida; vazio fora da estimativa.
@visibleForTesting
String comeCotasBasisText(ComeCotasEstimate estimate) {
  final basis = estimate.basisPerQuota;
  if (estimate.status != EstimateStatus.estimated || basis == null) return '';
  final origin = switch (estimate.basisSource) {
    BasisSource.lastComeCotas => 'valor da cota no último come-cotas',
    BasisSource.estoque => 'custo médio mais o estoque de 2023 já tributado',
    _ => 'custo médio',
  };
  return 'Base por cota ${formatBrl(basis)} ($origin) · ganho '
      '${formatBrl(estimate.gain)} × ${(estimate.rate * 100).toStringAsFixed(1)}%';
}

/// O estoque de 2023 (Lei 14.754, arts. 27 e 28) numa frase; vazio quando o fundo não
/// tem estoque.
@visibleForTesting
String estoqueText(FundReport report) {
  final estoque = report.estoque;
  switch (report.estoqueStatus) {
    case EstoqueStatus.notApplicable:
      return '';
    case EstoqueStatus.notTaxed:
      return 'Diga como o estoque de 2023 foi tributado para estimar o imposto '
          'e o custo do come-cotas.';
    case EstoqueStatus.needsQuota:
      return 'Lance a cota de 31/12/2023 (ou de até 10 dias antes) para estimar '
          'o estoque.';
    case EstoqueStatus.estimated:
      if (estoque == null) return '';
      return 'Estoque de 2023: ganho de ${formatBrl(estoque.base)} até '
          '${formatDay(estoque.date)} (cota ${formatBrl(estoque.quotaValue)}) a '
          '${(estoque.rate * 100).toStringAsFixed(0)}% = '
          '${formatBrl(estoque.tax)}';
  }
}

/// O come-cotas pago acima do IR devido, somado, como possível restituição; vazio sem
/// excesso. A regra (PER/DCOMP, 5 anos do pagamento) não foi confirmada para o
/// come-cotas.
@visibleForTesting
String refundableText(FundReport report) {
  final total = report.refundableComeCotas;
  if (total <= 0.005) return '';
  return 'Come-cotas pago acima do IR devido, possivelmente restituível: '
      '${formatBrl(total)}. O imposto retido a maior pode ser pedido por '
      'PER/DCOMP em até 5 anos da data do pagamento; essa regra não foi '
      'confirmada para o come-cotas, então confira com a administradora ou um '
      'contador. Não está incluído nos líquidos.';
}

/// Avisos do cartão do fundo, em frases.
@visibleForTesting
List<String> fundWarnings(FundReport report) {
  final estimate = report.estimate;
  return [
    if (estimate.staleQuota)
      'O último valor de cota tem mais de $staleQuotaDays dias: a estimativa '
          'pode estar velha.',
    if (estimate.lastReferenceUnrecorded != null)
      'O come-cotas de ${formatDay(estimate.lastReferenceUnrecorded!)} já '
          'passou sem desconto registrado: lance a transação Come-cotas para a '
          'base ficar certa.',
    for (final redemption in report.redemptions) ...[
      if (redemption.uncoveredQuotas > 1e-9)
        'Resgate de ${formatDay(redemption.date)} maior que as aplicações '
            'lançadas: o custo considera só a parte conhecida.',
      if (redemption.excessComeCotas > 0.005)
        'No resgate de ${formatDay(redemption.date)} o come-cotas pago passou '
            'do IR devido em ${formatBrl(redemption.excessComeCotas)}: fica '
            'fora do líquido (veja a possível restituição).',
    ],
  ];
}

/// Quanto do ganho da estimativa o crédito de prejuízo já abateu; vazio sem crédito.
@visibleForTesting
String comeCotasCreditText(ComeCotasEstimate estimate) {
  if (estimate.lossCreditUsed <= 0.005) return '';
  return 'Crédito de prejuízo de ${formatBrl(estimate.lossCreditUsed)} já '
      'reduziu o ganho: o imposto seria '
      '${formatBrl(estimate.taxBeforeCompensation)} sem ele.';
}

/// O que o resgate tem de compensação, uma frase por linha; vazio sem prejuízo nem
/// crédito.
@visibleForTesting
List<String> redemptionCompensationLines(RedemptionEstimate redemption) {
  return [
    if (redemption.lossCreditUsed > 0.005)
      'Prejuízo de resgates anteriores do grupo abateu '
          '${formatBrl(redemption.lossCreditUsed)} do ganho: IR '
          '${formatBrl(redemption.ir)} em vez de '
          '${formatBrl(redemption.irBeforeCompensation)}.',
    if (redemption.lossCreated > 0.005)
      'Este resgate perdeu ${formatBrl(redemption.lossCreated)}: compensa o '
          'ganho de resgates e come-cotas posteriores do mesmo grupo '
          '(mesma administradora e mesmo tipo).',
  ];
}

/// Uma linha por grupo com prejuízo ainda a compensar.
@visibleForTesting
String lossCreditText(LossCredit credit) =>
    '${formatBrl(credit.amount)} — ${credit.tickers.join(', ')} '
    '(${credit.administrator ?? 'sem administradora'}, ${credit.term.label})';

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

/// Aviso antes de baixar: o informe da CVM é um zip de ~12 MB por mês fechado (medido
/// em setembro/2026; o mês corrente, parcial, é menor).
@visibleForTesting
const cvmDownloadWarning =
    'Baixa da CVM um arquivo de até ~12 MB por mês lido (em geral 4 meses, '
    'uns 40 MB). Continuar?';

/// Estado da leitura da CVM de um fundo numa frase; vazio se não há o que dizer.
@visibleForTesting
String cvmStatusText(CvmStatus? status, DateTime? lastCvmDate) {
  final latest = lastCvmDate == null
      ? ''
      : ' Última cota da CVM: ${formatDay(lastCvmDate)}.';
  if (status == null) {
    return lastCvmDate == null ? '' : 'Cotas lidas da CVM.$latest';
  }
  switch (status.kind) {
    case CvmStatusKind.noCnpj:
      return 'Informe o CNPJ do fundo para ler a cota da CVM.';
    case CvmStatusKind.pending:
      return 'Ainda não lida.$latest';
    case CvmStatusKind.ok:
      return 'Cotas lidas da CVM.$latest';
    case CvmStatusKind.notFound:
      return 'A CVM não trouxe cota para este CNPJ (ou subclasse) nos meses '
          'lidos. Confira o CNPJ.';
    case CvmStatusKind.needsSubclass:
      return 'Este CNPJ só tem subclasses no informe: escolha uma.';
    case CvmStatusKind.error:
      return 'Não foi possível ler a CVM agora (${status.message ?? 'erro'}). '
          'Tente de novo em instantes.$latest';
  }
}

class _FundTaxScreenState extends State<FundTaxScreen> {
  final _repository = PortfolioRepository();
  late final FundQuotaSync _sync = widget.sync ?? FundQuotaSync();

  bool _loading = true;
  bool _refreshing = false;
  String? _progress;
  List<FundReport> _reports = [];
  List<LossCredit> _lossCredits = [];
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
      _subclasses = subclasses;
      _loading = false;
    });
  }

  void _say(String message) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(message)));
  }

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
      _say(e.message);
      return;
    }
    _statuses.remove(report.asset.id);
    await _load();
  }

  Future<void> _setAdministrator(FundReport report, String administrator) async {
    try {
      await _repository.setFundAdministrator(report.asset.id!, administrator);
    } on FormatException catch (e) {
      _say(e.message);
      return;
    }
    await _load();
  }

  Future<void> _setStructure(FundReport report, FundStructure structure) async {
    try {
      await _repository.setFundStructure(report.asset.id!, structure);
    } on FormatException catch (e) {
      _say(e.message);
      return;
    }
    await _load();
  }

  Future<void> _setEstoque(FundReport report, EstoqueOption? option) async {
    try {
      await _repository.setFundEstoque(report.asset.id!, option);
    } on FormatException catch (e) {
      _say(e.message);
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
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Atualizar da CVM'),
        content: const Text(cvmDownloadWarning),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancelar'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Baixar'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    setState(() {
      _refreshing = true;
      _progress = 'Preparando…';
    });
    try {
      final statuses = await _sync.refresh(
        onProgress: (done, total) {
          if (mounted) setState(() => _progress = 'Lendo mês $done de $total…');
        },
      );
      _statuses = statuses;
    } catch (e) {
      _say('Não foi possível ler a CVM: $e');
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
    return Scaffold(
      appBar: AppBar(title: const Text('IR de fundos')),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                Text(
                  'Estimativa com as transações e as cotas lançadas neste app. '
                  'O come-cotas e o IR do resgate saem do fundo na fonte: não há '
                  'DARF. As regras vêm de fontes secundárias; confira na '
                  'Receita e no extrato do fundo.',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                const SizedBox(height: 12),
                if (_reports.any((r) => r.asset.cnpj != null)) ...[
                  FilledButton.tonalIcon(
                    onPressed: _refreshing ? null : _refreshCvm,
                    icon: const Icon(Icons.cloud_download),
                    label: const Text('Atualizar da CVM'),
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
                    'Prejuízo a compensar em ganhos e come-cotas posteriores '
                    '(mesma administradora e mesmo tipo)',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  for (final credit in _lossCredits) Text(lossCreditText(credit)),
                  const SizedBox(height: 12),
                ],
                if (_reports.isEmpty)
                  const Padding(
                    padding: EdgeInsets.only(top: 24),
                    child: Center(child: Text('Nenhum fundo cadastrado ainda.')),
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
    final estimate = report.estimate;
    final basis = comeCotasBasisText(estimate);
    final cvmDates = [
      for (final q in report.quotas)
        if (q.source == 'cvm') q.date,
    ];
    final lastCvm = cvmDates.isEmpty
        ? null
        : cvmDates.reduce((a, b) => a.isAfter(b) ? a : b);
    final cvmText = cvmStatusText(cvmStatus, lastCvm);
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
              decoration: const InputDecoration(labelText: 'Tipo do fundo'),
              items: FundTerm.values
                  .map((t) => DropdownMenuItem(value: t, child: Text(t.label)))
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
                decoration: const InputDecoration(
                  labelText: 'Estrutura do fundo (Lei 14.754)',
                ),
                items: FundStructure.values
                    .map(
                      (s) => DropdownMenuItem(value: s, child: Text(s.label)),
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
                  decoration: const InputDecoration(
                    labelText: 'Estoque de 2023 (arts. 27 e 28)',
                  ),
                  items: [
                    const DropdownMenuItem<EstoqueOption?>(
                      value: null,
                      child: Text('Não tributado / não informado'),
                    ),
                    for (final option in EstoqueOption.values)
                      DropdownMenuItem<EstoqueOption?>(
                        value: option,
                        child: Text(option.label),
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
                decoration: const InputDecoration(
                  labelText: 'Subclasse da CVM',
                ),
                items: [
                  const DropdownMenuItem<String?>(
                    value: null,
                    child: Text('Classe (sem subclasse)'),
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
              comeCotasStatusText(estimate),
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
            if (basis.isNotEmpty) Text(basis, style: textTheme.bodySmall),
            if (estoqueText(report).isNotEmpty)
              Text(estoqueText(report), style: textTheme.bodySmall),
            if (refundableText(report).isNotEmpty)
              Text(refundableText(report), style: textTheme.bodySmall),
            if (comeCotasCreditText(estimate).isNotEmpty)
              Text(comeCotasCreditText(estimate), style: textTheme.bodySmall),
            for (final warning in fundWarnings(report))
              Padding(
                padding: const EdgeInsets.only(top: 6),
                child: Text(warning, style: TextStyle(color: error)),
              ),
            ExpansionTile(
              tilePadding: EdgeInsets.zero,
              title: Text(
                'Valor da cota (${report.quotas.length})',
                style: textTheme.bodyMedium,
              ),
              children: [
                for (final quota in report.quotas)
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    title: Text(
                      '${formatDay(quota.date)} — ${formatBrl(quota.value)}'
                      '${quota.source == 'cvm' ? ' · CVM' : ''}',
                    ),
                    trailing: IconButton(
                      icon: const Icon(Icons.delete_outline),
                      tooltip: 'Apagar cota',
                      onPressed: () => onDeleteQuota(quota),
                    ),
                  ),
                Align(
                  alignment: Alignment.centerLeft,
                  child: TextButton.icon(
                    onPressed: onAddQuota,
                    icon: const Icon(Icons.add),
                    label: const Text('Lançar cota'),
                  ),
                ),
              ],
            ),
            if (report.deductions.isNotEmpty)
              ExpansionTile(
                tilePadding: EdgeInsets.zero,
                title: Text(
                  'Come-cotas registrados (${report.deductions.length})',
                  style: textTheme.bodyMedium,
                ),
                children: [
                  for (final tx in report.deductions)
                    ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      title: Text(formatDay(tx.date)),
                      subtitle: Text(
                        '${tx.quantity} cotas a ${formatBrl(tx.unitPrice ?? 0)}'
                        ' · imposto ${formatBrl(tx.totalValue)}',
                      ),
                    ),
                ],
              ),
            if (report.redemptions.isNotEmpty)
              ExpansionTile(
                tilePadding: EdgeInsets.zero,
                title: Text(
                  'Resgates e amortizações estimados (${report.redemptions.length})',
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
    return ListTile(
      dense: true,
      contentPadding: EdgeInsets.zero,
      title: Text(
        redemption.isAmortization
            ? '${formatDay(redemption.date)} — amortização (cota a '
                  '${formatBrl(redemption.quotaValue)}; as cotas ficam)'
            : '${formatDay(redemption.date)} — ${redemption.quotas} cotas a '
                  '${formatBrl(redemption.quotaValue)}',
      ),
      subtitle: Text(
        'Bruto ${formatBrl(redemption.gross)} · IOF ${formatBrl(redemption.iof)}'
        ' · IR ${formatBrl(redemption.ir)} · líquido '
        '${formatBrl(redemption.net)}'
        '${redemption.comeCotasCredit > 0.005 ? '\nCome-cotas já pago abateu ${formatBrl(redemption.comeCotasCredit)} do IR' : ''}'
        '${[for (final line in redemptionCompensationLines(redemption)) '\n$line'].join()}',
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
    return TextField(
      controller: _controller,
      enabled: widget.enabled,
      decoration: InputDecoration(
        labelText: 'Administradora / intermediário (compensa prejuízo)',
        hintText: 'ex.: XP, BTG',
        helperText: !widget.enabled
            ? 'Escolha o tipo do fundo primeiro.'
            : widget.administrator == null
            ? 'Sem isso, o fundo só compensa prejuízo com ele mesmo.'
            : 'Fundos do mesmo tipo e da mesma administradora compensam entre si.',
        helperMaxLines: 2,
        suffixIcon: IconButton(
          icon: const Icon(Icons.check),
          tooltip: 'Salvar administradora',
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
    return TextField(
      key: ValueKey('cnpj-${widget.cnpj}'),
      controller: _controller,
      keyboardType: TextInputType.number,
      decoration: InputDecoration(
        labelText: 'CNPJ do fundo (cota pela CVM)',
        hintText: '00.000.000/0000-00',
        suffixIcon: IconButton(
          icon: const Icon(Icons.check),
          tooltip: 'Salvar CNPJ',
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
    return AlertDialog(
      title: Text('Cota de ${widget.ticker}'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            controller: _valueController,
            autofocus: true,
            keyboardType: const TextInputType.numberWithOptions(decimal: true),
            decoration: const InputDecoration(labelText: 'Valor da cota (R\$)'),
          ),
          ListTile(
            contentPadding: EdgeInsets.zero,
            title: Text('Data: ${formatDay(_date)}'),
            trailing: const Icon(Icons.calendar_today),
            onTap: _pickDate,
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancelar'),
        ),
        FilledButton(onPressed: _confirm, child: const Text('Salvar')),
      ],
    );
  }
}
