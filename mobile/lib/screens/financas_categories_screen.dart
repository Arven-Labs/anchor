import 'package:flutter/material.dart';

import '../domain/general_finance.dart';
import '../domain/money_format.dart';
import '../models/bank_account.dart';
import '../models/general_transaction_category.dart';
import '../services/general_finance_repository.dart';

/// Categorias de despesa com o limite mensal opcional e o gasto do mês, na
/// moeda base (o limite é nessa moeda, como no desktop). O câmbio vem pronto
/// da tela de Finanças: aqui não há rede.
class FinancasCategoriesScreen extends StatefulWidget {
  final String base;
  final Map<String, double> ratesToBrl;
  final List<BankAccount> accounts;

  /// "Hoje" injetável para o mês do gasto; o padrão é o relógio.
  final DateTime? now;

  const FinancasCategoriesScreen({
    super.key,
    required this.base,
    required this.ratesToBrl,
    required this.accounts,
    this.now,
  });

  @override
  State<FinancasCategoriesScreen> createState() =>
      _FinancasCategoriesScreenState();
}

class _FinancasCategoriesScreenState extends State<FinancasCategoriesScreen> {
  final _repository = GeneralFinanceRepository();

  bool _loading = true;
  List<GeneralTransactionCategory> _categories = [];
  final Map<int, ({double spent, int missingFx})> _spent = {};

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final categories = await _repository.listCategories();
    final transactions = await _repository.listTransactions();
    final month = toIsoDay(widget.now ?? DateTime.now()).substring(0, 7);
    final currencies = {for (final a in widget.accounts) a.id!: a.currency};
    final spent = {
      for (final c in categories)
        c.id!: spentThisMonth(
          transactions,
          categoryId: c.id!,
          monthPrefix: month,
          accountCurrency: currencies,
          base: widget.base,
          ratesToBrl: widget.ratesToBrl,
        ),
    };
    if (!mounted) return;
    setState(() {
      _categories = categories;
      _spent
        ..clear()
        ..addAll(spent);
      _loading = false;
    });
  }

  Future<void> _edit([GeneralTransactionCategory? existing]) async {
    final nomeController = TextEditingController(text: existing?.nome);
    final limitController = TextEditingController(
      text: existing?.limiteMensal?.toString(),
    );

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(existing == null ? 'Nova categoria' : 'Editar categoria'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: nomeController,
              decoration: const InputDecoration(
                labelText: 'Nome',
                hintText: 'Mercado',
              ),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: limitController,
              keyboardType: const TextInputType.numberWithOptions(
                decimal: true,
              ),
              decoration: InputDecoration(
                labelText: 'Limite mensal (${widget.base})',
                helperText: 'Vazio = sem limite.',
              ),
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
    );

    final nome = nomeController.text.trim();
    final limitText = limitController.text.trim();
    nomeController.dispose();
    limitController.dispose();
    if (saved != true || !mounted) return;

    final messenger = ScaffoldMessenger.of(context);
    final limit = limitText.isEmpty ? null : parseAmount(limitText);
    if (nome.isEmpty || (limitText.isNotEmpty && limit == null)) {
      messenger.showSnackBar(
        const SnackBar(
          content: Text('Informe o nome e, se houver, um limite maior que zero.'),
        ),
      );
      return;
    }
    final category = GeneralTransactionCategory(
      id: existing?.id,
      nome: nome,
      limiteMensal: limit,
      createdAt: existing?.createdAt ?? DateTime.now(),
    );
    if (existing == null) {
      await _repository.insertCategory(category);
    } else {
      await _repository.updateCategory(category);
    }
    await _load();
  }

  Future<void> _delete(GeneralTransactionCategory category) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Excluir ${category.nome}?'),
        content: const Text(
          'Os lançamentos dela ficam sem categoria; nenhum é apagado.',
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
    await _repository.deleteCategory(category.id!);
    await _load();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Categorias')),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => _edit(),
        icon: const Icon(Icons.add),
        label: const Text('Categoria'),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _categories.isEmpty
          ? const Center(child: Text('Nenhuma categoria cadastrada ainda.'))
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [for (final c in _categories) _categoryCard(c)],
            ),
    );
  }

  Widget _categoryCard(GeneralTransactionCategory category) {
    final textTheme = Theme.of(context).textTheme;
    final spent = _spent[category.id] ?? (spent: 0.0, missingFx: 0);
    final limit = category.limiteMensal;
    final over = limit != null && spent.spent > limit;
    final missingNote = spent.missingFx == 0
        ? null
        : '${spent.missingFx} despesa(s) fora da soma por falta de câmbio.';
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 4, 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(category.nome, style: textTheme.titleSmall),
                ),
                PopupMenuButton<String>(
                  onSelected: (action) =>
                      action == 'edit' ? _edit(category) : _delete(category),
                  itemBuilder: (_) => const [
                    PopupMenuItem(value: 'edit', child: Text('Editar')),
                    PopupMenuItem(value: 'delete', child: Text('Excluir')),
                  ],
                ),
              ],
            ),
            Padding(
              padding: const EdgeInsets.only(right: 12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    limit == null
                        ? 'Gasto no mês: ${formatMoney(spent.spent, widget.base)}'
                        : '${formatMoney(spent.spent, widget.base)} de '
                              '${formatMoney(limit, widget.base)}',
                    key: Key('financas-category-spent-${category.id}'),
                  ),
                  if (limit != null) ...[
                    const SizedBox(height: 6),
                    LinearProgressIndicator(
                      key: Key('financas-category-bar-${category.id}'),
                      value: (spent.spent / limit).clamp(0.0, 1.0),
                      color: over ? Theme.of(context).colorScheme.error : null,
                    ),
                    if (over)
                      Padding(
                        padding: const EdgeInsets.only(top: 4),
                        child: Text(
                          'Estourou em ${formatMoney(spent.spent - limit, widget.base)}',
                          key: Key('financas-category-over-${category.id}'),
                          style: textTheme.bodySmall?.copyWith(
                            color: Theme.of(context).colorScheme.error,
                          ),
                        ),
                      ),
                  ],
                  if (missingNote != null)
                    Padding(
                      padding: const EdgeInsets.only(top: 4),
                      child: Text(missingNote, style: textTheme.bodySmall),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
