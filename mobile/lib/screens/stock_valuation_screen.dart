import 'package:material_ui/material_ui.dart';

import '../domain/number_locale.dart';
import '../domain/stock_valuation.dart';
import '../domain/tax_format.dart';
import '../domain/valuation_fields.dart';
import '../l10n/app_localizations.dart';
import '../l10n/valuation_labels.dart';
import '../models/stock_valuation.dart';
import '../services/stock_valuation_repository.dart';
import 'stock_valuations_list_screen.dart';

/// Calcula e salva o preço justo de uma ação por um dos 8 modelos (ver `calculateValuation`).
/// Os campos vêm de `valuationFields`. [initial] reabre um cálculo salvo, com o modelo e as
/// premissas preenchidos; a cotação é a de quem chamou (a da busca ou a gravada no cálculo).
class StockValuationScreen extends StatefulWidget {
  final String ticker;
  final double currentPrice;
  final StockValuation? initial;

  const StockValuationScreen({
    super.key,
    required this.ticker,
    required this.currentPrice,
    this.initial,
  });

  @override
  State<StockValuationScreen> createState() => _StockValuationScreenState();
}

class _StockValuationScreenState extends State<StockValuationScreen> {
  final _repository = StockValuationRepository();

  late ValuationModel _model = widget.initial?.model ?? ValuationModel.bazin;
  final _controllers = <ValuationModel, Map<String, TextEditingController>>{};
  ValuationOutcome? _outcome;
  ValuationException? _error;

  @override
  void dispose() {
    for (final byKey in _controllers.values) {
      for (final controller in byKey.values) {
        controller.dispose();
      }
    }
    super.dispose();
  }

  /// Um controlador por campo e por modelo, criado na 1ª vez que o modelo aparece: trocar de
  /// modelo e voltar mantém o que foi digitado.
  Map<String, TextEditingController> _controllersFor(ValuationModel model, String locale) {
    return _controllers.putIfAbsent(model, () {
      final initial = widget.initial;
      return {
        for (final field in valuationFields[model]!)
          field.key: TextEditingController(
            text: initial != null && initial.model == model && initial.inputs[field.key] != null
                ? _display(initial.inputs[field.key]!, field.format, locale)
                : '',
          ),
      };
    });
  }

  /// Taxa guardada em fração (0,06) volta como se digita (6), sem zeros à direita.
  String _display(double value, ValuationFieldFormat format, String locale) {
    final shown = format == ValuationFieldFormat.percent ? value * 100 : value;
    var text = shown.toStringAsFixed(6);
    if (text.contains('.')) {
      text = text.replaceFirst(RegExp(r'0+$'), '').replaceFirst(RegExp(r'\.$'), '');
    }
    return withDecimalSeparator(text, locale);
  }

  /// Campo vazio ou ilegível fica de fora (o cálculo acusa `missingInput`). Porcentagem digitada
  /// em % vira fração, como no desktop.
  Map<String, double> _inputs(String locale) {
    final inputs = <String, double>{};
    for (final field in valuationFields[_model]!) {
      final text = _controllersFor(_model, locale)[field.key]!.text.trim().replaceAll(',', '.');
      final parsed = double.tryParse(text);
      if (parsed == null) continue;
      inputs[field.key] = field.format == ValuationFieldFormat.percent ? parsed / 100 : parsed;
    }
    return inputs;
  }

  void _calculate(String locale) {
    try {
      final outcome = calculateValuation(_model, _inputs(locale), widget.currentPrice);
      setState(() {
        _outcome = outcome;
        _error = null;
      });
    } on ValuationException catch (error) {
      setState(() {
        _outcome = null;
        _error = error;
      });
    }
  }

  Future<void> _save(String locale) async {
    final messenger = ScaffoldMessenger.of(context);
    final saved = AppLocalizations.of(context).valuationSaved;
    try {
      final record = await _repository.save(
        ticker: widget.ticker,
        model: _model,
        currentPrice: widget.currentPrice,
        inputs: _inputs(locale),
      );
      if (!mounted) return;
      setState(() {
        _outcome = ValuationOutcome(
          fairPrice: record.fairPrice,
          safetyMargin: record.safetyMargin,
          verdict: record.verdict,
        );
        _error = null;
      });
      messenger.showSnackBar(SnackBar(content: Text(saved)));
    } on ValuationException catch (error) {
      if (!mounted) return;
      setState(() {
        _outcome = null;
        _error = error;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final locale = l.localeName;
    final theme = Theme.of(context);
    final outcome = _outcome;
    final error = _error;
    return Scaffold(
      appBar: AppBar(
        title: Text(l.valuationTitle(widget.ticker)),
        actions: [
          IconButton(
            icon: const Icon(Icons.history),
            tooltip: l.valuationSavedList,
            onPressed: () => Navigator.of(context).push(
              MaterialPageRoute(builder: (_) => StockValuationsScreen(ticker: widget.ticker)),
            ),
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text(l.valuationDisclaimer, style: theme.textTheme.bodySmall),
          const SizedBox(height: 12),
          Text(
            l.valuationCurrentPrice(formatBrl(widget.currentPrice, locale: locale)),
            style: theme.textTheme.titleMedium,
          ),
          const SizedBox(height: 16),
          DropdownButtonFormField<ValuationModel>(
            initialValue: _model,
            isExpanded: true,
            decoration: InputDecoration(labelText: l.valuationModelLabel),
            items: [
              for (final model in ValuationModel.values)
                DropdownMenuItem(value: model, child: Text(valuationModelName(l, model))),
            ],
            onChanged: (value) {
              if (value == null) return;
              setState(() {
                _model = value;
                _outcome = null;
                _error = null;
              });
            },
          ),
          const SizedBox(height: 8),
          for (final field in valuationFields[_model]!)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: TextField(
                key: ValueKey('${_model.name}-${field.key}'),
                controller: _controllersFor(_model, locale)[field.key],
                keyboardType: field.format == ValuationFieldFormat.integer
                    ? TextInputType.number
                    : const TextInputType.numberWithOptions(decimal: true, signed: true),
                decoration: InputDecoration(labelText: valuationFieldLabel(l, field.key)),
              ),
            ),
          const SizedBox(height: 16),
          Row(
            children: [
              Expanded(
                child: FilledButton(
                  onPressed: () => _calculate(locale),
                  child: Text(l.valuationCalculate),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: OutlinedButton(
                  onPressed: () => _save(locale),
                  child: Text(l.valuationSave),
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),
          if (error != null)
            Text(
              valuationErrorText(l, error),
              style: TextStyle(color: theme.colorScheme.error),
            ),
          if (outcome != null)
            Card(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      l.valuationFairPrice(formatBrl(outcome.fairPrice, locale: locale)),
                      style: theme.textTheme.titleMedium,
                    ),
                    const SizedBox(height: 4),
                    Text(
                      l.valuationSafetyMargin(
                        '${formatFixed(outcome.safetyMargin * 100, 1, locale: locale)}%',
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      valuationVerdictText(l, outcome.verdict),
                      style: TextStyle(
                        fontWeight: FontWeight.bold,
                        color: outcome.verdict == ValuationVerdict.cheap
                            ? theme.colorScheme.primary
                            : theme.colorScheme.error,
                      ),
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}
