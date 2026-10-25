import 'package:flutter/material.dart';

import '../domain/foreign_tax.dart' show foreignAssetClasses;
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
  const PortfolioScreen({super.key});

  @override
  State<PortfolioScreen> createState() => _PortfolioScreenState();
}

/// Texto do "Preço atual" do card: o número, "indisponível agora" quando a
/// fonte falhou de forma passageira, ou "—" quando não há cotação.
@visibleForTesting
String livePriceLabel(
  double? price, {
  bool unavailable = false,
  String? currency,
}) {
  if (price != null) {
    final number = price.toStringAsFixed(2);
    return currency == null ? number : '$currency $number';
  }
  return unavailable ? 'indisponível agora' : '—';
}

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

    final positions = await _repository.getPositions();
    final custodias = await _repository.listCustodias();

    final rows = await loadLivePositions(
      positions,
      repository: _repository,
      dispatcher: _dispatcher,
    );

    if (mounted) {
      setState(() {
        _rows = rows;
        _custodiasById = {for (final c in custodias) c.id!: c};
        _loading = false;
      });
    }
  }

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
    return Scaffold(
      appBar: AppBar(
        title: const Text('Portfolio'),
        actions: [
          if (_rows.any((r) => r.position.asset.assetClass == AssetClass.cripto))
            IconButton(
              icon: const Icon(Icons.calculate),
              tooltip: 'IR de cripto',
              onPressed: _openCryptoTax,
            ),
          if (_rows.any(
            (r) => foreignAssetClasses.contains(r.position.asset.assetClass),
          ))
            IconButton(
              icon: const Icon(Icons.public),
              tooltip: 'IR de exterior',
              onPressed: _openForeignTax,
            ),
          if (_rows.any((r) => r.position.asset.assetClass == AssetClass.fundo))
            IconButton(
              icon: const Icon(Icons.savings),
              tooltip: 'IR de fundos',
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
              tooltip: 'Trocar cripto',
              onPressed: _openCryptoSwap,
            ),
          IconButton(
            icon: const Icon(Icons.account_balance),
            tooltip: 'Custódias',
            onPressed: _openCustodias,
          ),
          IconButton(
            icon: const Icon(Icons.receipt_long),
            tooltip: 'Histórico',
            onPressed: _openHistory,
          ),
          IconButton(
            icon: const Icon(Icons.add_chart),
            tooltip: 'Adicionar ativo',
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
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Text('Nenhum ativo cadastrado ainda.'),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: onAddAsset,
              child: const Text('Adicionar ativo'),
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
                  Text(
                    '${quantity.toStringAsFixed(quantity == quantity.roundToDouble() ? 0 : 2)} un.',
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    'Preço médio: ${avgPrice != null ? '$currency ${avgPrice.toStringAsFixed(2)}' : '—'}',
                  ),
                  Text(
                    'Preço atual: ${livePriceLabel(livePrice, unavailable: row.priceUnavailable, currency: currency)}',
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    'Valor: ${currentValue != null ? '$currency ${currentValue.toStringAsFixed(2)}' : '—'}',
                  ),
                  if (pnl != null && pnlPercent != null)
                    Text(
                      '${pnl >= 0 ? '+' : ''}${pnl.toStringAsFixed(2)} '
                      '(${pnlPercent >= 0 ? '+' : ''}${pnlPercent.toStringAsFixed(1)}%)',
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
                      '${breakdown.quantity.toStringAsFixed(breakdown.quantity == breakdown.quantity.roundToDouble() ? 0 : 2)} un.',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ),
              ],
              if (participationValue != null &&
                  participationPercentual != null) ...[
                const Divider(height: 20),
                Text(
                  'Participação: ${(participationPercentual * 100).toStringAsFixed(2)}% — '
                  '${participationValue.toStringAsFixed(2)}',
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
