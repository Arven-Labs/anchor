import 'package:flutter/material.dart';

import '../domain/currency_catalog.dart';
import '../domain/general_finance.dart';
import '../domain/money_format.dart';
import '../models/bank_account.dart';
import '../services/general_finance_repository.dart';
import 'financas_transactions_screen.dart';

/// Finanças Gerais: contas em qualquer moeda e o saldo de cada uma, derivado
/// dos lançamentos. Standalone, como o resto do mobile (sem sync com o
/// desktop). Fora do mobile por ora: categorias, transferências, dívidas e
/// empréstimos.
class FinancasScreen extends StatefulWidget {
  const FinancasScreen({super.key});

  @override
  State<FinancasScreen> createState() => _FinancasScreenState();
}

class _FinancasScreenState extends State<FinancasScreen> {
  final _repository = GeneralFinanceRepository();

  bool _loading = true;
  List<BankAccount> _accounts = [];
  Map<int, double> _balances = {};

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final accounts = await _repository.listAccounts();
    final transactions = await _repository.listTransactions();
    final balances = computeBalances(transactions, toIsoDay(DateTime.now()));
    if (!mounted) return;
    setState(() {
      _accounts = accounts;
      _balances = balances;
      _loading = false;
    });
  }

  /// Cria (`existing == null`) ou edita. A moeda fica travada na conta que já
  /// tem lançamentos: o histórico seria relido em outra moeda.
  Future<void> _editAccount([BankAccount? existing]) async {
    final nomeController = TextEditingController(text: existing?.nome);
    final titularController = TextEditingController(text: existing?.titular);
    var currency = existing?.currency ?? defaultBaseCurrency;
    final currencyLocked =
        existing != null && await _repository.accountInUse(existing.id!);
    if (!mounted) return;

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: Text(existing == null ? 'Nova conta' : 'Editar conta'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                controller: nomeController,
                decoration: const InputDecoration(
                  labelText: 'Nome',
                  hintText: 'Nubank',
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: titularController,
                decoration: const InputDecoration(labelText: 'Titular'),
              ),
              const SizedBox(height: 12),
              DropdownButtonFormField<String>(
                initialValue: currency,
                decoration: InputDecoration(
                  labelText: 'Moeda',
                  helperText: currencyLocked
                      ? 'Não muda numa conta que já tem lançamentos.'
                      : null,
                ),
                items: [
                  for (final (code, name) in currencyCatalog)
                    DropdownMenuItem(value: code, child: Text('$code — $name')),
                ],
                onChanged: currencyLocked
                    ? null
                    : (value) =>
                          setDialogState(() => currency = value ?? currency),
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

    final nome = nomeController.text.trim();
    final titular = titularController.text.trim();
    nomeController.dispose();
    titularController.dispose();
    if (saved != true || nome.isEmpty || titular.isEmpty || !mounted) return;

    final messenger = ScaffoldMessenger.of(context);
    final account = BankAccount(
      id: existing?.id,
      nome: nome,
      titular: titular,
      currency: currency,
      createdAt: existing?.createdAt ?? DateTime.now(),
    );
    try {
      if (existing == null) {
        await _repository.insertAccount(account);
      } else {
        await _repository.updateAccount(account);
      }
    } on StateError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(e.message)));
      return;
    }
    await _load();
  }

  Future<void> _deleteAccount(BankAccount account) async {
    final inUse = await _repository.accountInUse(account.id!);
    if (!mounted) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Excluir ${account.nome}?'),
        content: Text(
          inUse
              ? 'Os lançamentos desta conta também serão apagados.'
              : 'A conta não tem lançamentos.',
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
    await _repository.deleteAccount(account.id!);
    await _load();
  }

  Future<void> _open(BankAccount account) async {
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => FinancasTransactionsScreen(account: account),
      ),
    );
    await _load();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Finanças'),
        actions: [
          IconButton(
            tooltip: 'Atualizar',
            icon: const Icon(Icons.refresh),
            onPressed: _load,
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => _editAccount(),
        icon: const Icon(Icons.add),
        label: const Text('Conta'),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _accounts.isEmpty
          ? const Center(child: Text('Nenhuma conta cadastrada ainda.'))
          : ListView.builder(
              padding: const EdgeInsets.all(16),
              itemCount: _accounts.length,
              itemBuilder: (context, index) {
                final account = _accounts[index];
                final balance = _balances[account.id] ?? 0;
                return Card(
                  margin: const EdgeInsets.only(bottom: 8),
                  child: ListTile(
                    title: Text(account.nome),
                    subtitle: Text(account.titular),
                    onTap: () => _open(account),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          formatMoney(balance, account.currency),
                          key: Key('financas-balance-${account.id}'),
                        ),
                        PopupMenuButton<String>(
                          onSelected: (action) => action == 'edit'
                              ? _editAccount(account)
                              : _deleteAccount(account),
                          itemBuilder: (_) => const [
                            PopupMenuItem(value: 'edit', child: Text('Editar')),
                            PopupMenuItem(
                              value: 'delete',
                              child: Text('Excluir'),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                );
              },
            ),
    );
  }
}
