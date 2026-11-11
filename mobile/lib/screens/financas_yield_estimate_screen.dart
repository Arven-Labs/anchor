import 'package:material_ui/material_ui.dart';

import '../domain/finance_error.dart';
import '../domain/general_finance.dart';
import '../domain/money_format.dart';
import '../domain/porquinho_yield.dart';
import '../l10n/app_localizations.dart';
import '../l10n/finance_labels.dart';
import '../models/bank_account.dart';
import '../services/general_finance_repository.dart';
import '../services/quote_exceptions.dart';

/// Estimativa do rendimento dos resgates de uma conta Porquinho (porta da entrega 10 da Fase 35 do
/// desktop). O desktop manda cada resgate para a fila de Pendências; aqui a revisão é a própria
/// lista: o usuário marca os resgates que quer e lança. Lançar grava a receita bruta (`Yield`) e o
/// IOF e o IR retidos (`Taxes`); estimar de novo pula os dias que já têm rendimento.
class FinancasYieldEstimateScreen extends StatefulWidget {
  final BankAccount account;

  /// Série diária da Selic meta desde a data; injetável para teste, sem rede.
  final Future<List<SelicPoint>> Function(DateTime from) fetchSelic;

  const FinancasYieldEstimateScreen({super.key, required this.account, required this.fetchSelic});

  @override
  State<FinancasYieldEstimateScreen> createState() => _FinancasYieldEstimateScreenState();
}

class _FinancasYieldEstimateScreenState extends State<FinancasYieldEstimateScreen> {
  final _repository = GeneralFinanceRepository();
  final _pctController = TextEditingController(text: '100');

  bool _busy = false;
  String? _error;
  YieldProposals? _result;
  Set<int> _selected = {};
  bool _booked = false;

  @override
  void dispose() {
    _pctController.dispose();
    super.dispose();
  }

  Future<void> _estimate() async {
    final l = AppLocalizations.of(context);
    setState(() {
      _busy = true;
      _error = null;
      _result = null;
      _booked = false;
    });
    try {
      final pct = double.tryParse(_pctController.text.trim().replaceAll(',', '.'));
      if (pct == null) throw FinanceArgumentError(FinanceErrorCode.pctOfCdiOutOfRange);
      final pctOfCdi = pct / 100;
      if (!pctOfCdi.isFinite || pctOfCdi <= 0 || pctOfCdi > maxPctOfCdi) {
        throw FinanceArgumentError(FinanceErrorCode.pctOfCdiOutOfRange);
      }
      final legs = await _repository.porquinhoLegs(widget.account.id!);
      final hasRedemption = legs.any((leg) => leg.kind == LegKind.redemption);
      // Sem resgate não há o que estimar, e a Selic nem precisa ser buscada.
      final selic = hasRedemption
          ? await widget.fetchSelic(legs.map((leg) => leg.date).reduce((a, b) => a.isBefore(b) ? a : b))
          : <SelicPoint>[];
      final covered = await _repository.daysWithYield(widget.account.id!);
      final result = proposeYields(legs: legs, selic: selic, pctOfCdi: pctOfCdi, coveredDays: covered);
      if (!mounted) return;
      setState(() {
        _result = result;
        _selected = {for (var i = 0; i < result.proposals.length; i++) i};
      });
    } on QuoteUnavailableException {
      if (mounted) setState(() => _error = l.finYieldSelicUnavailable);
    } on Object catch (error) {
      if (mounted) setState(() => _error = financeErrorText(l, error));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _book() async {
    final l = AppLocalizations.of(context);
    final result = _result;
    if (result == null || _selected.isEmpty) return;
    setState(() => _busy = true);
    try {
      await _repository.bookYieldEstimates(
        accountId: widget.account.id!,
        estimates: [for (final i in _selected.toList()..sort()) result.proposals[i]],
      );
      if (!mounted) return;
      setState(() {
        _booked = true;
        _result = null;
        _selected = {};
      });
    } on Object catch (error) {
      if (mounted) setState(() => _error = financeErrorText(l, error));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final result = _result;
    String money(double value) => formatMoney(value, widget.account.currency, locale: l.localeName);
    return Scaffold(
      appBar: AppBar(title: Text(l.finYieldTitle(widget.account.nome))),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(l.finYieldIntro),
          const SizedBox(height: 12),
          TextField(
            key: const Key('financas-yield-pct'),
            controller: _pctController,
            keyboardType: const TextInputType.numberWithOptions(decimal: true),
            decoration: InputDecoration(labelText: l.finYieldPctLabel),
          ),
          const SizedBox(height: 12),
          FilledButton(
            key: const Key('financas-yield-estimate'),
            onPressed: _busy ? null : _estimate,
            child: Text(l.finYieldEstimateButton),
          ),
          if (_busy) const Padding(padding: EdgeInsets.only(top: 12), child: LinearProgressIndicator()),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: Text(
                _error!,
                key: const Key('financas-yield-error'),
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          if (_booked)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: Text(l.finYieldBooked, key: const Key('financas-yield-booked')),
            ),
          if (result != null) ...[
            const SizedBox(height: 12),
            if (result.proposals.isEmpty) Text(l.finYieldEmpty, key: const Key('financas-yield-empty')),
            for (var i = 0; i < result.proposals.length; i++)
              CheckboxListTile(
                key: Key('financas-yield-row-$i'),
                contentPadding: EdgeInsets.zero,
                controlAffinity: ListTileControlAffinity.leading,
                value: _selected.contains(i),
                onChanged: _busy
                    ? null
                    : (checked) => setState(() => checked == true ? _selected.add(i) : _selected.remove(i)),
                title: Text(
                  l.finYieldRowTitle(
                    formatReadingDate(toIsoDay(result.proposals[i].date), locale: l.localeName),
                    money(result.proposals[i].paid),
                  ),
                ),
                subtitle: Text(
                  '${l.finYieldRowDetail(money(result.proposals[i].grossYield), money(result.proposals[i].iof), money(result.proposals[i].ir), money(result.proposals[i].netYield))}'
                  '${result.proposals[i].fullyCovered ? '' : '\n${l.finYieldPartial}'}',
                ),
              ),
            if (result.skippedCoveredDay > 0 || result.skippedNoYield > 0)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(l.finYieldSkipped(result.skippedCoveredDay, result.skippedNoYield)),
              ),
            if (result.proposals.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 12),
                child: FilledButton(
                  key: const Key('financas-yield-book'),
                  onPressed: _busy || _selected.isEmpty ? null : _book,
                  child: Text(l.finYieldBookSelected(_selected.length)),
                ),
              ),
          ],
        ],
      ),
    );
  }
}
