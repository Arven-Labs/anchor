import 'dart:convert';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';

import '../domain/transactions_csv.dart';
import '../domain/transactions_csv_import.dart';
import '../models/asset.dart';
import '../models/custodia.dart';
import '../models/portfolio_transaction.dart';
import '../services/portfolio_repository.dart';
import 'add_transaction_screen.dart';

/// Somente leitura — `aporte`/`retirada`/`provento` não aparecem em nenhuma
/// posição calculada (ver `PortfolioRepository.computePositions`), então
/// sem esta tela ficariam gravados no banco sem lugar nenhum pra aparecer.
class TransactionHistoryScreen extends StatefulWidget {
  const TransactionHistoryScreen({super.key});

  @override
  State<TransactionHistoryScreen> createState() =>
      _TransactionHistoryScreenState();
}

class _TransactionHistoryScreenState extends State<TransactionHistoryScreen> {
  final _repository = PortfolioRepository();

  bool _loading = true;
  List<PortfolioTransaction> _transactions = [];
  Map<int, Asset> _assetsById = {};
  Map<int, Custodia> _custodiasById = {};

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);

    final transactions = await _repository.listAllTransactions();
    final assets = await _repository.listAssets();
    final custodias = await _repository.listCustodias();

    if (mounted) {
      setState(() {
        _transactions = transactions;
        _assetsById = {for (final a in assets) a.id!: a};
        _custodiasById = {for (final c in custodias) c.id!: c};
        _loading = false;
      });
    }
  }

  Future<void> _edit(PortfolioTransaction tx) async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(builder: (_) => AddTransactionScreen(transaction: tx)),
    );
    if (saved == true) _load();
  }

  Future<void> _delete(int id) async {
    await _repository.deleteTransaction(id);
    await _load();
  }

  /// Salva as transações num CSV no formato "Anchor CSV" do desktop, que importa
  /// pela fila de revisão (só acrescenta, nunca apaga nada lá).
  Future<void> _exportCsv() async {
    final messenger = ScaffoldMessenger.of(context);
    final day = DateTime.now().toIso8601String().substring(0, 10);
    final result = transactionsCsv(
      _assetsById.values.toList(),
      _custodiasById.values.toList(),
      _transactions,
    );
    try {
      final path = await FilePicker.platform.saveFile(
        dialogTitle: 'Salvar transações para o desktop',
        fileName: 'anchor-transactions-$day.csv',
        type: FileType.custom,
        allowedExtensions: const ['csv'],
        bytes: utf8.encode(result.csv),
      );
      if (path == null) return;
      final skipped = result.skipped == 0
          ? ''
          : ' ${result.skipped} ficaram de fora (transferência, amortização ou ativo removido).';
      messenger.showSnackBar(
        SnackBar(content: Text('${result.exported} linhas salvas.$skipped')),
      );
    } catch (e) {
      messenger.showSnackBar(
        SnackBar(content: Text('Não consegui salvar o CSV: $e')),
      );
    }
  }

  /// Lê um CSV "Anchor CSV" exportado pelo desktop, mostra o que entraria (novas,
  /// duplicadas, erros) e só grava depois da confirmação. Só acrescenta.
  Future<void> _importCsv() async {
    final messenger = ScaffoldMessenger.of(context);
    try {
      final picked = await FilePicker.platform.pickFiles(
        type: FileType.custom,
        allowedExtensions: const ['csv'],
        withData: true,
      );
      final bytes = picked?.files.firstOrNull?.bytes;
      if (bytes == null) return;

      final plan = planTransactionsImport(
        parseTransactionsCsv(utf8.decode(bytes, allowMalformed: true)),
        assets: await _repository.listAssets(),
        custodias: await _repository.listCustodias(),
        transactions: await _repository.listAllTransactions(),
      );
      if (!mounted) return;
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (_) => _ImportPreviewDialog(plan: plan),
      );
      if (confirmed != true) return;

      final written = await _repository.applyTransactionsImport(plan);
      await _load();
      messenger.showSnackBar(
        SnackBar(content: Text('$written transações importadas.')),
      );
    } catch (e) {
      messenger.showSnackBar(
        SnackBar(content: Text('Não consegui importar o CSV: $e')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Histórico'),
        actions: [
          IconButton(
            icon: const Icon(Icons.file_download_outlined),
            tooltip: 'Importar CSV do desktop',
            onPressed: _importCsv,
          ),
          IconButton(
            icon: const Icon(Icons.ios_share),
            tooltip: 'Exportar CSV para o desktop',
            onPressed: _transactions.isEmpty ? null : _exportCsv,
          ),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _transactions.isEmpty
          ? const Center(child: Text('Nenhuma transação lançada ainda.'))
          : RefreshIndicator(
              onRefresh: _load,
              child: ListView.builder(
                padding: const EdgeInsets.all(16),
                itemCount: _transactions.length,
                itemBuilder: (context, index) {
                  final tx = _transactions[index];
                  final ticker = tx.assetId != null
                      ? _assetsById[tx.assetId]?.ticker
                      : null;
                  return _TransactionTile(
                    transaction: tx,
                    ticker: ticker,
                    custodiasById: _custodiasById,
                    onTap: () => _edit(tx),
                    onDelete: () => _delete(tx.id!),
                  );
                },
              ),
            ),
    );
  }
}

class _ImportPreviewDialog extends StatelessWidget {
  final ImportPlan plan;

  const _ImportPreviewDialog({required this.plan});

  @override
  Widget build(BuildContext context) {
    final lines = <String>[
      '${plan.rows.length} transações novas',
      if (plan.newAssets.isNotEmpty) '${plan.newAssets.length} ativos novos',
      if (plan.newCustodias.isNotEmpty)
        '${plan.newCustodias.length} custódias novas',
      if (plan.duplicates > 0) '${plan.duplicates} já existem (ignoradas)',
      if (plan.skipped > 0)
        '${plan.skipped} transferências ignoradas (o CSV não traz o destino)',
    ];
    final shownIssues = plan.issues.take(5).toList();

    return AlertDialog(
      title: const Text('Importar do desktop'),
      content: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final line in lines) Text(line),
            if (plan.issues.isNotEmpty) ...[
              const SizedBox(height: 12),
              Text(
                '${plan.issues.length} linhas com problema (não entram):',
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
              for (final issue in shownIssues)
                Text(
                  issue.line == 0
                      ? issue.message
                      : 'linha ${issue.line}: ${issue.message}',
                ),
              if (plan.issues.length > shownIssues.length)
                Text('… e mais ${plan.issues.length - shownIssues.length}'),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: const Text('Cancelar'),
        ),
        FilledButton(
          onPressed: plan.rows.isEmpty
              ? null
              : () => Navigator.of(context).pop(true),
          child: const Text('Importar'),
        ),
      ],
    );
  }
}

class _TransactionTile extends StatelessWidget {
  final PortfolioTransaction transaction;
  final String? ticker;
  final Map<int, Custodia> custodiasById;
  final VoidCallback onTap;
  final VoidCallback onDelete;

  const _TransactionTile({
    required this.transaction,
    required this.ticker,
    required this.custodiasById,
    required this.onTap,
    required this.onDelete,
  });

  String? get _custodiaLabel {
    final origin = custodiasById[transaction.custodiaId]?.label;
    if (!transaction.type.needsTransferDestination) return origin;

    final destination = custodiasById[transaction.transferToCustodiaId]?.label;
    return '${origin ?? '—'} → ${destination ?? '—'}';
  }

  @override
  Widget build(BuildContext context) {
    final date = transaction.date;
    final dateLabel =
        '${date.day.toString().padLeft(2, '0')}/'
        '${date.month.toString().padLeft(2, '0')}/${date.year}';
    final paidOn = transaction.foreignTaxPaidOn;
    final paidLabel = paidOn == null
        ? ''
        : ' (pago em ${paidOn.day.toString().padLeft(2, '0')}/'
              '${paidOn.month.toString().padLeft(2, '0')}/${paidOn.year})';
    final quantity = transaction.quantity;
    final custodiaLabel = _custodiaLabel;

    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: ListTile(
        title: Text('${transaction.type.label} — ${ticker ?? '—'}'),
        subtitle: Text(
          '$dateLabel'
          '${quantity != null ? ' · ${quantity.toStringAsFixed(quantity == quantity.roundToDouble() ? 0 : 2)} un.' : ''}'
          '${custodiaLabel != null ? ' · $custodiaLabel' : ''}'
          ' · ${transaction.totalValue.toStringAsFixed(2)}'
          '${transaction.fee != null && transaction.fee! > 0 ? ' · taxa ${transaction.fee!.toStringAsFixed(2)}' : ''}'
          '${transaction.foreignTaxWithheld != null && transaction.foreignTaxWithheld! > 0 ? ' · retido ${transaction.foreignTaxWithheld!.toStringAsFixed(2)}$paidLabel' : ''}'
          '${transaction.lossCreditUsed != null ? ' · crédito usado ${transaction.lossCreditUsed!.toStringAsFixed(2)}' : ''}',
        ),
        onTap: onTap,
        trailing: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            IconButton(icon: const Icon(Icons.edit_outlined), onPressed: onTap),
            IconButton(
              icon: const Icon(Icons.delete_outline),
              onPressed: onDelete,
            ),
          ],
        ),
      ),
    );
  }
}
