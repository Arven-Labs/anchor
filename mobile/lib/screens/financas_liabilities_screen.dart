import 'package:material_ui/material_ui.dart';

import '../domain/general_finance.dart';
import '../domain/liability.dart';
import '../domain/money_format.dart';
import '../domain/number_locale.dart';
import '../l10n/app_localizations.dart';
import '../l10n/finance_labels.dart';
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
    final l = AppLocalizations.of(context);
    final messenger = ScaffoldMessenger.of(context);
    if (widget.accounts.isEmpty) {
      messenger.showSnackBar(SnackBar(content: Text(l.liabNeedAccount)));
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
          title: Text(l.liabNew),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  key: const Key('financas-liability-name'),
                  controller: nomeController,
                  decoration: InputDecoration(
                    labelText: l.finFieldName,
                    hintText: l.liabNameHint,
                  ),
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<BankAccount>(
                  initialValue: account,
                  isExpanded: true,
                  decoration: InputDecoration(labelText: l.liabFieldAccount),
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
                    labelText: l.liabFieldPrincipal(account.currency),
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-liability-rate'),
                  controller: rateController,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: InputDecoration(
                    labelText: l.liabFieldRate,
                    helperText: l.liabRateHelp,
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  key: const Key('financas-liability-term'),
                  controller: termController,
                  keyboardType: TextInputType.number,
                  decoration: InputDecoration(labelText: l.liabFieldTerm),
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<AmortizationSystem>(
                  initialValue: system,
                  isExpanded: true,
                  decoration: InputDecoration(labelText: l.liabFieldSystem),
                  items: [
                    for (final s in AmortizationSystem.values)
                      DropdownMenuItem(value: s, child: Text(amortizationName(l, s))),
                  ],
                  onChanged: (value) =>
                      setDialogState(() => system = value ?? system),
                ),
                const SizedBox(height: 12),
                OutlinedButton.icon(
                  icon: const Icon(Icons.calendar_today_outlined),
                  label: Text(
                    l.liabStart(
                      formatReadingDate(toIsoDay(start), locale: l.localeName),
                    ),
                  ),
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
                    l.liabFirstDue,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
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
      messenger.showSnackBar(SnackBar(content: Text(l.liabInvalid)));
      return;
    }
    try {
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
    } on ArgumentError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(financeErrorText(l, e))));
      return;
    } on StateError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(financeErrorText(l, e))));
      return;
    }
    await _load();
  }

  Future<void> _delete(LiabilityWithInstallments item) async {
    final l = AppLocalizations.of(context);
    final messenger = ScaffoldMessenger.of(context);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l.liabDeleteTitle(item.liability.nome)),
        content: Text(l.liabDeleteBody),
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
    try {
      await _repository.deleteLiability(item.liability.id!);
    } on StateError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(financeErrorText(l, e))));
      return;
    }
    await _load();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.liabTitle)),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _create,
        icon: const Icon(Icons.add),
        label: Text(l.liabFab),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _liabilities.isEmpty
          ? Center(child: Text(l.liabEmpty))
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [for (final item in _liabilities) _card(l, item)],
            ),
    );
  }

  Widget _card(AppLocalizations l, LiabilityWithInstallments item) {
    final textTheme = Theme.of(context).textTheme;
    final locale = l.localeName;
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
            l.liabBalance(formatMoney(balance, currency, locale: locale)),
            l.liabPaid(paid, liability.prazoMeses),
            if (next != null)
              l.liabNext(
                formatMoney(next.valorParcela, currency, locale: locale),
                formatReadingDate(next.dataVencimento, locale: locale),
              )
            else
              l.liabSettled,
          ].join('\n'),
          key: Key('financas-liability-summary-${liability.id}'),
        ),
        childrenPadding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
        expandedCrossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l.liabSummary(
              amortizationName(l, liability.sistema),
              withDecimalSeparator(liability.taxaPercentual.toString(), locale),
              formatMoney(liability.principal, currency, locale: locale),
            ),
            style: textTheme.bodySmall,
          ),
          const SizedBox(height: 8),
          for (final i in item.installments)
            Text(
              l.liabInstallmentLine(
                i.numeroParcela.toString().padLeft(2, '0'),
                formatReadingDate(i.dataVencimento, locale: locale),
                formatMoney(i.valorParcela, currency, locale: locale),
                formatMoney(i.valorJuros, currency, locale: locale),
              ),
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
              label: Text(l.liabDeleteAction),
              onPressed: () => _delete(item),
            ),
          ),
        ],
      ),
    );
  }
}
