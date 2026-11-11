import 'package:material_ui/material_ui.dart';

import '../domain/general_finance.dart';
import '../domain/generated_tx_info.dart';
import '../domain/money_format.dart';
import '../l10n/app_localizations.dart';
import '../l10n/finance_labels.dart';
import '../models/bank_account.dart';
import '../models/general_transaction.dart';
import '../models/general_transaction_category.dart';
import '../services/general_finance_repository.dart';

/// Lançamentos de uma conta: lista do mais novo ao mais antigo, receita e
/// despesa à mão, exclusão. O saldo no topo ignora lançamento de data futura
/// (mesma regra do desktop).
class FinancasTransactionsScreen extends StatefulWidget {
  final BankAccount account;

  const FinancasTransactionsScreen({super.key, required this.account});

  @override
  State<FinancasTransactionsScreen> createState() =>
      _FinancasTransactionsScreenState();
}

class _FinancasTransactionsScreenState
    extends State<FinancasTransactionsScreen> {
  final _repository = GeneralFinanceRepository();

  bool _loading = true;
  List<GeneralTransaction> _transactions = [];
  List<GeneralTransactionCategory> _categories = [];
  Map<int, GeneratedTxInfo> _generated = {};

  BankAccount get _account => widget.account;

  /// A observação do lançamento: a que o usuário digitou (ou a gravada antes do ADR 0001) tem
  /// prioridade; sem ela, a frase derivada da dívida ou do empréstimo que o gerou.
  String? _noteText(AppLocalizations l, GeneralTransaction tx) {
    if (tx.notes != null) return tx.notes;
    final info = _generated[tx.id];
    return info == null ? null : generatedTxText(l, info);
  }

  String? _categoryName(int? id) {
    for (final c in _categories) {
      if (c.id == id) return c.nome;
    }
    return null;
  }

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final transactions = await _repository.listTransactions(
      accountId: _account.id,
    );
    final categories = await _repository.listCategories();
    final generated = await _repository.generatedTransactionInfo();
    if (!mounted) return;
    setState(() {
      _transactions = transactions;
      _categories = categories;
      _generated = generated;
      _loading = false;
    });
  }

  double get _balance =>
      computeBalances(_transactions, toIsoDay(DateTime.now()))[_account.id] ??
      0;

  Future<void> _add() async {
    final l = AppLocalizations.of(context);
    final valorController = TextEditingController();
    final notesController = TextEditingController();
    var type = GeneralTransactionType.despesa;
    int? categoryId;
    var date = DateTime.now();

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: Text(l.txNewTitle),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              SegmentedButton<GeneralTransactionType>(
                segments: [
                  for (final t in GeneralTransactionType.manual)
                    ButtonSegment(value: t, label: Text(transactionTypeName(l, t))),
                ],
                selected: {type},
                onSelectionChanged: (selection) =>
                    setDialogState(() => type = selection.first),
              ),
              if (type == GeneralTransactionType.despesa &&
                  _categories.isNotEmpty) ...[
                const SizedBox(height: 12),
                DropdownButtonFormField<int?>(
                  key: const Key('financas-tx-category'),
                  initialValue: categoryId,
                  decoration: InputDecoration(labelText: l.txCategory),
                  items: [
                    DropdownMenuItem(
                      value: null,
                      child: Text(l.txNoCategory),
                    ),
                    for (final c in _categories)
                      DropdownMenuItem(value: c.id, child: Text(c.nome)),
                  ],
                  onChanged: (value) =>
                      setDialogState(() => categoryId = value),
                ),
              ],
              const SizedBox(height: 12),
              TextField(
                controller: valorController,
                keyboardType: const TextInputType.numberWithOptions(
                  decimal: true,
                ),
                decoration: InputDecoration(
                  labelText: l.finAmount(_account.currency),
                ),
              ),
              const SizedBox(height: 12),
              OutlinedButton.icon(
                icon: const Icon(Icons.calendar_today_outlined),
                label: Text(formatReadingDate(toIsoDay(date), locale: l.localeName)),
                onPressed: () async {
                  final picked = await showDatePicker(
                    context: context,
                    initialDate: date,
                    firstDate: DateTime(2000),
                    lastDate: DateTime(2100),
                  );
                  if (picked != null) setDialogState(() => date = picked);
                },
              ),
              const SizedBox(height: 12),
              TextField(
                controller: notesController,
                decoration: InputDecoration(labelText: l.finNotes),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l.actionCancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l.actionSave),
            ),
          ],
        ),
      ),
    );

    final valor = parseAmount(valorController.text);
    final notes = notesController.text.trim();
    valorController.dispose();
    notesController.dispose();
    if (saved != true || !mounted) return;

    final messenger = ScaffoldMessenger.of(context);
    if (valor == null) {
      messenger.showSnackBar(
        SnackBar(content: Text(l.txAmountRequired)),
      );
      return;
    }
    await _repository.insertTransaction(
      GeneralTransaction(
        bankAccountId: _account.id!,
        type: type,
        categoryId: type == GeneralTransactionType.despesa ? categoryId : null,
        valor: valor,
        date: toIsoDay(date),
        notes: notes.isEmpty ? null : notes,
        createdAt: DateTime.now(),
      ),
    );
    await _load();
  }

  Future<void> _delete(GeneralTransaction tx) async {
    final l = AppLocalizations.of(context);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.txDeleteTitle),
        content: Text(
          l.txDeleteBody(
                transactionTypeName(l, tx.type),
                formatMoney(tx.valor, _account.currency, locale: l.localeName),
                formatReadingDate(tx.date, locale: l.localeName),
              ) +
              (tx.transferId == null ? '' : '\n${l.txDeleteTransferLeg}'),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l.actionCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l.actionDelete),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    await _repository.deleteTransaction(tx.id!);
    await _load();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final today = toIsoDay(DateTime.now());
    return Scaffold(
      appBar: AppBar(title: Text(_account.label)),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _add,
        icon: const Icon(Icons.add),
        label: Text(l.txFab),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.all(16),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        l.txBalance,
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                      Text(
                        formatMoney(_balance, _account.currency, locale: l.localeName),
                        key: const Key('financas-account-balance'),
                        style: Theme.of(context).textTheme.headlineSmall,
                      ),
                    ],
                  ),
                ),
                Expanded(
                  child: _transactions.isEmpty
                      ? Center(child: Text(l.txEmpty))
                      : ListView.builder(
                          itemCount: _transactions.length,
                          itemBuilder: (context, index) {
                            final tx = _transactions[index];
                            final future = tx.date.compareTo(today) > 0;
                            final signed = signedValor(tx.type, tx.valor);
                            final note = _noteText(l, tx);
                            return Card(
                              margin: const EdgeInsets.fromLTRB(16, 0, 16, 8),
                              child: ListTile(
                                title: Text(
                                  formatMoney(signed, _account.currency, locale: l.localeName),
                                ),
                                subtitle: Text(
                                  [
                                    [
                                      transactionTypeName(l, tx.type),
                                      ?_categoryName(tx.categoryId),
                                      formatReadingDate(tx.date, locale: l.localeName),
                                    ].join(' · '),
                                    if (future) l.txFuture,
                                    ?note,
                                  ].join('\n'),
                                ),
                                isThreeLine: future || note != null,
                                // Parcela de dívida e lançamento de empréstimo
                                // saem junto com o cadastro que os gerou.
                                trailing: tx.type.isGenerated
                                    ? null
                                    : IconButton(
                                        icon: const Icon(Icons.delete_outline),
                                        onPressed: () => _delete(tx),
                                      ),
                              ),
                            );
                          },
                        ),
                ),
              ],
            ),
    );
  }
}
