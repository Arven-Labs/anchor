import 'package:flutter/material.dart';

import '../domain/come_cotas.dart';
import '../domain/fund_redemption.dart';
import '../domain/fund_tax.dart';
import '../models/fund.dart';
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
      return 'Fundo de ações: sem come-cotas.';
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
  final origin = estimate.basisSource == BasisSource.lastComeCotas
      ? 'valor da cota no último come-cotas'
      : 'custo médio';
  return 'Base por cota ${formatBrl(basis)} ($origin) · ganho '
      '${formatBrl(estimate.gain)} × ${(estimate.rate * 100).toStringAsFixed(1)}%';
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
            'do IR devido em ${formatBrl(redemption.excessComeCotas)}: a '
            'compensação não é modelada.',
    ],
  ];
}

/// IR e IOF de fundos (ver `buildFundReports`): cota por data, estimativa do
/// próximo come-cotas e resgates. Estimativa informativa: o imposto sai na fonte
/// e o app não guarda pagamento.
class FundTaxScreen extends StatefulWidget {
  const FundTaxScreen({super.key});

  @override
  State<FundTaxScreen> createState() => _FundTaxScreenState();
}

class _FundTaxScreenState extends State<FundTaxScreen> {
  final _repository = PortfolioRepository();

  bool _loading = true;
  List<FundReport> _reports = [];

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
    if (!mounted) return;
    setState(() {
      _reports = buildFundReports(
        assets: assets,
        transactions: transactions,
        terms: terms,
        quotas: quotas,
        today: DateTime.now(),
      );
      _loading = false;
    });
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
                if (_reports.isEmpty)
                  const Padding(
                    padding: EdgeInsets.only(top: 24),
                    child: Center(child: Text('Nenhum fundo cadastrado ainda.')),
                  )
                else
                  for (final report in _reports)
                    _FundCard(
                      report: report,
                      onTerm: (term) => _setTerm(report, term),
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
  final ValueChanged<FundTerm> onTerm;
  final VoidCallback onAddQuota;
  final ValueChanged<FundQuota> onDeleteQuota;

  const _FundCard({
    required this.report,
    required this.onTerm,
    required this.onAddQuota,
    required this.onDeleteQuota,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final error = Theme.of(context).colorScheme.error;
    final estimate = report.estimate;
    final basis = comeCotasBasisText(estimate);

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
            const SizedBox(height: 12),
            Text(
              comeCotasStatusText(estimate),
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
            if (basis.isNotEmpty) Text(basis, style: textTheme.bodySmall),
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
                      '${formatDay(quota.date)} — ${formatBrl(quota.value)}',
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
                  'Resgates estimados (${report.redemptions.length})',
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
        '${formatDay(redemption.date)} — ${redemption.quotas} cotas a '
        '${formatBrl(redemption.quotaValue)}',
      ),
      subtitle: Text(
        'Bruto ${formatBrl(redemption.gross)} · IOF ${formatBrl(redemption.iof)}'
        ' · IR ${formatBrl(redemption.ir)} · líquido '
        '${formatBrl(redemption.net)}'
        '${redemption.comeCotasCredit > 0.005 ? '\nCome-cotas já pago abateu ${formatBrl(redemption.comeCotasCredit)} do IR' : ''}',
      ),
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
