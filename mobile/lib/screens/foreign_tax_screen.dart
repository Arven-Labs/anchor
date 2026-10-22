import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../domain/foreign_tax.dart';
import '../domain/fx_history.dart';
import '../domain/ptax_series.dart';
import '../models/portfolio_transaction.dart';
import '../services/bcb_ptax_service.dart';
import '../services/portfolio_repository.dart';
import '../services/yahoo_quote_service.dart';
import 'crypto_tax_screen.dart' show formatBrl, formatDay;

@visibleForTesting
const foreignIntroNote =
    'Calculado só com as transações lançadas neste app, a custo médio, com a '
    'taxa de cada operação e em reais pela PTAX do Banco Central (fechamento, '
    'venda; compra no imposto retido no exterior), e no dia sem cotação pela '
    'última anterior. Onde a PTAX falta, vale o câmbio de mercado (Yahoo) e o '
    'ano avisa que é aproximado. Vai na declaração anual, sem DARF mensal. As '
    'regras vêm de fontes secundárias: confira na Receita antes de pagar.';

@visibleForTesting
const foreignFxUnavailableNote =
    'Câmbio indisponível agora para alguma moeda (nem PTAX nem de mercado): '
    'as operações nela ficaram de fora. Tente de novo em instantes.';

/// Câmbio de um fato e de onde veio: `Câmbio 6,0000 (PTAX)` ou `(mercado)`.
@visibleForTesting
String foreignRateLabel(double rate, RateSource source) =>
    'Câmbio ${rate.toStringAsFixed(4).replaceAll('.', ',')} '
    '(${source == RateSource.ptax ? 'PTAX' : 'mercado'})';

/// Situação do ano numa frase: imposto estimado, coberto pelo crédito, sem
/// imposto por prejuízo ou sem imposto por ficar abaixo do que já foi retido.
@visibleForTesting
String foreignYearStatus(ForeignYear year) {
  if (year.taxDue > 0) {
    return 'Imposto estimado de ${formatBrl(year.taxDue)} na declaração';
  }
  if (year.taxBeforeCredit > 0) {
    return 'Sem imposto a pagar: o retido no exterior cobre os '
        '${formatBrl(year.taxBeforeCredit)} devidos';
  }
  if (year.lossCarriedOut > year.lossCarriedIn) {
    return 'Sem imposto: o ano fechou com prejuízo de '
        '${formatBrl(year.lossCarriedOut - year.lossCarriedIn)}, que abate os '
        'anos seguintes';
  }
  return 'Sem imposto: o resultado do ano coube no prejuízo de anos anteriores';
}

@visibleForTesting
String foreignWarningText(ForeignTaxWarning warning) {
  switch (warning.kind) {
    case ForeignTaxWarningKind.costUnknown:
      return 'Venda de ${warning.ticker} em ${formatDay(warning.date!)} maior '
          'que as compras lançadas: o ganho considera só a parte com custo '
          'conhecido. Lance as compras que faltam.';
    case ForeignTaxWarningKind.fxMissing:
      return 'Operações ou proventos de ${warning.year} sem câmbio (nem PTAX '
          'nem de mercado) para a data: ficaram de fora da apuração.';
    case ForeignTaxWarningKind.fxApproximate:
      return 'Operações ou proventos de ${warning.year} foram convertidos pelo '
          'câmbio de mercado porque a PTAX do Banco Central não estava '
          'disponível para eles (sem rede, moeda que a PTAX não cota ou '
          'buraco longo): os valores do ano são aproximados.';
  }
}

@visibleForTesting
const foreignHoldingsNote =
    'Custo de aquisição em reais, não valor de mercado, só do que está lançado '
    'neste app. Uma linha por custódia: o custo de cada uma é a quantidade dela '
    'vezes o custo médio do ativo, então as custódias somam a posição. Grupo, '
    'código e país da ficha não são preenchidos. Estimativa de fontes '
    'secundárias: confira no programa da Receita.';

@visibleForTesting
const foreignHoldingsLegend =
    '"aproximado": alguma compra sem PTAX usou o câmbio de mercado. '
    '"incompleto": uma operação ficou sem câmbio, ou uma venda foi maior que '
    'as compras lançadas, e o custo está subestimado.';

/// Quantidade sem zeros à direita, com vírgula decimal (até 8 casas).
@visibleForTesting
String formatHoldingQuantity(double quantity) {
  var text = quantity.toStringAsFixed(8);
  text = text.replaceFirst(RegExp(r'0+$'), '').replaceFirst(RegExp(r'\.$'), '');
  return text.replaceAll('.', ',');
}

/// Texto de discriminação de um ativo na declaração de `year` (posição em
/// 31/12 do ano anterior e do ano), para colar no programa da Receita.
@visibleForTesting
String foreignHoldingDescription({
  required int year,
  required String ticker,
  required String name,
  String? custodiaLabel,
  ForeignHolding? previous,
  ForeignHolding? current,
}) {
  String units(ForeignHolding? h) =>
      h == null ? 'sem posição' : '${formatHoldingQuantity(h.quantity)} un.';
  String cost(ForeignHolding? h) => formatBrl(h?.costBrl ?? 0);
  final custody = custodiaLabel == null ? '' : ' Custódia: $custodiaLabel.';
  return [
    '$ticker — $name.$custody',
    'Situação em 31/12/${year - 1}: ${units(previous)}, custo de aquisição '
        '${cost(previous)}.',
    'Situação em 31/12/$year: ${units(current)}, custo de aquisição '
        '${cost(current)}.',
    'Custo de aquisição convertido em reais pela cotação de fechamento (venda) '
        'do Banco Central na data de cada compra.',
  ].join('\n');
}

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
      final first = firstDateOf[currency];
      if (first == null || t.date.isBefore(first)) {
        firstDateOf[currency] = t.date;
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
        fx[entry.key] = await _yahoo.fetchFxHistory(
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
    return Scaffold(
      appBar: AppBar(title: const Text('IR de exterior')),
      body: _loading || assessment == null
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                Text(
                  foreignIntroNote,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                if (_fxUnavailable) ...[
                  const SizedBox(height: 8),
                  Text(
                    foreignFxUnavailableNote,
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
                      foreignWarningText(warning),
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
                if (assessment.years.isEmpty)
                  const Padding(
                    padding: EdgeInsets.only(top: 24),
                    child: Center(
                      child: Text(
                        'Nenhuma venda ou provento de ativo do exterior a '
                        'partir de 2024 ainda.',
                        textAlign: TextAlign.center,
                      ),
                    ),
                  )
                else
                  for (final year in assessment.years.reversed)
                    _YearCard(year: year),
                if (_holdings.isNotEmpty)
                  _HoldingsCard(
                    holdings: _holdings,
                    custodiaLabels: _custodiaLabels,
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

  const _HoldingsCard({required this.holdings, required this.custodiaLabels});

  @override
  State<_HoldingsCard> createState() => _HoldingsCardState();
}

class _HoldingsCardState extends State<_HoldingsCard> {
  int? _year;

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
            Text('Bens e Direitos', style: textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(foreignHoldingsNote, style: textTheme.bodySmall),
            const SizedBox(height: 8),
            DropdownButton<int>(
              value: year,
              items: [
                for (final y in years)
                  DropdownMenuItem(value: y, child: Text('Declaração de $y')),
              ],
              onChanged: (y) => setState(() => _year = y),
            ),
            if (rows.isEmpty)
              Text(
                'Nenhuma posição no exterior em 31/12/${year - 1} nem em '
                '31/12/$year.',
              ),
            for (final row in rows)
              _HoldingRow(
                year: year,
                custodiaLabel: labelOf((row.current ?? row.previous)!),
                previous: row.previous,
                current: row.current,
              ),
            const SizedBox(height: 4),
            Text(foreignHoldingsLegend, style: textTheme.bodySmall),
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

  const _HoldingRow({
    required this.year,
    required this.custodiaLabel,
    required this.previous,
    required this.current,
  });

  String _line(String label, ForeignHolding? h) => h == null
      ? '$label: —'
      : '$label: ${formatBrl(h.costBrl)} '
            '(${formatHoldingQuantity(h.quantity)} un.)'
            '${h.fxApproximate ? ' · aproximado' : ''}'
            '${h.incomplete ? ' · incompleto' : ''}';

  @override
  Widget build(BuildContext context) {
    final base = (current ?? previous)!;
    return ListTile(
      contentPadding: EdgeInsets.zero,
      title: Text('${base.ticker} — ${base.name}'),
      subtitle: Text(
        '${custodiaLabel.isEmpty ? 'Sem custódia' : custodiaLabel}\n'
        '${_line('31/12/${year - 1}', previous)}\n'
        '${_line('31/12/$year', current)}',
      ),
      isThreeLine: true,
      trailing: IconButton(
        icon: const Icon(Icons.copy),
        tooltip: 'Copiar descrição',
        onPressed: () async {
          final messenger = ScaffoldMessenger.of(context);
          await Clipboard.setData(
            ClipboardData(
              text: foreignHoldingDescription(
                year: year,
                ticker: base.ticker,
                name: base.name,
                custodiaLabel: custodiaLabel.isEmpty ? null : custodiaLabel,
                previous: previous,
                current: current,
              ),
            ),
          );
          messenger.showSnackBar(
            const SnackBar(content: Text('Descrição copiada.')),
          );
        },
      ),
    );
  }
}

class _YearCard extends StatelessWidget {
  final ForeignYear year;

  const _YearCard({required this.year});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
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
              foreignYearStatus(year),
              style: TextStyle(
                fontWeight: FontWeight.bold,
                color: year.taxDue > 0
                    ? Theme.of(context).colorScheme.error
                    : null,
              ),
            ),
            const SizedBox(height: 12),
            Text('Ganho de capital: ${formatBrl(year.capitalGain)}'),
            Text('Proventos: ${formatBrl(year.dividends)}'),
            if (year.lossCarriedIn > 0)
              Text(
                'Prejuízo de anos anteriores usado: '
                '${formatBrl(year.lossCarriedIn - year.lossCarriedOut)}',
              ),
            Text('Base de cálculo: ${formatBrl(year.taxableBase)}'),
            Text(
              'Imposto: ${formatBrl(year.taxBeforeCredit)} '
              '(alíquota ${(year.rate * 100).toStringAsFixed(1)}%)',
            ),
            if (year.withheld > 0)
              Text(
                'Retido no exterior: ${formatBrl(year.withheld)} '
                '(crédito de ${formatBrl(year.creditUsed)})',
              ),
            if (year.lossCarriedOut > 0)
              Text(
                'Prejuízo que abate os anos seguintes: '
                '${formatBrl(year.lossCarriedOut)}',
              ),
            if (year.sales.isNotEmpty)
              ExpansionTile(
                tilePadding: EdgeInsets.zero,
                title: Text(
                  'Vendas (${year.sales.length})',
                  style: textTheme.bodyMedium,
                ),
                children: [
                  for (final sale in year.sales)
                    ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      title: Text('${sale.ticker} — ${formatDay(sale.date)}'),
                      subtitle: Text(
                        '${foreignRateLabel(sale.fxRate, sale.fxSource)}'
                        ' · venda ${formatBrl(sale.saleValue)}'
                        ' · custo ${formatBrl(sale.cost)}'
                        ' · ganho ${formatBrl(sale.gain)}',
                      ),
                    ),
                ],
              ),
            if (year.dividendItems.isNotEmpty)
              ExpansionTile(
                tilePadding: EdgeInsets.zero,
                title: Text(
                  'Proventos (${year.dividendItems.length})',
                  style: textTheme.bodyMedium,
                ),
                children: [
                  for (final item in year.dividendItems)
                    ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      title: Text('${item.ticker} — ${formatDay(item.date)}'),
                      subtitle: Text(
                        '${foreignRateLabel(item.fxRate, item.fxSource)}'
                        ' · ${formatBrl(item.gross)}'
                        '${item.withheld > 0 ? ' · retido ${formatBrl(item.withheld)}' : ''}',
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
