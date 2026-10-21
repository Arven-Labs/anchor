import 'package:flutter/material.dart';

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

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final assets = await _repository.listAssets();
    final transactions = await _repository.listAllTransactions();

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
      _assessment = assessForeignTax(
        assets,
        transactions,
        ptax: ptax,
        fx: fx,
      );
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
              ],
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
