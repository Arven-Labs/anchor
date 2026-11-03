import 'dart:convert';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../domain/dirpf.dart' show dirpfCodeLength, dirpfCountryLength, dirpfGroupLength;
import '../domain/foreign_holdings_csv.dart';
import '../domain/foreign_tax.dart';
import '../domain/fx_history.dart';
import '../domain/number_locale.dart' show formatFixed;
import '../domain/ptax_series.dart';
import '../domain/tax_format.dart';
import '../l10n/app_localizations.dart';
import '../l10n/foreign_tax_labels.dart';
import '../models/portfolio_transaction.dart';
import '../services/bcb_ptax_service.dart';
import '../services/portfolio_repository.dart';
import '../services/yahoo_quote_service.dart';

/// IR de ativos no exterior ano a ano (ver `assessForeignTax`). Somente
/// leitura: a apuração é anual, não há DARF para marcar como pago.
class ForeignTaxScreen extends StatefulWidget {
  const ForeignTaxScreen({super.key});

  @override
  State<ForeignTaxScreen> createState() => _ForeignTaxScreenState();
}

class _ForeignTaxScreenState extends State<ForeignTaxScreen> {
  final _repository = PortfolioRepository();
  final _yahoo = YahooQuoteService();
  final _bcb = BcbPtaxService();

  bool _loading = true;
  bool _fxUnavailable = false;
  ForeignTaxAssessment? _assessment;
  List<ForeignHolding> _holdings = const [];
  Map<int, String> _custodiaLabels = const {};

  @override
  void initState() {
    super.initState();
    _load();
  }

  /// Abre o diálogo da ficha do ativo, grava o que o usuário digitou e recarrega o informe.
  Future<void> _editFicha(ForeignHolding holding) async {
    final result = await showDialog<({String group, String code, String country})>(
      context: context,
      builder: (_) => _FichaDialog(holding: holding),
    );
    if (result == null || !mounted) return;
    final messenger = ScaffoldMessenger.of(context);
    final l = AppLocalizations.of(context);
    try {
      await _repository.setAssetDirpf(
        holding.assetId,
        group: result.group,
        code: result.code,
        country: result.country,
      );
    } on FormatException {
      messenger.showSnackBar(SnackBar(content: Text(l.foreignFichaInvalid)));
      return;
    }
    await _load();
  }

  Future<void> _load() async {
    final assets = await _repository.listAssets();
    final transactions = await _repository.listAllTransactions();
    final custodias = await _repository.listCustodias();

    // O custo médio nasce em reais, então o câmbio de cada moeda vale desde a
    // 1ª compra de um ativo do exterior nela (não só desde a 1ª venda ou
    // provento). Uma chamada por moeda que não é BRL.
    final currencyById = {
      for (final a in assets)
        if (foreignAssetClasses.contains(a.assetClass) &&
            a.currency != 'BRL' &&
            a.id != null)
          a.id!: a.currency,
    };
    final firstDateOf = <String, DateTime>{};
    for (final t in transactions) {
      final currency = currencyById[t.assetId];
      if (currency == null ||
          (t.type != TransactionType.compra &&
              t.type != TransactionType.venda &&
              t.type != TransactionType.provento)) {
        continue;
      }
      // A data de pagamento do imposto retido também precisa de câmbio (pode cair antes
      // do primeiro fato); a busca vai até hoje, então a data de depois não importa.
      final needed = fxStartDate(t);
      final first = firstDateOf[currency];
      if (first == null || needed.isBefore(first)) {
        firstDateOf[currency] = needed;
      }
    }

    // A lei manda a PTAX do Banco Central. O câmbio de mercado (Yahoo) só é
    // buscado, para aquela moeda, quando a PTAX falha ou não cota a moeda.
    final ptax = <String, PtaxSeries>{};
    final fx = <String, FxHistory>{};
    var fxUnavailable = false;
    for (final entry in firstDateOf.entries) {
      try {
        ptax[entry.key] = await _bcb.fetchPtax(
          currency: entry.key,
          from: entry.value,
        );
        continue;
      } catch (_) {
        // Segue para o câmbio de mercado.
      }
      try {
        fx[entry.key] = await _yahoo.fetchFxHistoryWithFallback(
          from: entry.value,
          currency: entry.key,
        );
      } catch (_) {
        fxUnavailable = true;
      }
    }

    if (!mounted) return;
    setState(() {
      _assessment = assessForeignTax(assets, transactions, ptax: ptax, fx: fx);
      _holdings = foreignHoldings(assets, transactions, ptax: ptax, fx: fx);
      _custodiaLabels = {
        for (final c in custodias)
          if (c.id != null) c.id!: c.label,
      };
      _fxUnavailable = fxUnavailable;
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final assessment = _assessment;
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.foreignTitle)),
      body: _loading || assessment == null
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                Text(
                  l.foreignIntroNote,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                if (_fxUnavailable) ...[
                  const SizedBox(height: 8),
                  Text(
                    l.foreignFxUnavailableNote,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                ],
                const SizedBox(height: 12),
                for (final warning in assessment.warnings)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 8),
                    child: Text(
                      foreignWarningText(l, warning),
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
                if (assessment.years.isEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 24),
                    child: Center(
                      child: Text(l.foreignEmpty, textAlign: TextAlign.center),
                    ),
                  )
                else
                  for (final year in assessment.years.reversed)
                    _YearCard(year: year),
                if (_holdings.isNotEmpty)
                  _HoldingsCard(
                    holdings: _holdings,
                    custodiaLabels: _custodiaLabels,
                    onEditFicha: _editFicha,
                  ),
              ],
            ),
    );
  }
}

/// Informe de Bens e Direitos: posição de cada ativo em 31/12 (ver
/// `foreignHoldings`). A declaração de Y mostra 31/12/(Y−1) e 31/12/Y.
class _HoldingsCard extends StatefulWidget {
  final List<ForeignHolding> holdings;
  final Map<int, String> custodiaLabels;
  final Future<void> Function(ForeignHolding holding) onEditFicha;

  const _HoldingsCard({
    required this.holdings,
    required this.custodiaLabels,
    required this.onEditFicha,
  });

  @override
  State<_HoldingsCard> createState() => _HoldingsCardState();
}

class _HoldingsCardState extends State<_HoldingsCard> {
  int? _year;

  /// Salva o informe inteiro (todos os anos) num CSV com as colunas do desktop.
  Future<void> _exportCsv() async {
    final messenger = ScaffoldMessenger.of(context);
    final l = AppLocalizations.of(context);
    final day = DateTime.now().toIso8601String().substring(0, 10);
    try {
      final path = await FilePicker.platform.saveFile(
        dialogTitle: l.foreignCsvDialogTitle,
        fileName: 'anchor-foreign-holdings-$day.csv',
        type: FileType.custom,
        allowedExtensions: const ['csv'],
        bytes: utf8.encode(
          foreignHoldingsCsv(widget.holdings, widget.custodiaLabels),
        ),
      );
      if (path == null) return;
      messenger.showSnackBar(
        SnackBar(content: Text(l.foreignCsvSaved(widget.holdings.length))),
      );
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(l.foreignCsvFailed('$e'))));
    }
  }

  List<int> get _years {
    final maxYear = widget.holdings
        .map((h) => h.year)
        .reduce((a, b) => a > b ? a : b);
    final years =
        {
            for (final h in widget.holdings) ...[h.year, h.year + 1],
          }.where((y) => y >= foreignFirstYear && y <= maxYear).toList()
          ..sort((a, b) => b.compareTo(a));
    return years;
  }

  @override
  Widget build(BuildContext context) {
    final years = _years;
    if (years.isEmpty) return const SizedBox.shrink();
    final year = years.contains(_year) ? _year! : years.first;
    final textTheme = Theme.of(context).textTheme;
    final l = AppLocalizations.of(context);
    final locale = l.localeName;

    ForeignHolding? at((int, int?) key, int y) {
      for (final h in widget.holdings) {
        if (h.assetId == key.$1 && h.custodiaId == key.$2 && h.year == y) {
          return h;
        }
      }
      return null;
    }

    // Uma linha por ativo e custódia com posição em 31/12/(Y−1) ou em 31/12/Y.
    final keys = {
      for (final h in widget.holdings)
        if (h.year == year - 1 || h.year == year) (h.assetId, h.custodiaId),
    }.toList();
    String labelOf(ForeignHolding h) =>
        h.custodiaId == null ? '' : widget.custodiaLabels[h.custodiaId] ?? '';
    final rows =
        [
          for (final key in keys)
            (previous: at(key, year - 1), current: at(key, year)),
        ]..sort((a, b) {
          final x = (a.current ?? a.previous)!, y = (b.current ?? b.previous)!;
          final byTicker = x.ticker.compareTo(y.ticker);
          return byTicker != 0 ? byTicker : labelOf(x).compareTo(labelOf(y));
        });

    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(l.foreignHoldingsTitle, style: textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(l.foreignHoldingsNote, style: textTheme.bodySmall),
            const SizedBox(height: 8),
            Row(
              children: [
                DropdownButton<int>(
                  value: year,
                  items: [
                    for (final y in years)
                      DropdownMenuItem(
                        value: y,
                        child: Text(l.foreignReturnOf(y)),
                      ),
                  ],
                  onChanged: (y) => setState(() => _year = y),
                ),
                const Spacer(),
                OutlinedButton(
                  onPressed: _exportCsv,
                  child: Text(l.foreignExportCsv),
                ),
              ],
            ),
            if (rows.isEmpty)
              Text(
                l.foreignNoPosition(
                  formatDay(DateTime(year - 1, 12, 31), locale: locale),
                  formatDay(DateTime(year, 12, 31), locale: locale),
                ),
              ),
            for (final row in rows)
              _HoldingRow(
                year: year,
                custodiaLabel: labelOf((row.current ?? row.previous)!),
                previous: row.previous,
                current: row.current,
                onEditFicha: widget.onEditFicha,
              ),
            const SizedBox(height: 4),
            Text(l.foreignHoldingsLegend, style: textTheme.bodySmall),
          ],
        ),
      ),
    );
  }
}

class _HoldingRow extends StatelessWidget {
  final int year;

  /// Vazio nas operações sem custódia.
  final String custodiaLabel;
  final ForeignHolding? previous;
  final ForeignHolding? current;
  final Future<void> Function(ForeignHolding holding) onEditFicha;

  const _HoldingRow({
    required this.year,
    required this.custodiaLabel,
    required this.previous,
    required this.current,
    required this.onEditFicha,
  });

  String _line(AppLocalizations l, DateTime day, ForeignHolding? h) {
    final label = formatDay(day, locale: l.localeName);
    if (h == null) return l.foreignHoldingNone(label);
    return l.foreignHoldingValue(
      label,
      formatBrl(h.costBrl, locale: l.localeName),
      formatHoldingQuantity(h.quantity, locale: l.localeName),
      h.fxApproximate ? l.foreignApproximate : '',
      h.incomplete ? l.foreignIncomplete : '',
    );
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final base = (current ?? previous)!;
    final ficha = dirpfFichaText(
      l,
      group: base.dirpfGroup,
      code: base.dirpfCode,
      country: base.dirpfCountry,
    );
    return ListTile(
      contentPadding: EdgeInsets.zero,
      title: Text('${base.ticker} — ${base.name}'),
      subtitle: Text(
        '${custodiaLabel.isEmpty ? l.foreignNoCustody : custodiaLabel}\n'
        '${_line(l, DateTime(year - 1, 12, 31), previous)}\n'
        '${_line(l, DateTime(year, 12, 31), current)}\n'
        '${l.foreignFichaLine(ficha.isEmpty ? l.foreignFichaNotFilled : ficha)}',
      ),
      isThreeLine: true,
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            icon: const Icon(Icons.edit_note),
            tooltip: l.foreignEditFichaTooltip,
            onPressed: () => onEditFicha(base),
          ),
          IconButton(
            icon: const Icon(Icons.copy),
            tooltip: l.foreignCopyTooltip,
            onPressed: () async {
              final messenger = ScaffoldMessenger.of(context);
              await Clipboard.setData(
                ClipboardData(
                  text: foreignHoldingDescription(
                    l,
                    year: year,
                    ticker: base.ticker,
                    name: base.name,
                    custodiaLabel: custodiaLabel.isEmpty ? null : custodiaLabel,
                    previous: previous,
                    current: current,
                    group: base.dirpfGroup,
                    code: base.dirpfCode,
                    country: base.dirpfCountry,
                  ),
                ),
              );
              messenger.showSnackBar(
                SnackBar(content: Text(l.foreignDescriptionCopied)),
              );
            },
          ),
        ],
      ),
    );
  }
}

/// Os três campos da ficha de um ativo, que o usuário digita a partir do programa da
/// Receita. O app só completa zeros à esquerda e não confere se o par existe na tabela.
class _FichaDialog extends StatefulWidget {
  final ForeignHolding holding;

  const _FichaDialog({required this.holding});

  @override
  State<_FichaDialog> createState() => _FichaDialogState();
}

class _FichaDialogState extends State<_FichaDialog> {
  late final _group = TextEditingController(text: widget.holding.dirpfGroup);
  late final _code = TextEditingController(text: widget.holding.dirpfCode);
  late final _country = TextEditingController(
    text: widget.holding.dirpfCountry,
  );

  @override
  void dispose() {
    _group.dispose();
    _code.dispose();
    _country.dispose();
    super.dispose();
  }

  Widget _field(TextEditingController controller, String label, int maxLength) =>
      TextField(
        controller: controller,
        decoration: InputDecoration(labelText: label),
        keyboardType: TextInputType.number,
        maxLength: maxLength,
      );

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return AlertDialog(
      title: Text(l.foreignFichaDialogTitle(widget.holding.ticker)),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          _field(_group, l.foreignFieldGroup, dirpfGroupLength),
          _field(_code, l.foreignFieldCode, dirpfCodeLength),
          _field(_country, l.foreignFieldCountry, dirpfCountryLength),
          Text(l.foreignFichaHelp),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l.actionCancel),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop((
            group: _group.text,
            code: _code.text,
            country: _country.text,
          )),
          child: Text(l.actionSave),
        ),
      ],
    );
  }
}

class _YearCard extends StatelessWidget {
  final ForeignYear year;

  const _YearCard({required this.year});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final l = AppLocalizations.of(context);
    final locale = l.localeName;
    String brl(double value) => formatBrl(value, locale: locale);
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('${year.year}', style: textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(
              foreignYearStatus(l, year),
              style: TextStyle(
                fontWeight: FontWeight.bold,
                color: year.taxDue > 0
                    ? Theme.of(context).colorScheme.error
                    : null,
              ),
            ),
            const SizedBox(height: 12),
            Text(l.foreignCapitalGain(brl(year.capitalGain))),
            Text(l.foreignDividends(brl(year.dividends))),
            if (year.lossCarriedIn > 0)
              Text(l.foreignLossUsed(brl(year.lossCarriedIn - year.lossCarriedOut))),
            Text(l.foreignTaxableBase(brl(year.taxableBase))),
            Text(
              l.foreignTax(
                brl(year.taxBeforeCredit),
                formatFixed(year.rate * 100, 1, locale: locale),
              ),
            ),
            if (year.withheld > 0)
              Text(l.foreignWithheld(brl(year.withheld), brl(year.creditUsed))),
            if (year.lossCarriedOut > 0)
              Text(l.foreignLossCarriedOut(brl(year.lossCarriedOut))),
            if (year.sales.isNotEmpty)
              ExpansionTile(
                tilePadding: EdgeInsets.zero,
                title: Text(
                  l.foreignSalesCount(year.sales.length),
                  style: textTheme.bodyMedium,
                ),
                children: [
                  for (final sale in year.sales)
                    ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      title: Text(
                        l.foreignItemTitle(sale.ticker, formatDay(sale.date, locale: locale)),
                      ),
                      subtitle: Text(
                        l.foreignSaleDetail(
                          foreignRateLabel(l, sale.fxRate, sale.fxSource),
                          brl(sale.saleValue),
                          brl(sale.cost),
                          brl(sale.gain),
                        ),
                      ),
                    ),
                ],
              ),
            if (year.dividendItems.isNotEmpty)
              ExpansionTile(
                tilePadding: EdgeInsets.zero,
                title: Text(
                  l.foreignDividendsCount(year.dividendItems.length),
                  style: textTheme.bodyMedium,
                ),
                children: [
                  for (final item in year.dividendItems)
                    ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      title: Text(
                        l.foreignItemTitle(item.ticker, formatDay(item.date, locale: locale)),
                      ),
                      subtitle: Text(
                        l.foreignDividendDetail(
                          foreignRateLabel(l, item.fxRate, item.fxSource),
                          brl(item.gross),
                          item.withheld > 0
                              ? l.foreignDividendWithheld(brl(item.withheld))
                              : '',
                        ),
                      ),
                    ),
                ],
              ),
          ],
        ),
      ),
    );
  }
}
