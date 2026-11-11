import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';

import '../domain/crypto_threshold_input.dart';
import '../l10n/app_localizations.dart';
import '../l10n/crypto_labels.dart';
import '../models/indicator_threshold_view.dart';
import '../services/crypto_score_repository.dart';

/// Os limites verde/vermelho de cada indicador da moeda, com edição. Tocar numa
/// linha abre um diálogo com os dois campos, Salvar e Redefinir. Salvar
/// **reclassifica as leituras já gravadas** do indicador (o valor bruto fica), o
/// que também muda o histórico do score: o passado é visto com a régua de hoje.
class CryptoThresholdsScreen extends StatefulWidget {
  final String coin;
  final CryptoScoreRepository? repository;

  const CryptoThresholdsScreen({super.key, required this.coin, this.repository});

  @override
  State<CryptoThresholdsScreen> createState() => _CryptoThresholdsScreenState();
}

class _CryptoThresholdsScreenState extends State<CryptoThresholdsScreen> {
  late final CryptoScoreRepository _repository =
      widget.repository ?? CryptoScoreRepository();

  List<IndicatorThresholdView>? _thresholds;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final thresholds = await _repository.listThresholds(widget.coin);
    if (!mounted) return;
    setState(() => _thresholds = thresholds);
  }

  Future<void> _edit(IndicatorThresholdView threshold) async {
    final changed = await showDialog<bool>(
      context: context,
      builder: (context) => _ThresholdDialog(
        coin: widget.coin,
        threshold: threshold,
        repository: _repository,
      ),
    );
    if (changed == true) await _load();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final thresholds = _thresholds;
    return Scaffold(
      appBar: AppBar(title: Text(l.thresholdsTitle(widget.coin))),
      body: thresholds == null
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              children: [
                for (final threshold in thresholds)
                  ListTile(
                    key: ValueKey('threshold-${threshold.indicator}'),
                    title: Text(indicatorName(l, threshold.indicator)),
                    subtitle: Text(
                      bandsText(l, threshold.greenBoundary, threshold.redBoundary),
                    ),
                    trailing: Text(
                      threshold.isDefault ? l.thresholdDefault : l.thresholdEdited,
                    ),
                    onTap: () => _edit(threshold),
                  ),
              ],
            ),
    );
  }
}

class _ThresholdDialog extends StatefulWidget {
  final String coin;
  final IndicatorThresholdView threshold;
  final CryptoScoreRepository repository;

  const _ThresholdDialog({
    required this.coin,
    required this.threshold,
    required this.repository,
  });

  @override
  State<_ThresholdDialog> createState() => _ThresholdDialogState();
}

class _ThresholdDialogState extends State<_ThresholdDialog> {
  // Iniciados no primeiro `build` (precisam do idioma, que vem do contexto).
  TextEditingController? _greenField;
  TextEditingController? _redField;
  bool _busy = false;
  String? _error;

  TextEditingController get _green => _greenField!;
  TextEditingController get _red => _redField!;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_greenField != null) return;
    final locale = AppLocalizations.of(context).localeName;
    _greenField = TextEditingController(
      text: formatBoundary(widget.threshold.greenBoundary, locale: locale),
    );
    _redField = TextEditingController(
      text: formatBoundary(widget.threshold.redBoundary, locale: locale),
    );
  }

  @override
  void dispose() {
    _greenField?.dispose();
    _redField?.dispose();
    super.dispose();
  }

  BoundaryDraftError? get _draftError => boundaryDraftError(_green.text, _red.text);

  bool get _dirty =>
      parseBoundary(_green.text) != widget.threshold.greenBoundary ||
      parseBoundary(_red.text) != widget.threshold.redBoundary;

  bool get _canSave => !_busy && _draftError == null && _dirty;

  Future<void> _run(Future<IndicatorThresholdView> Function() action) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await action();
      if (mounted) Navigator.of(context).pop(true);
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _error = e is ArgumentError ? '${e.message}' : '$e';
      });
    }
  }

  Future<void> _save() => _run(
    () => widget.repository.updateThreshold(
      widget.coin,
      widget.threshold.indicator,
      parseBoundary(_green.text)!,
      parseBoundary(_red.text)!,
    ),
  );

  Future<void> _reset() => _run(
    () => widget.repository.resetThreshold(widget.coin, widget.threshold.indicator),
  );

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final threshold = widget.threshold;
    final draftError = _draftError;
    final higherIsBetter = draftHigherIsBetter(_green.text, _red.text);
    const keyboard = TextInputType.numberWithOptions(signed: true, decimal: true);
    // Só o que se digita num número: o regex do `parseBoundary` decide o resto.
    final filter = FilteringTextInputFormatter.allow(RegExp(r'[0-9.,+\-]'));
    return AlertDialog(
      title: Text(indicatorName(l, threshold.indicator)),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            TextField(
              key: const Key('threshold-green'),
              controller: _green,
              keyboardType: keyboard,
              inputFormatters: [filter],
              decoration: InputDecoration(labelText: l.thresholdGreen),
              onChanged: (_) => setState(() {}),
            ),
            const SizedBox(height: 12),
            TextField(
              key: const Key('threshold-red'),
              controller: _red,
              keyboardType: keyboard,
              inputFormatters: [filter],
              decoration: InputDecoration(labelText: l.thresholdRed),
              onChanged: (_) => setState(() {}),
            ),
            const SizedBox(height: 8),
            if (draftError != null)
              Text(
                boundaryErrorText(l, draftError),
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              )
            else if (higherIsBetter != null)
              Text(higherIsBetter ? l.thresholdHigherIsBetter : l.thresholdLowerIsBetter),
            if (threshold.defaultGreen != null && threshold.defaultRed != null) ...[
              const SizedBox(height: 8),
              Text(
                l.thresholdDefaultValues(
                  formatBoundary(threshold.defaultGreen!, locale: l.localeName),
                  formatBoundary(threshold.defaultRed!, locale: l.localeName),
                ),
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
            const SizedBox(height: 8),
            Text(l.thresholdSaveWarning, style: Theme.of(context).textTheme.bodySmall),
            if (_error != null) ...[
              const SizedBox(height: 8),
              Text(
                _error!,
                key: const Key('threshold-error'),
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
          ],
        ),
      ),
      actions: [
        OutlinedButton(
          key: const Key('threshold-reset'),
          onPressed: threshold.isDefault || _busy ? null : _reset,
          child: Text(l.actionReset),
        ),
        TextButton(
          onPressed: _busy ? null : () => Navigator.of(context).pop(false),
          child: Text(l.actionCancel),
        ),
        FilledButton(
          key: const Key('threshold-save'),
          onPressed: _canSave ? _save : null,
          child: Text(_busy ? l.actionSaving : l.actionSave),
        ),
      ],
    );
  }
}
