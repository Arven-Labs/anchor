import 'package:material_ui/material_ui.dart';

import '../domain/crypto_tax.dart';
import '../domain/fx_history.dart';
import '../domain/number_locale.dart';
import '../domain/tax_format.dart';
import '../l10n/app_localizations.dart';
import '../l10n/crypto_tax_labels.dart';
import '../models/asset.dart';
import '../models/portfolio_transaction.dart';
import '../services/portfolio_repository.dart';
import '../services/yahoo_quote_service.dart';

/// IR de cripto mês a mês (ver `assessCryptoTax`). Somente leitura: o mobile
/// não guarda pagamento de DARF.
class CryptoTaxScreen extends StatefulWidget {
  const CryptoTaxScreen({super.key});

  @override
  State<CryptoTaxScreen> createState() => _CryptoTaxScreenState();
}

class _CryptoTaxScreenState extends State<CryptoTaxScreen> {
  final _repository = PortfolioRepository();

  final _yahoo = YahooQuoteService();

  bool _loading = true;
  bool _fxUnavailable = false;
  CryptoTaxAssessment? _assessment;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final assets = await _repository.listAssets();
    final transactions = await _repository.listAllTransactions();

    // Cripto antiga em USD só é apurada convertida pelo dólar da data: busca
    // o câmbio desde a 1ª operação em USD, e só se houver alguma.
    final usdIds = {
      for (final a in assets)
        if (a.assetClass == AssetClass.cripto &&
            a.currency == 'USD' &&
            a.id != null)
          a.id!,
    };
    final usdDates = [
      for (final t in transactions)
        if (usdIds.contains(t.assetId) &&
            (t.type == TransactionType.compra ||
                t.type == TransactionType.venda))
          t.date,
    ]..sort();

    FxHistory? usdBrl;
    var fxUnavailable = false;
    if (usdDates.isNotEmpty) {
      try {
        usdBrl = await _yahoo.fetchFxHistoryWithFallback(from: usdDates.first);
      } catch (_) {
        fxUnavailable = true;
      }
    }

    if (!mounted) return;
    setState(() {
      _assessment = assessCryptoTax(assets, transactions, usdBrl: usdBrl);
      _fxUnavailable = fxUnavailable;
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final assessment = _assessment;
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l.cryptoTaxTitle)),
      body: _loading || assessment == null
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                Text(
                  l.cryptoTaxDisclaimer,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                if (assessment.usdConverted) ...[
                  const SizedBox(height: 8),
                  Text(l.cryptoTaxUsdNote,
                      style: Theme.of(context).textTheme.bodySmall),
                ],
                if (_fxUnavailable) ...[
                  const SizedBox(height: 8),
                  Text(
                    l.cryptoTaxFxUnavailable,
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
                      cryptoWarningText(l, warning),
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
                if (assessment.months.isEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 24),
                    child: Center(child: Text(l.cryptoTaxEmpty)),
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
    final l = AppLocalizations.of(context);
    final locale = l.localeName;
    String brl(double value) => formatBrl(value, locale: locale);
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
              cryptoMonthStatus(l, month),
              style: TextStyle(
                fontWeight: FontWeight.bold,
                color: owes ? Theme.of(context).colorScheme.error : null,
              ),
            ),
            const SizedBox(height: 12),
            Text(l.cryptoTaxMonthSales(brl(month.salesTotal))),
            Text(l.cryptoTaxMonthGain(brl(month.gain))),
            if (!month.exempt && month.tax > 0)
              Text(
                l.cryptoTaxTax(
                  brl(month.tax),
                  formatFixed(month.rate * 100, 2, locale: locale),
                ),
              ),
            if (month.carriedIn > 0)
              Text(l.cryptoTaxCarriedIn(brl(month.carriedIn))),
            ExpansionTile(
              tilePadding: EdgeInsets.zero,
              title: Text(
                l.cryptoTaxSalesCount(month.sales.length),
                style: textTheme.bodyMedium,
              ),
              children: [
                for (final sale in month.sales)
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    title: Text(
                      l.cryptoTaxSaleTitle(
                        sale.ticker,
                        formatDay(sale.date, locale: locale),
                      ),
                    ),
                    subtitle: Text(
                      l.cryptoTaxSaleDetail(
                        brl(sale.saleValue),
                        sale.saleValue - sale.proceeds > 0.005
                            ? l.cryptoTaxSaleFee(brl(sale.saleValue - sale.proceeds))
                            : '',
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
