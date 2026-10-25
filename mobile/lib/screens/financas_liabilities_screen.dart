import 'package:flutter/material.dart';

import '../domain/general_finance.dart';
import '../domain/liability.dart';
import '../domain/money_format.dart';
import '../models/bank_account.dart';
import '../models/liability.dart';
import '../services/general_finance_repository.dart';

/// Dívidas com parcelas geradas (Price ou SAC): cadastro, saldo devedor de
/// hoje, parcelas pagas e o cronograma. As parcelas viram lançamentos na
/// conta, todos de uma vez, e o saldo da conta só sente a que já venceu.
class FinancasLiabilitiesScreen extends StatefulWidget {
  final List<BankAccount> accounts;

  /// "Hoje" injetável; o padrão é o relógio.
  final DateTime? now;

  const FinancasLiabilitiesScreen({super.key, required this.accounts, this.now});

  @override
  State<FinancasLiabilitiesScreen> createState() =>
      _FinancasLiabilitiesScreenState();
}

class _FinancasLiabilitiesScreenState extends State<FinancasLiabilitiesScreen> {
  final _repository = GeneralFinanceRepository();

  bool _loading = true;
  List<LiabilityWithInstallments> _liabilities = [];

  String get _today => toIsoDay(widget.now ?? DateTime.now());

  BankAccount? _account(int id) {
    for (final a in widget.accounts) {
      if (a.id == id) return a;
    }
    return null;
  }

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final liabilities = await _repository.listLiabilities();
    if (!mounted) return;
    setState(() {
      _liabilities = liabilities;
      _loading = false;
    });
  }

  /// Taxa digitada: aceita vírgula e o zero (dívida sem juros).
  double? _parseRate(String text) {
    final value = double.tryParse(text.trim().replaceAll(',', '.'));
    return value != null && value.isFinite && value >= 0 ? value : null;
  }

  Future<void> _create() async {
    final messenger = ScaffoldMessenger.of(context);
    if (widget.accounts.isEmpty) {
      messenger.showSnackBar(
        const SnackBar(content: Text('Cadastre uma conta antes da dívida.')),
      );
      return;
    }
    final nomeController = TextEditingController();
    final principalController = TextEditingController();
    final rateController = TextEditingController();
    final termController = TextEditingController();
    var account = widget.accounts.first;
    var system = AmortizationSystem.price;
    var start = widget.now ?? DateTime.now();

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: const Text('Nova dívida'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  key: const Key('financas-liability-name'),
                  controller: nomeController,
                  decoration: const InputDecoration(
                    labelText: 'Nome',
                    hintText: 'Financiamento do carro',
                  ),
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<BankAccount>(
                  initialValue: account,
                  isExpanded: true,
                  decoration: const InputDecoration(labelText: 'Conta'),
                  items: [
                    for (final a in widget.accounts)
                      DropdownMenuItem(value: a, child: Text(a.label)),
                  ],
                  onChanged: (value) =>
                      setDialogState(() => account = value ?? account),
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-liability-principal'),
                  controller: principalController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: InputDecoration(
                    labelText: 'Principal (${account.currency})',
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-liability-rate'),
                  controller: rateController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: const InputDecoration(
                    labelText: 'Taxa (% a.m.)',
                    helperText: '0 para dívida sem juros.',
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-liability-term'),
                  controller: termController,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(labelText: 'Prazo (meses)'),
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<AmortizationSystem>(
                  initialValue: system,
                  isExpanded: true,
                  decoration: const InputDecoration(labelText: 'Sistema'),
                  items: [
                    for (final s in AmortizationSystem.values)
                      DropdownMenuItem(value: s, child: Text(s.label)),
                  ],
                  onChanged: (value) =>
                      setDialogState(() => system = value ?? system),
                ),
                const SizedBox(height: 12),
                OutlinedButton.icon(
                  icon: const Icon(Icons.calendar_today_outlined),
                  label: Text('Início ${formatIsoDay(toIsoDay(start))}'),
                  onPressed: () async {
                    final picked = await showDatePicker(
                      context: context,
                      initialDate: start,
                      firstDate: DateTime(2000),
                      lastDate: DateTime(2100),
                    );
                    if (picked != null) setDialogState(() => start = picked);
                  },
                ),
                Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Text(
                    'A parcela 1 vence um mês depois do início.',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: const Text('Cancelar'),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: const Text('Cadastrar'),
            ),
          ],
        ),
      ),
    );

    // Sem `dispose()` dos controladores: o diálogo ainda está saindo (ver
    // `FinancasScreen._transfer`).
    final nome = nomeController.text.trim();
    final principal = parseAmount(principalController.text);
    final rate = _parseRate(rateController.text);
    final term = int.tryParse(termController.text.trim());
    if (saved != true || !mounted) return;

    if (nome.isEmpty ||
        principal == null ||
        rate == null ||
        term == null ||
        term <= 0) {
      messenger.showSnackBar(
        const SnackBar(
          content: Text(
            'Informe nome, principal e prazo maiores que zero e a taxa.',
          ),
        ),
      );
      return;
    }
    await _repository.createLiability(
      Liability(
        bankAccountId: account.id!,
        nome: nome,
        titular: account.titular,
        principal: principal,
        taxaPercentual: rate,
        prazoMeses: term,
        sistema: system,
        dataInicio: toIsoDay(start),
        createdAt: DateTime.now(),
      ),
    );
    await _load();
  }

  Future<void> _delete(LiabilityWithInstallments item) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Excluir ${item.liability.nome}?'),
        content: const Text(
          'As parcelas desta dívida também saem dos lançamentos da conta.',
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
    await _repository.deleteLiability(item.liability.id!);
    await _load();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Dívidas')),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _create,
        icon: const Icon(Icons.add),
        label: const Text('Dívida'),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _liabilities.isEmpty
          ? const Center(child: Text('Nenhuma dívida cadastrada ainda.'))
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [for (final l in _liabilities) _card(l)],
            ),
    );
  }

  Widget _card(LiabilityWithInstallments item) {
    final textTheme = Theme.of(context).textTheme;
    final liability = item.liability;
    final currency = _account(liability.bankAccountId)?.currency ?? 'BRL';
    final balance = item.balanceOn(_today);
    final paid = item.paidOn(_today);
    final next = item.nextAfter(_today);
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: ExpansionTile(
        key: Key('financas-liability-${liability.id}'),
        title: Text(liability.nome),
        subtitle: Text(
          [
            'Saldo devedor ${formatMoney(balance, currency)}',
            '$paid de ${liability.prazoMeses} parcelas pagas',
            if (next != null)
              'Próxima: ${formatMoney(next.valorParcela, currency)} em '
                  '${formatIsoDay(next.dataVencimento)}'
            else
              'Quitada',
          ].join('\n'),
          key: Key('financas-liability-summary-${liability.id}'),
        ),
        childrenPadding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
        expandedCrossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            '${liability.sistema.label} · '
            '${liability.taxaPercentual.toString().replaceAll('.', ',')}% a.m. · '
            'principal ${formatMoney(liability.principal, currency)}',
            style: textTheme.bodySmall,
          ),
          const SizedBox(height: 8),
          for (final i in item.installments)
            Text(
              '${i.numeroParcela.toString().padLeft(2, '0')} · '
              '${formatIsoDay(i.dataVencimento)} · '
              '${formatMoney(i.valorParcela, currency)} '
              '(juros ${formatMoney(i.valorJuros, currency)})',
              style: textTheme.bodySmall?.copyWith(
                decoration: i.dataVencimento.compareTo(_today) <= 0
                    ? TextDecoration.lineThrough
                    : null,
              ),
            ),
          Align(
            alignment: Alignment.centerRight,
            child: TextButton.icon(
              icon: const Icon(Icons.delete_outline),
              label: const Text('Excluir dívida'),
              onPressed: () => _delete(item),
            ),
          ),
        ],
      ),
    );
  }
}
