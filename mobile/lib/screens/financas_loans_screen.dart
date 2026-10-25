import 'package:flutter/material.dart';

import '../domain/currency_catalog.dart';
import '../domain/general_finance.dart';
import '../domain/money_format.dart';
import '../models/bank_account.dart';
import '../models/loan.dart';
import '../services/general_finance_repository.dart';

/// Empréstimos concedidos a terceiros: devedor, principal, juros simples
/// opcionais, baixas manuais (o que entrou e o que abate do saldo) e a opção
/// de somar o saldo a receber ao patrimônio. Com conta, o principal e cada
/// recebimento viram lançamentos dela.
class FinancasLoansScreen extends StatefulWidget {
  final List<BankAccount> accounts;

  /// "Hoje" injetável; o padrão é o relógio.
  final DateTime? now;

  const FinancasLoansScreen({super.key, required this.accounts, this.now});

  @override
  State<FinancasLoansScreen> createState() => _FinancasLoansScreenState();
}

class _FinancasLoansScreenState extends State<FinancasLoansScreen> {
  final _repository = GeneralFinanceRepository();

  bool _loading = true;
  List<LoanWithRepayments> _loans = [];
  List<Borrower> _borrowers = [];

  DateTime get _now => widget.now ?? DateTime.now();

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final loans = await _repository.listLoans();
    final borrowers = await _repository.listBorrowers();
    if (!mounted) return;
    setState(() {
      _loans = loans;
      _borrowers = borrowers;
      _loading = false;
    });
  }

  /// Aceita vírgula; o zero só quando `allowZero`.
  double? _parse(String text, {bool allowZero = false}) {
    final value = double.tryParse(text.trim().replaceAll(',', '.'));
    if (value == null || !value.isFinite) return null;
    return (allowZero ? value >= 0 : value > 0) ? value : null;
  }

  Widget _dateButton(
    BuildContext context,
    DateTime date,
    void Function(DateTime) onPicked, {
    String prefix = '',
  }) => OutlinedButton.icon(
    icon: const Icon(Icons.calendar_today_outlined),
    label: Text('$prefix${formatIsoDay(toIsoDay(date))}'),
    onPressed: () async {
      final picked = await showDatePicker(
        context: context,
        initialDate: date,
        firstDate: DateTime(2000),
        lastDate: DateTime(2100),
      );
      if (picked != null) onPicked(picked);
    },
  );

  Future<void> _create() async {
    final messenger = ScaffoldMessenger.of(context);
    final nameController = TextEditingController();
    final principalController = TextEditingController();
    final rateController = TextEditingController();
    final notesController = TextEditingController();
    // `null` = devedor novo (nome digitado); senão o id de um existente.
    int? borrowerId = _borrowers.isEmpty ? null : _borrowers.first.id;
    BankAccount? account;
    var currency = defaultBaseCurrency;
    var include = true;
    var start = _now;

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: const Text('Novo empréstimo'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                DropdownButtonFormField<int?>(
                  key: const Key('financas-loan-borrower'),
                  initialValue: borrowerId,
                  isExpanded: true,
                  decoration: const InputDecoration(labelText: 'Devedor'),
                  items: [
                    for (final b in _borrowers)
                      DropdownMenuItem(value: b.id, child: Text(b.nome)),
                    const DropdownMenuItem(
                      value: null,
                      child: Text('Novo devedor…'),
                    ),
                  ],
                  onChanged: (value) =>
                      setDialogState(() => borrowerId = value),
                ),
                if (borrowerId == null) ...[
                  const SizedBox(height: 12),
                  TextField(
                    key: const Key('financas-loan-new-borrower'),
                    controller: nameController,
                    decoration: const InputDecoration(labelText: 'Nome do devedor'),
                  ),
                ],
                const SizedBox(height: 12),
                DropdownButtonFormField<BankAccount?>(
                  key: const Key('financas-loan-account'),
                  initialValue: account,
                  isExpanded: true,
                  decoration: const InputDecoration(
                    labelText: 'Conta de onde saiu',
                    helperText: 'Sem conta é só registro: nada muda em saldo.',
                  ),
                  items: [
                    const DropdownMenuItem(
                      value: null,
                      child: Text('Sem conta'),
                    ),
                    for (final a in widget.accounts)
                      DropdownMenuItem(value: a, child: Text(a.label)),
                  ],
                  onChanged: (value) => setDialogState(() => account = value),
                ),
                if (account == null) ...[
                  const SizedBox(height: 12),
                  DropdownButtonFormField<String>(
                    initialValue: currency,
                    isExpanded: true,
                    decoration: const InputDecoration(labelText: 'Moeda'),
                    items: [
                      for (final (code, name) in currencyCatalog)
                        DropdownMenuItem(
                          value: code,
                          child: Text('$code — $name'),
                        ),
                    ],
                    onChanged: (value) =>
                        setDialogState(() => currency = value ?? currency),
                  ),
                ],
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-loan-principal'),
                  controller: principalController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: InputDecoration(
                    labelText: 'Principal (${account?.currency ?? currency})',
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-loan-rate'),
                  controller: rateController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: const InputDecoration(
                    labelText: 'Taxa (% a.m., opcional)',
                    helperText: 'Juros simples, só informativos. Vazio = sem juros.',
                  ),
                ),
                const SizedBox(height: 12),
                _dateButton(
                  context,
                  start,
                  (picked) => setDialogState(() => start = picked),
                  prefix: 'Em ',
                ),
                SwitchListTile(
                  key: const Key('financas-loan-include'),
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Somar ao patrimônio'),
                  value: include,
                  onChanged: (value) => setDialogState(() => include = value),
                ),
                TextField(
                  controller: notesController,
                  decoration: const InputDecoration(labelText: 'Observação'),
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
    final newName = nameController.text.trim();
    final principal = _parse(principalController.text);
    final rateText = rateController.text.trim();
    final rate = rateText.isEmpty ? null : _parse(rateText, allowZero: true);
    final notes = notesController.text;
    if (saved != true || !mounted) return;

    if (principal == null ||
        (rateText.isNotEmpty && rate == null) ||
        (borrowerId == null && newName.isEmpty)) {
      messenger.showSnackBar(
        const SnackBar(
          content: Text(
            'Informe o devedor e um principal maior que zero; a taxa, se houver, não pode ser negativa.',
          ),
        ),
      );
      return;
    }
    final owner =
        borrowerId ??
        await _repository.insertBorrower(
          Borrower(nome: newName, createdAt: DateTime.now()),
        );
    await _repository.createLoan(
      Loan(
        borrowerId: owner,
        bankAccountId: account?.id,
        principal: principal,
        taxaMensalPercentual: rate,
        dataInicio: toIsoDay(start),
        includeInNetWorth: include,
        currency: account?.currency ?? currency,
        notes: notes,
        createdAt: DateTime.now(),
      ),
    );
    await _load();
  }

  Future<void> _edit(LoanWithRepayments item) async {
    final rateController = TextEditingController(
      text: item.loan.taxaMensalPercentual?.toString(),
    );
    final notesController = TextEditingController(text: item.loan.notes);
    var include = item.loan.includeInNetWorth;

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: const Text('Editar empréstimo'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  key: const Key('financas-loan-edit-rate'),
                  controller: rateController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: const InputDecoration(
                    labelText: 'Taxa (% a.m., opcional)',
                  ),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Somar ao patrimônio'),
                  value: include,
                  onChanged: (value) => setDialogState(() => include = value),
                ),
                TextField(
                  controller: notesController,
                  decoration: const InputDecoration(labelText: 'Observação'),
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
              child: const Text('Salvar'),
            ),
          ],
        ),
      ),
    );

    final rateText = rateController.text.trim();
    final rate = rateText.isEmpty ? null : _parse(rateText, allowZero: true);
    final notes = notesController.text;
    if (saved != true || !mounted) return;
    if (rateText.isNotEmpty && rate == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('A taxa não pode ser negativa.')),
      );
      return;
    }
    await _repository.updateLoan(
      item.loan.id!,
      taxaMensalPercentual: rate,
      includeInNetWorth: include,
      notes: notes,
    );
    await _load();
  }

  Future<void> _repay(LoanWithRepayments item) async {
    final messenger = ScaffoldMessenger.of(context);
    final receivedController = TextEditingController();
    final writtenOffController = TextEditingController();
    final notesController = TextEditingController();
    BankAccount? account;
    var date = _now;
    // Só contas na moeda do empréstimo recebem o dinheiro.
    final candidates = [
      for (final a in widget.accounts)
        if (a.currency == item.loan.currency) a,
    ];

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: const Text('Registrar baixa'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                _dateButton(
                  context,
                  date,
                  (picked) => setDialogState(() => date = picked),
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-repay-received'),
                  controller: receivedController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: InputDecoration(
                    labelText: 'Recebido (${item.loan.currency})',
                    helperText: '0 ou vazio para perdão, sem dinheiro entrando.',
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-repay-written-off'),
                  controller: writtenOffController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: const InputDecoration(
                    labelText: 'Baixado do saldo',
                    helperText: 'Vazio = o mesmo valor recebido.',
                  ),
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<BankAccount?>(
                  key: const Key('financas-repay-account'),
                  initialValue: account,
                  isExpanded: true,
                  decoration: const InputDecoration(
                    labelText: 'Conta que recebeu',
                  ),
                  items: [
                    const DropdownMenuItem(
                      value: null,
                      child: Text('Sem conta'),
                    ),
                    for (final a in candidates)
                      DropdownMenuItem(value: a, child: Text(a.label)),
                  ],
                  onChanged: (value) => setDialogState(() => account = value),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: notesController,
                  decoration: const InputDecoration(labelText: 'Observação'),
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
              child: const Text('Registrar'),
            ),
          ],
        ),
      ),
    );

    final receivedText = receivedController.text.trim();
    final received = receivedText.isEmpty
        ? 0.0
        : _parse(receivedText, allowZero: true);
    final writtenOffText = writtenOffController.text.trim();
    final writtenOff = writtenOffText.isEmpty
        ? (received != null && received > 0 ? received : null)
        : _parse(writtenOffText);
    final notes = notesController.text;
    if (saved != true || !mounted) return;

    if (received == null || writtenOff == null) {
      messenger.showSnackBar(
        const SnackBar(
          content: Text(
            'Informe o valor baixado (maior que zero); o recebido não pode ser negativo.',
          ),
        ),
      );
      return;
    }
    await _repository.registerLoanRepayment(
      LoanRepayment(
        loanId: item.loan.id!,
        bankAccountId: account?.id,
        data: toIsoDay(date),
        valorRecebido: received,
        valorBaixado: writtenOff,
        notes: notes,
        createdAt: DateTime.now(),
      ),
    );
    await _load();
  }

  Future<bool> _confirm(String title, String message) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(title),
        content: Text(message),
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
    return confirmed == true;
  }

  Future<void> _deleteLoan(LoanWithRepayments item) async {
    if (!await _confirm(
      'Excluir o empréstimo de ${item.borrower.nome}?',
      'As baixas e os lançamentos que ele gerou na conta também saem.',
    )) {
      return;
    }
    await _repository.deleteLoan(item.loan.id!);
    await _load();
  }

  Future<void> _deleteRepayment(LoanRepayment repayment, String currency) async {
    if (!await _confirm(
      'Excluir a baixa?',
      'Baixa de ${formatMoney(repayment.valorBaixado, currency)} em '
          '${formatIsoDay(repayment.data)}. O recebimento lançado na conta '
          'também sai.',
    )) {
      return;
    }
    await _repository.deleteLoanRepayment(repayment.id!);
    await _load();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Empréstimos')),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _create,
        icon: const Icon(Icons.add),
        label: const Text('Empréstimo'),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _loans.isEmpty
          ? const Center(child: Text('Nenhum empréstimo cadastrado ainda.'))
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [for (final l in _loans) _card(l)],
            ),
    );
  }

  Widget _card(LoanWithRepayments item) {
    final textTheme = Theme.of(context).textTheme;
    final loan = item.loan;
    final currency = loan.currency;
    final now = _now;
    final interest = item.interestOn(now);
    final balance = item.balanceOn(now);
    final extra = item.extra;
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: ExpansionTile(
        key: Key('financas-loan-${loan.id}'),
        title: Text(item.borrower.nome),
        subtitle: Text(
          [
            'A receber ${formatMoney(balance, currency)}'
                '${loan.includeInNetWorth ? '' : ' (fora do patrimônio)'}',
            'Principal ${formatMoney(loan.principal, currency)} em '
                '${formatIsoDay(loan.dataInicio)}',
            if (interest > 0) 'Juros acumulados ${formatMoney(interest, currency)}',
            if (extra != 0)
              'Recebido a ${extra > 0 ? 'mais' : 'menos'}: '
                  '${formatMoney(extra.abs(), currency)}',
          ].join('\n'),
          key: Key('financas-loan-summary-${loan.id}'),
        ),
        childrenPadding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
        expandedCrossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (loan.taxaMensalPercentual != null)
            Text(
              'Taxa ${loan.taxaMensalPercentual.toString().replaceAll('.', ',')}% a.m. (juros simples)',
              style: textTheme.bodySmall,
            ),
          if (loan.notes != null) Text(loan.notes!, style: textTheme.bodySmall),
          const SizedBox(height: 8),
          if (item.repayments.isEmpty)
            Text('Nenhuma baixa ainda.', style: textTheme.bodySmall)
          else
            for (final r in item.repayments)
              Row(
                children: [
                  Expanded(
                    child: Text(
                      '${formatIsoDay(r.data)} · recebido '
                      '${formatMoney(r.valorRecebido, currency)} · baixou '
                      '${formatMoney(r.valorBaixado, currency)}',
                      style: textTheme.bodySmall,
                    ),
                  ),
                  IconButton(
                    key: Key('financas-repay-delete-${r.id}'),
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(Icons.delete_outline, size: 18),
                    onPressed: () => _deleteRepayment(r, currency),
                  ),
                ],
              ),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: 8,
            children: [
              TextButton.icon(
                icon: const Icon(Icons.add),
                label: const Text('Registrar baixa'),
                onPressed: () => _repay(item),
              ),
              TextButton.icon(
                icon: const Icon(Icons.edit_outlined),
                label: const Text('Editar'),
                onPressed: () => _edit(item),
              ),
              TextButton.icon(
                icon: const Icon(Icons.delete_outline),
                label: const Text('Excluir'),
                onPressed: () => _deleteLoan(item),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
