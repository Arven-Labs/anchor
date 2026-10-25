import 'package:flutter/material.dart';

import '../domain/general_finance.dart';
import '../domain/money_format.dart';
import '../models/bank_account.dart';
import '../models/general_transaction.dart';
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

  BankAccount get _account => widget.account;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final transactions = await _repository.listTransactions(
      accountId: _account.id,
    );
    if (!mounted) return;
    setState(() {
      _transactions = transactions;
      _loading = false;
    });
  }

  double get _balance =>
      computeBalances(_transactions, toIsoDay(DateTime.now()))[_account.id] ??
      0;

  Future<void> _add() async {
    final valorController = TextEditingController();
    final notesController = TextEditingController();
    var type = GeneralTransactionType.despesa;
    var date = DateTime.now();

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: const Text('Novo lançamento'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              SegmentedButton<GeneralTransactionType>(
                segments: [
                  for (final t in GeneralTransactionType.values)
                    ButtonSegment(value: t, label: Text(t.label)),
                ],
                selected: {type},
                onSelectionChanged: (selection) =>
                    setDialogState(() => type = selection.first),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: valorController,
                keyboardType: const TextInputType.numberWithOptions(
                  decimal: true,
                ),
                decoration: InputDecoration(
                  labelText: 'Valor (${_account.currency})',
                ),
              ),
              const SizedBox(height: 12),
              OutlinedButton.icon(
                icon: const Icon(Icons.calendar_today_outlined),
                label: Text(formatIsoDay(toIsoDay(date))),
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
                decoration: const InputDecoration(labelText: 'Observação'),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: const Text('Cancelar'),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: const Text('Salvar'),
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
        const SnackBar(content: Text('Informe um valor maior que zero.')),
      );
      return;
    }
    await _repository.insertTransaction(
      GeneralTransaction(
        bankAccountId: _account.id!,
        type: type,
        valor: valor,
        date: toIsoDay(date),
        notes: notes.isEmpty ? null : notes,
        createdAt: DateTime.now(),
      ),
    );
    await _load();
  }

  Future<void> _delete(GeneralTransaction tx) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Excluir lançamento?'),
        content: Text(
          '${tx.type.label} de ${formatMoney(tx.valor, _account.currency)} '
          'em ${formatIsoDay(tx.date)}.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancelar'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Excluir'),
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
    final today = toIsoDay(DateTime.now());
    return Scaffold(
      appBar: AppBar(title: Text(_account.label)),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _add,
        icon: const Icon(Icons.add),
        label: const Text('Lançamento'),
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
                        'Saldo',
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                      Text(
                        formatMoney(_balance, _account.currency),
                        key: const Key('financas-account-balance'),
                        style: Theme.of(context).textTheme.headlineSmall,
                      ),
                    ],
                  ),
                ),
                Expanded(
                  child: _transactions.isEmpty
                      ? const Center(
                          child: Text('Nenhum lançamento nesta conta ainda.'),
                        )
                      : ListView.builder(
                          itemCount: _transactions.length,
                          itemBuilder: (context, index) {
                            final tx = _transactions[index];
                            final future = tx.date.compareTo(today) > 0;
                            final signed = signedValor(tx.type, tx.valor);
                            return Card(
                              margin: const EdgeInsets.fromLTRB(16, 0, 16, 8),
                              child: ListTile(
                                title: Text(
                                  formatMoney(signed, _account.currency),
                                ),
                                subtitle: Text(
                                  [
                                    '${tx.type.label} · '
                                        '${formatIsoDay(tx.date)}',
                                    if (future) 'ainda não entra no saldo',
                                    if (tx.notes != null) tx.notes!,
                                  ].join('\n'),
                                ),
                                isThreeLine: future || tx.notes != null,
                                trailing: IconButton(
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
