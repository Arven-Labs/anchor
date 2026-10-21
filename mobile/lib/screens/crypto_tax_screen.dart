import 'package:flutter/material.dart';

import '../domain/crypto_tax.dart';
import '../services/portfolio_repository.dart';

/// Reais no formato brasileiro (`R$ 1.234,56`), sem depender de `intl`.
@visibleForTesting
String formatBrl(double value) {
  final cents = (value.abs() * 100).round();
  final whole = (cents ~/ 100).toString();
  final grouped = whole.replaceAllMapped(
    RegExp(r'\B(?=(\d{3})+(?!\d))'),
    (_) => '.',
  );
  final fraction = (cents % 100).toString().padLeft(2, '0');
  final sign = value < 0 && cents > 0 ? '-' : '';
  return '${sign}R\$ $grouped,$fraction';
}

@visibleForTesting
String formatDay(DateTime date) =>
    '${date.day.toString().padLeft(2, '0')}/'
    '${date.month.toString().padLeft(2, '0')}/${date.year}';

/// `YYYY-MM` → `MM/YYYY`.
@visibleForTesting
String formatPeriod(String period) =>
    '${period.substring(5, 7)}/${period.substring(0, 4)}';

/// Situação do mês numa frase: isento, sem imposto, abaixo do mínimo ou DARF.
@visibleForTesting
String cryptoMonthStatus(CryptoMonth month) {
  if (month.exempt) {
    return 'Isento: vendas até ${formatBrl(cryptoMonthlyExemption)} no mês';
  }
  if (month.due > 0) {
    return 'DARF ${month.darfCode} de ${formatBrl(month.due)} até '
        '${formatDay(month.dueDate)}';
  }
  if (month.carriedOut > 0) {
    return 'Imposto de ${formatBrl(month.carriedOut)} abaixo do mínimo de '
        '${formatBrl(darfMinimum)}: soma ao mês seguinte';
  }
  return 'Sem imposto: o mês fechou sem ganho';
}

@visibleForTesting
String cryptoWarningText(CryptoTaxWarning warning) {
  switch (warning.kind) {
    case CryptoTaxWarningKind.costUnknown:
      return 'Venda de ${warning.ticker} em ${formatDay(warning.date!)} maior '
          'que as compras lançadas: o ganho considera só a parte com custo '
          'conhecido. Lance as compras que faltam.';
    case CryptoTaxWarningKind.salesNonBrl:
      return 'Vendas de cripto fora de BRL em ${formatPeriod(warning.period!)} '
          'não são apuradas aqui (regra de exterior).';
  }
}

/// IR de cripto mês a mês (ver `assessCryptoTax`). Somente leitura: o mobile
/// não guarda pagamento de DARF.
class CryptoTaxScreen extends StatefulWidget {
  const CryptoTaxScreen({super.key});

  @override
  State<CryptoTaxScreen> createState() => _CryptoTaxScreenState();
}

class _CryptoTaxScreenState extends State<CryptoTaxScreen> {
  final _repository = PortfolioRepository();

  bool _loading = true;
  CryptoTaxAssessment? _assessment;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final assets = await _repository.listAssets();
    final transactions = await _repository.listAllTransactions();
    if (!mounted) return;
    setState(() {
      _assessment = assessCryptoTax(assets, transactions);
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final assessment = _assessment;
    return Scaffold(
      appBar: AppBar(title: const Text('IR de cripto')),
      body: _loading || assessment == null
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                Text(
                  'Calculado só com as transações lançadas neste app, a custo '
                  'médio, sem taxas. As regras vêm de fontes secundárias: '
                  'confira na Receita (GCAP) antes de pagar.',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                const SizedBox(height: 12),
                for (final warning in assessment.warnings)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 8),
                    child: Text(
                      cryptoWarningText(warning),
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
                if (assessment.months.isEmpty)
                  const Padding(
                    padding: EdgeInsets.only(top: 24),
                    child: Center(
                      child: Text('Nenhuma venda de cripto em reais ainda.'),
                    ),
                  )
                else
                  for (final month in assessment.months.reversed)
                    _MonthCard(month: month),
              ],
            ),
    );
  }
}

class _MonthCard extends StatelessWidget {
  final CryptoMonth month;

  const _MonthCard({required this.month});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final owes = !month.exempt && month.due > 0;
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(formatPeriod(month.period), style: textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(
              cryptoMonthStatus(month),
              style: TextStyle(
                fontWeight: FontWeight.bold,
                color: owes ? Theme.of(context).colorScheme.error : null,
              ),
            ),
            const SizedBox(height: 12),
            Text('Vendas do mês: ${formatBrl(month.salesTotal)}'),
            Text('Ganho do mês: ${formatBrl(month.gain)}'),
            if (!month.exempt && month.tax > 0)
              Text(
                'Imposto: ${formatBrl(month.tax)} '
                '(alíquota ${(month.rate * 100).toStringAsFixed(2)}%)',
              ),
            if (month.carriedIn > 0)
              Text(
                'Acumulado de meses anteriores: ${formatBrl(month.carriedIn)}',
              ),
            ExpansionTile(
              tilePadding: EdgeInsets.zero,
              title: Text(
                'Vendas (${month.sales.length})',
                style: textTheme.bodyMedium,
              ),
              children: [
                for (final sale in month.sales)
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    title: Text(
                      '${sale.ticker} — ${formatDay(sale.date)}',
                    ),
                    subtitle: Text(
                      'Venda ${formatBrl(sale.saleValue)} · custo '
                      '${formatBrl(sale.cost)} · ganho ${formatBrl(sale.gain)}',
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
