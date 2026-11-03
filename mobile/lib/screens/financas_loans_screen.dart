import 'package:flutter/material.dart';

import '../domain/currency_catalog.dart';
import '../domain/general_finance.dart';
import '../domain/money_format.dart';
import '../domain/number_locale.dart';
import '../l10n/app_localizations.dart';
import '../l10n/finance_labels.dart';
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

  /// Roda a gravação e mostra o erro do repositório no idioma da tela. Devolve `false` se
  /// falhou (nada foi gravado pela chamada que lançou).
  Future<bool> _guard(
    AppLocalizations l,
    ScaffoldMessengerState messenger,
    Future<void> Function() action,
  ) async {
    try {
      await action();
      return true;
    } on ArgumentError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(financeErrorText(l, e))));
    } on StateError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(financeErrorText(l, e))));
    }
    return false;
  }

  Widget _dateButton(
    BuildContext context,
    AppLocalizations l,
    DateTime date,
    void Function(DateTime) onPicked, {
    String Function(String)? decorate,
  }) {
    final shown = formatReadingDate(toIsoDay(date), locale: l.localeName);
    return OutlinedButton.icon(
      icon: const Icon(Icons.calendar_today_outlined),
      label: Text(decorate == null ? shown : decorate(shown)),
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
  }

  Future<void> _create() async {
    final l = AppLocalizations.of(context);
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
          title: Text(l.loanNew),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                DropdownButtonFormField<int?>(
                  key: const Key('financas-loan-borrower'),
                  initialValue: borrowerId,
                  isExpanded: true,
                  decoration: InputDecoration(labelText: l.loanBorrower),
                  items: [
                    for (final b in _borrowers)
                      DropdownMenuItem(value: b.id, child: Text(b.nome)),
                    DropdownMenuItem(
                      value: null,
                      child: Text(l.loanNewBorrowerOption),
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
                    decoration: InputDecoration(labelText: l.loanNewBorrowerName),
                  ),
                ],
                const SizedBox(height: 12),
                DropdownButtonFormField<BankAccount?>(
                  key: const Key('financas-loan-account'),
                  initialValue: account,
                  isExpanded: true,
                  decoration: InputDecoration(
                    labelText: l.loanFromAccount,
                    helperText: l.loanNoAccountHelp,
                    helperMaxLines: 3,
                  ),
                  items: [
                    DropdownMenuItem(
                      value: null,
                      child: Text(l.loanNoAccount),
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
                    decoration: InputDecoration(labelText: l.loanCurrency),
                    items: [
                      for (final code in currencyCatalog)
                        DropdownMenuItem(
                          value: code,
                          child: Text('$code — ${currencyName(l, code)}'),
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
                    labelText: l.loanPrincipal(account?.currency ?? currency),
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-loan-rate'),
                  controller: rateController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: InputDecoration(
                    labelText: l.loanRateOptional,
                    helperText: l.loanRateHelp,
                    helperMaxLines: 3,
                  ),
                ),
                const SizedBox(height: 12),
                _dateButton(
                  context,
                  l,
                  start,
                  (picked) => setDialogState(() => start = picked),
                  decorate: l.loanDateOn,
                ),
                SwitchListTile(
                  key: const Key('financas-loan-include'),
                  contentPadding: EdgeInsets.zero,
                  title: Text(l.loanInclude),
                  value: include,
                  onChanged: (value) => setDialogState(() => include = value),
                ),
                TextField(
                  controller: notesController,
                  decoration: InputDecoration(labelText: l.loanNotes),
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l.actionCancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l.actionRegister),
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
      messenger.showSnackBar(SnackBar(content: Text(l.loanInvalid)));
      return;
    }
    final ok = await _guard(l, messenger, () async {
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
    });
    if (ok) await _load();
  }

  Future<void> _edit(LoanWithRepayments item) async {
    final l = AppLocalizations.of(context);
    final messenger = ScaffoldMessenger.of(context);
    final rateController = TextEditingController(
      text: item.loan.taxaMensalPercentual == null
          ? null
          : withDecimalSeparator(
              item.loan.taxaMensalPercentual.toString(),
              l.localeName,
            ),
    );
    final notesController = TextEditingController(text: item.loan.notes);
    var include = item.loan.includeInNetWorth;

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: Text(l.loanEdit),
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
                  decoration: InputDecoration(labelText: l.loanRateOptional),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: Text(l.loanInclude),
                  value: include,
                  onChanged: (value) => setDialogState(() => include = value),
                ),
                TextField(
                  controller: notesController,
                  decoration: InputDecoration(labelText: l.loanNotes),
                ),
              ],
            ),
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

    final rateText = rateController.text.trim();
    final rate = rateText.isEmpty ? null : _parse(rateText, allowZero: true);
    final notes = notesController.text;
    if (saved != true || !mounted) return;
    if (rateText.isNotEmpty && rate == null) {
      messenger.showSnackBar(SnackBar(content: Text(l.loanRateNegative)));
      return;
    }
    final ok = await _guard(
      l,
      messenger,
      () => _repository.updateLoan(
        item.loan.id!,
        taxaMensalPercentual: rate,
        includeInNetWorth: include,
        notes: notes,
      ),
    );
    if (ok) await _load();
  }

  Future<void> _repay(LoanWithRepayments item) async {
    final l = AppLocalizations.of(context);
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
          title: Text(l.loanRepayTitle),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                _dateButton(
                  context,
                  l,
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
                    labelText: l.loanReceived(item.loan.currency),
                    helperText: l.loanReceivedHelp,
                    helperMaxLines: 3,
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-repay-written-off'),
                  controller: writtenOffController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: InputDecoration(
                    labelText: l.loanWrittenOff,
                    helperText: l.loanWrittenOffHelp,
                    helperMaxLines: 3,
                  ),
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<BankAccount?>(
                  key: const Key('financas-repay-account'),
                  initialValue: account,
                  isExpanded: true,
                  decoration: InputDecoration(labelText: l.loanReceivingAccount),
                  items: [
                    DropdownMenuItem(
                      value: null,
                      child: Text(l.loanNoAccount),
                    ),
                    for (final a in candidates)
                      DropdownMenuItem(value: a, child: Text(a.label)),
                  ],
                  onChanged: (value) => setDialogState(() => account = value),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: notesController,
                  decoration: InputDecoration(labelText: l.loanNotes),
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l.actionCancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l.loanRepayConfirm),
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
      messenger.showSnackBar(SnackBar(content: Text(l.loanRepayInvalid)));
      return;
    }
    final ok = await _guard(
      l,
      messenger,
      () => _repository.registerLoanRepayment(
        LoanRepayment(
          loanId: item.loan.id!,
          bankAccountId: account?.id,
          data: toIsoDay(date),
          valorRecebido: received,
          valorBaixado: writtenOff,
          notes: notes,
          createdAt: DateTime.now(),
        ),
      ),
    );
    if (ok) await _load();
  }

  Future<bool> _confirm(AppLocalizations l, String title, String message) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(title),
        content: Text(message),
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
    return confirmed == true;
  }

  Future<void> _deleteLoan(LoanWithRepayments item) async {
    final l = AppLocalizations.of(context);
    final messenger = ScaffoldMessenger.of(context);
    if (!await _confirm(
      l,
      l.loanDeleteTitle(item.borrower.nome),
      l.loanDeleteBody,
    )) {
      return;
    }
    final ok = await _guard(l, messenger, () => _repository.deleteLoan(item.loan.id!));
    if (ok) await _load();
  }

  Future<void> _deleteRepayment(LoanRepayment repayment, String currency) async {
    final l = AppLocalizations.of(context);
    final messenger = ScaffoldMessenger.of(context);
    if (!await _confirm(
      l,
      l.loanDeleteRepayTitle,
      l.loanDeleteRepayBody(
        formatMoney(repayment.valorBaixado, currency, locale: l.localeName),
        formatReadingDate(repayment.data, locale: l.localeName),
      ),
    )) {
      return;
    }
    final ok = await _guard(
      l,
      messenger,
      () => _repository.deleteLoanRepayment(repayment.id!),
    );
    if (ok) await _load();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.loanTitle)),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _create,
        icon: const Icon(Icons.add),
        label: Text(l.loanFab),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _loans.isEmpty
          ? Center(child: Text(l.loanEmpty))
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [for (final item in _loans) _card(l, item)],
            ),
    );
  }

  Widget _card(AppLocalizations l, LoanWithRepayments item) {
    final textTheme = Theme.of(context).textTheme;
    final locale = l.localeName;
    final loan = item.loan;
    final currency = loan.currency;
    final now = _now;
    final interest = item.interestOn(now);
    final balance = item.balanceOn(now);
    final extra = item.extra;
    String money(double value) => formatMoney(value, currency, locale: locale);
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: ExpansionTile(
        key: Key('financas-loan-${loan.id}'),
        title: Text(item.borrower.nome),
        subtitle: Text(
          [
            '${l.loanToReceive(money(balance))}'
                '${loan.includeInNetWorth ? '' : l.loanOutsideNetWorth}',
            l.loanPrincipalOn(
              money(loan.principal),
              formatReadingDate(loan.dataInicio, locale: locale),
            ),
            if (interest > 0) l.loanInterest(money(interest)),
            if (extra != 0)
              extra > 0
                  ? l.loanExtraMore(money(extra.abs()))
                  : l.loanExtraLess(money(extra.abs())),
          ].join('\n'),
          key: Key('financas-loan-summary-${loan.id}'),
        ),
        childrenPadding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
        expandedCrossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (loan.taxaMensalPercentual != null)
            Text(
              l.loanRate(
                withDecimalSeparator(loan.taxaMensalPercentual.toString(), locale),
              ),
              style: textTheme.bodySmall,
            ),
          if (loan.notes != null) Text(loan.notes!, style: textTheme.bodySmall),
          const SizedBox(height: 8),
          if (item.repayments.isEmpty)
            Text(l.loanNoRepayments, style: textTheme.bodySmall)
          else
            for (final r in item.repayments)
              Row(
                children: [
                  Expanded(
                    child: Text(
                      l.loanRepaymentLine(
                        formatReadingDate(r.data, locale: locale),
                        money(r.valorRecebido),
                        money(r.valorBaixado),
                      ),
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
                label: Text(l.loanRepayTitle),
                onPressed: () => _repay(item),
              ),
              TextButton.icon(
                icon: const Icon(Icons.edit_outlined),
                label: Text(l.actionEdit),
                onPressed: () => _edit(item),
              ),
              TextButton.icon(
                icon: const Icon(Icons.delete_outline),
                label: Text(l.actionDelete),
                onPressed: () => _deleteLoan(item),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
