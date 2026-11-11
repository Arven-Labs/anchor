import 'package:material_ui/material_ui.dart';

import '../domain/general_finance.dart' show toIsoDay;
import '../domain/number_locale.dart';
import '../domain/profitability.dart';
import '../domain/twr.dart';
import '../l10n/app_localizations.dart';
import '../l10n/profitability_labels.dart';
import '../services/profitability_repository.dart';

/// Rentabilidade da carteira mês a mês (TWR por Dietz Modificado, Fase 10.3 do desktop),
/// bruta de imposto. Abrir a tela só calcula com o que está gravado; o preço novo vem do
/// botão de atualizar.
class ProfitabilityScreen extends StatefulWidget {
  /// Cálculo e atualização; o padrão usa o banco e o Yahoo. Injetáveis para teste.
  final Future<ProfitabilityResult> Function()? compute;
  final Future<PriceRefreshSummary> Function()? refresh;

  const ProfitabilityScreen({super.key, this.compute, this.refresh});

  @override
  State<ProfitabilityScreen> createState() => _ProfitabilityScreenState();
}

class _ProfitabilityScreenState extends State<ProfitabilityScreen> {
  late final ProfitabilityRepository _repository = ProfitabilityRepository();

  bool _loading = true;
  bool _refreshing = false;
  ProfitabilityResult? _result;
  Object? _error;
  PriceRefreshSummary? _summary;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    ProfitabilityResult? result;
    Object? error;
    try {
      result = await (widget.compute ?? _repository.compute)();
    } catch (e) {
      error = e;
    }
    if (!mounted) return;
    setState(() {
      _result = result;
      _error = error;
      _loading = false;
    });
  }

  Future<void> _refresh() async {
    setState(() => _refreshing = true);
    PriceRefreshSummary? summary;
    try {
      summary = await (widget.refresh ?? _repository.refreshPriceHistory)();
    } catch (e) {
      if (mounted) setState(() => _error = e);
    }
    if (!mounted) return;
    setState(() {
      _summary = summary;
      _refreshing = false;
    });
    await _load();
  }

  String _pct(double value, String locale) => '${value >= 0 ? '+' : ''}${formatFixed(value, 2, locale: locale)}%';

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(
        title: Text(l.profitabilityTitle),
        actions: [
          IconButton(
            icon: _refreshing
                ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                : const Icon(Icons.refresh),
            tooltip: l.profitabilityRefresh,
            onPressed: _refreshing ? null : _refresh,
          ),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                Text(l.profitabilityBeforeTax, style: Theme.of(context).textTheme.titleMedium),
                if (_summary != null) ...[
                  const SizedBox(height: 8),
                  Text(l.profitabilityRefreshed(_summary!.updated.length)),
                  if (_summary!.failed.isNotEmpty) Text(l.profitabilityFailed(_summary!.failed.join(', '))),
                ],
                const SizedBox(height: 12),
                ..._content(context, l),
              ],
            ),
    );
  }

  List<Widget> _content(BuildContext context, AppLocalizations l) {
    final error = _error;
    if (error != null) {
      return [
        Text(profitabilityErrorText(l, error), style: TextStyle(color: Theme.of(context).colorScheme.error)),
      ];
    }
    final result = _result!;
    final locale = l.localeName;
    final widgets = <Widget>[];

    if (result.months.isEmpty) {
      widgets.add(Text(l.profitabilityEmpty));
    } else {
      final cumulative = [for (final m in result.months) m.rCumulativePct];
      final trailing = trailingTwelveMonthPct(cumulative);
      widgets
        ..add(Text(l.profitabilityCumulative(_pct(cumulative.last, locale))))
        ..add(Text(l.profitabilityTrailing12(trailing == null ? '—' : _pct(trailing, locale))))
        ..add(const SizedBox(height: 8));
      final scheme = Theme.of(context).colorScheme;
      for (final m in result.months) {
        widgets.add(
          ListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(m.yearMonth),
            subtitle: Text(l.profitabilityFinalValue('R\$ ${formatFixed(m.emv, 2, locale: locale)}')),
            trailing: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text(
                  _pct(m.rMonthPct, locale),
                  style: TextStyle(
                    fontWeight: FontWeight.bold,
                    color: m.rMonthPct >= 0 ? scheme.primary : scheme.error,
                  ),
                ),
                Text(
                  l.profitabilityCumulativeShort(_pct(m.rCumulativePct, locale)),
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ],
            ),
          ),
        );
      }
    }

    if (result.excluded.isNotEmpty) {
      widgets
        ..add(const Divider(height: 24))
        ..add(Text(l.profitabilityExcludedTitle, style: Theme.of(context).textTheme.titleSmall))
        ..add(Text(l.profitabilityExcludedNote, style: Theme.of(context).textTheme.bodySmall));
      for (final e in result.excluded) {
        widgets.add(Text(l.profitabilityExcludedLine(e.asset.ticker, toIsoDay(e.splitDate))));
      }
    }
    return widgets;
  }
}
