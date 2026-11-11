import 'package:material_ui/material_ui.dart';

import '../domain/b3_tax.dart';
import '../domain/number_locale.dart';
import '../domain/tax_format.dart';
import '../l10n/app_localizations.dart';
import '../l10n/b3_tax_labels.dart';
import '../services/portfolio_repository.dart';

/// IR de renda variável da B3 mês a mês (ver `assessB3Tax`). Somente leitura:
/// o mobile não guarda pagamento de DARF.
class B3TaxScreen extends StatefulWidget {
  const B3TaxScreen({super.key});

  @override
  State<B3TaxScreen> createState() => _B3TaxScreenState();
}

class _B3TaxScreenState extends State<B3TaxScreen> {
  final _repository = PortfolioRepository();

  bool _loading = true;
  B3TaxAssessment? _assessment;

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
      _assessment = assessB3Tax(assets, transactions);
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final assessment = _assessment;
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.b3TaxTitle)),
      body: _loading || assessment == null
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                Text(
                  l.b3TaxDisclaimer,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                const SizedBox(height: 12),
                for (final warning in assessment.warnings)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 8),
                    child: Text(
                      b3WarningText(l, warning),
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
                if (assessment.months.isEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 24),
                    child: Center(child: Text(l.b3TaxEmpty)),
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
  final B3Month month;

  const _MonthCard({required this.month});

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
            Text(formatPeriod(month.period), style: textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(
              b3MonthStatus(l, month),
              style: TextStyle(
                fontWeight: FontWeight.bold,
                color: month.due > 0 ? Theme.of(context).colorScheme.error : null,
              ),
            ),
            const SizedBox(height: 12),
            if (month.stockSalesTotal > 0)
              Text(l.b3TaxMonthStockSales(brl(month.stockSalesTotal))),
            if (month.exemptGain > 0)
              Text(l.b3TaxMonthExemptGain(brl(month.exemptGain))),
            for (final bucket in month.buckets)
              Text(
                l.b3TaxBucketLine(
                  b3BucketName(l, bucket.bucket),
                  brl(bucket.result),
                  brl(bucket.lossUsed),
                  brl(bucket.tax),
                  formatFixed(bucket.rate * 100, 0, locale: locale),
                ),
              ),
            if (month.irrfWithheld > 0)
              Text(l.b3TaxIrrf(brl(month.irrfWithheld))),
            if (month.carriedIn > 0)
              Text(l.b3TaxCarriedIn(brl(month.carriedIn))),
            ExpansionTile(
              tilePadding: EdgeInsets.zero,
              title: Text(
                l.b3TaxSalesCount(month.sales.length),
                style: textTheme.bodyMedium,
              ),
              children: [
                for (final sale in month.sales)
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    title: Text(
                      l.b3TaxSaleTitle(
                        sale.ticker,
                        formatDay(sale.date, locale: locale),
                      ),
                    ),
                    subtitle: Text(
                      l.b3TaxSaleDetail(
                        b3BucketName(l, sale.bucket),
                        brl(sale.saleValue),
                        brl(sale.cost),
                        brl(sale.gain),
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
