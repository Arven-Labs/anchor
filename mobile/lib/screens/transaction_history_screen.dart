import 'package:flutter/material.dart';

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

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Histórico')),
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
          '${transaction.foreignTaxWithheld != null && transaction.foreignTaxWithheld! > 0 ? ' · retido ${transaction.foreignTaxWithheld!.toStringAsFixed(2)}' : ''}'
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
