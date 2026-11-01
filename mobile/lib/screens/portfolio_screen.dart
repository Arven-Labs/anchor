import 'package:flutter/material.dart';

import '../domain/foreign_tax.dart' show foreignAssetClasses;
import '../domain/number_locale.dart';
import '../l10n/app_localizations.dart';
import '../models/asset.dart';
import '../models/custodia.dart';
import '../services/live_positions.dart';
import '../services/portfolio_repository.dart';
import '../services/quote_dispatcher.dart';
import 'add_asset_screen.dart';
import 'add_transaction_screen.dart';
import 'crypto_swap_screen.dart';
import 'crypto_tax_screen.dart';
import 'custodias_screen.dart';
import 'foreign_tax_screen.dart';
import 'fund_tax_screen.dart';
import 'manual_asset_details_screen.dart';
import 'transaction_history_screen.dart';

class PortfolioScreen extends StatefulWidget {
  /// Posições com preço atual; o padrão lê a carteira e cota na rede. Injetável para
  /// teste, sem rede (como na `HomeScreen`).
  final Future<List<LivePositionRow>> Function()? loadPositions;

  const PortfolioScreen({super.key, this.loadPositions});

  @override
  State<PortfolioScreen> createState() => _PortfolioScreenState();
}

/// Texto do "Preço atual" do card: o número (separador decimal do idioma), "indisponível
/// agora" quando a fonte falhou de forma passageira, ou "—" quando não há cotação.
@visibleForTesting
String livePriceLabel(
  AppLocalizations l,
  double? price, {
  bool unavailable = false,
  String? currency,
}) {
  if (price != null) {
    final number = formatFixed(price, 2, locale: l.localeName);
    return currency == null ? number : '$currency $number';
  }
  return unavailable ? l.portfolioPriceUnavailable : '—';
}

/// Quantidade sem casas quando é inteira, com duas quando não é.
String _quantityLabel(double quantity, String locale) =>
    formatFixed(quantity, quantity == quantity.roundToDouble() ? 0 : 2, locale: locale);

class _PortfolioScreenState extends State<PortfolioScreen> {
  final _repository = PortfolioRepository();
  final _dispatcher = QuoteDispatcher();

  bool _loading = true;
  List<LivePositionRow> _rows = [];
  Map<int, Custodia> _custodiasById = {};

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);

    final custodias = await _repository.listCustodias();

    final rows = await (widget.loadPositions ?? _livePositions)();

    if (mounted) {
      setState(() {
        _rows = rows;
        _custodiasById = {for (final c in custodias) c.id!: c};
        _loading = false;
      });
    }
  }

  Future<List<LivePositionRow>> _livePositions() async => loadLivePositions(
    await _repository.getPositions(),
    repository: _repository,
    dispatcher: _dispatcher,
  );

  Future<void> _openAddAsset() async {
    final saved = await Navigator.of(
      context,
    ).push<bool>(MaterialPageRoute(builder: (_) => const AddAssetScreen()));
    if (saved == true) _load();
  }

  Future<void> _openAddTransaction() async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(builder: (_) => const AddTransactionScreen()),
    );
    if (saved == true) _load();
  }

  Future<void> _openCryptoTax() async {
    await Navigator.of(
      context,
    ).push(MaterialPageRoute(builder: (_) => const CryptoTaxScreen()));
  }

  Future<void> _openForeignTax() async {
    await Navigator.of(
      context,
    ).push(MaterialPageRoute(builder: (_) => const ForeignTaxScreen()));
  }

  Future<void> _openFundTax() async {
    // Volta recarregando: as cotas lançadas lá mudam o preço das posições.
    await Navigator.of(
      context,
    ).push(MaterialPageRoute(builder: (_) => const FundTaxScreen()));
    _load();
  }

  Future<void> _openCryptoSwap() async {
    final saved = await Navigator.of(
      context,
    ).push<bool>(MaterialPageRoute(builder: (_) => const CryptoSwapScreen()));
    if (saved == true) _load();
  }

  Future<void> _openHistory() async {
    await Navigator.of(
      context,
    ).push(MaterialPageRoute(builder: (_) => const TransactionHistoryScreen()));
    _load();
  }

  Future<void> _openCustodias() async {
    await Navigator.of(
      context,
    ).push(MaterialPageRoute(builder: (_) => const CustodiasScreen()));
    _load();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(
        title: Text(l.portfolioTitle),
        actions: [
          if (_rows.any((r) => r.position.asset.assetClass == AssetClass.cripto))
            IconButton(
              icon: const Icon(Icons.calculate),
              tooltip: l.portfolioCryptoTax,
              onPressed: _openCryptoTax,
            ),
          if (_rows.any(
            (r) => foreignAssetClasses.contains(r.position.asset.assetClass),
          ))
            IconButton(
              icon: const Icon(Icons.public),
              tooltip: l.portfolioForeignTax,
              onPressed: _openForeignTax,
            ),
          if (_rows.any((r) => r.position.asset.assetClass == AssetClass.fundo))
            IconButton(
              icon: const Icon(Icons.savings),
              tooltip: l.portfolioFundTax,
              onPressed: _openFundTax,
            ),
          // Permuta só faz sentido com duas criptos cadastradas (como no desktop).
          if (_rows
                  .where(
                    (r) => r.position.asset.assetClass == AssetClass.cripto,
                  )
                  .length >=
              2)
            IconButton(
              icon: const Icon(Icons.swap_horiz),
              tooltip: l.portfolioSwapCrypto,
              onPressed: _openCryptoSwap,
            ),
          IconButton(
            icon: const Icon(Icons.account_balance),
            tooltip: l.portfolioCustodies,
            onPressed: _openCustodias,
          ),
          IconButton(
            icon: const Icon(Icons.receipt_long),
            tooltip: l.portfolioHistory,
            onPressed: _openHistory,
          ),
          IconButton(
            icon: const Icon(Icons.add_chart),
            tooltip: l.portfolioAddAsset,
            onPressed: _openAddAsset,
          ),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _rows.isEmpty
          ? _EmptyState(onAddAsset: _openAddAsset)
          : RefreshIndicator(
              onRefresh: _load,
              child: ListView.builder(
                padding: const EdgeInsets.all(16),
                itemCount: _rows.length,
                itemBuilder: (context, index) => _PositionCard(
                  row: _rows[index],
                  custodiasById: _custodiasById,
                ),
              ),
            ),
      floatingActionButton: _rows.isEmpty
          ? null
          : FloatingActionButton(
              onPressed: _openAddTransaction,
              child: const Icon(Icons.add),
            ),
    );
  }
}

class _EmptyState extends StatelessWidget {
  final VoidCallback onAddAsset;

  const _EmptyState({required this.onAddAsset});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(l.portfolioEmpty),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: onAddAsset,
              child: Text(l.portfolioAddAsset),
            ),
          ],
        ),
      ),
    );
  }
}

class _PositionCard extends StatelessWidget {
  final LivePositionRow row;
  final Map<int, Custodia> custodiasById;

  const _PositionCard({required this.row, required this.custodiasById});

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final locale = l.localeName;
    final position = row.position;
    final quantity = position.netQuantity;
    final avgPrice = position.averageBuyPrice;
    final livePrice = row.livePrice;
    final currency = position.asset.currency;

    final currentValue = livePrice != null ? quantity * livePrice : null;
    final costBasis = avgPrice != null ? quantity * avgPrice : null;
    final pnl = (currentValue != null && costBasis != null)
        ? currentValue - costBasis
        : null;
    final pnlPercent = (pnl != null && costBasis != null && costBasis != 0)
        ? (pnl / costBasis) * 100
        : null;

    final positiveColor = Theme.of(context).colorScheme.primary;
    final negativeColor = Theme.of(context).colorScheme.error;

    final byCustodia = position.byCustodia;
    final showCustodiaBreakdown =
        byCustodia.length > 1 || byCustodia.any((b) => b.custodiaId != null);

    final isManualAsset = position.asset.assetClass.isManualAsset;
    final participationValue = position.asset.equityParticipationValue;
    final participationPercentual = position.asset.equityPercentual;

    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: InkWell(
        onTap: isManualAsset
            ? () => Navigator.of(context).push(
                MaterialPageRoute(
                  builder: (_) =>
                      ManualAssetDetailsScreen(asset: position.asset),
                ),
              )
            : null,
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        position.asset.ticker,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      Text(
                        position.asset.name,
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ],
                  ),
                  Text(l.portfolioUnits(_quantityLabel(quantity, locale))),
                ],
              ),
              const SizedBox(height: 12),
              // `Wrap` em vez de `Row`: o inglês é mais longo ("Current price: unavailable
              // right now") e estourava a linha. Aqui cada texto vai inteiro para a linha
              // de baixo quando não cabe, em vez de quebrar no meio do valor.
              Wrap(
                alignment: WrapAlignment.spaceBetween,
                spacing: 8,
                runSpacing: 4,
                children: [
                  Text(
                    l.portfolioAveragePrice(
                      avgPrice != null
                          ? '$currency ${formatFixed(avgPrice, 2, locale: locale)}'
                          : '—',
                    ),
                  ),
                  Text(
                    l.portfolioCurrentPrice(
                      livePriceLabel(
                        l,
                        livePrice,
                        unavailable: row.priceUnavailable,
                        currency: currency,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    l.portfolioValue(
                      currentValue != null
                          ? '$currency ${formatFixed(currentValue, 2, locale: locale)}'
                          : '—',
                    ),
                  ),
                  if (pnl != null && pnlPercent != null)
                    Text(
                      '${pnl >= 0 ? '+' : ''}${formatFixed(pnl, 2, locale: locale)} '
                      '(${pnlPercent >= 0 ? '+' : ''}${formatFixed(pnlPercent, 1, locale: locale)}%)',
                      style: TextStyle(
                        color: pnl >= 0 ? positiveColor : negativeColor,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                ],
              ),
              if (showCustodiaBreakdown) ...[
                const Divider(height: 20),
                for (final breakdown in byCustodia)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 2),
                    child: Text(
                      '${custodiasById[breakdown.custodiaId]?.label ?? '—'}: '
                      '${l.portfolioUnits(_quantityLabel(breakdown.quantity, locale))}',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ),
              ],
              if (participationValue != null &&
                  participationPercentual != null) ...[
                const Divider(height: 20),
                Text(
                  l.portfolioShare(
                    formatFixed(participationPercentual * 100, 2, locale: locale),
                    formatFixed(participationValue, 2, locale: locale),
                  ),
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
