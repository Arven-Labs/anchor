// Junta, por fundo, o que a tela de IR de fundos mostra: a estimativa do próximo
// come-cotas e o IR/IOF estimado de cada resgate. Porta da montagem de
// `desktop/src-tauri/src/commands/fund.rs`; as contas ficam em `come_cotas.dart` e
// `fund_redemption.dart`.

import '../models/asset.dart';
import '../models/fund.dart';
import '../models/portfolio_transaction.dart';
import 'come_cotas.dart';
import 'cost_basis.dart';
import 'fund_redemption.dart';

class FundReport {
  final Asset asset;
  final FundTerm? term;

  /// Cotas na carteira hoje.
  final double quantity;

  /// Valores de cota lançados, da data mais nova para a mais antiga.
  final List<FundQuota> quotas;

  /// Descontos de come-cotas registrados, do mais novo para o mais antigo.
  final List<PortfolioTransaction> deductions;
  final ComeCotasEstimate estimate;

  /// Resgates estimados, em ordem de data. Vazio enquanto o fundo não tem tipo.
  final List<RedemptionEstimate> redemptions;

  const FundReport({
    required this.asset,
    required this.term,
    required this.quantity,
    required this.quotas,
    required this.deductions,
    required this.estimate,
    required this.redemptions,
  });
}

/// Valor da cota de uma transação de fundo: o preço unitário (o guard do desktop
/// garante `total = quantidade × preço`), ou o total dividido pelas cotas.
double _quotaOf(PortfolioTransaction tx) {
  final unit = tx.unitPrice;
  if (unit != null) return unit;
  final quantity = tx.quantity ?? 0;
  return quantity > 0 ? tx.totalValue / quantity : 0;
}

int _byDateThenId(PortfolioTransaction a, PortfolioTransaction b) {
  final byDate = a.date.compareTo(b.date);
  return byDate != 0 ? byDate : (a.id ?? 0).compareTo(b.id ?? 0);
}

/// Um relatório por ativo da classe `fundo`, na ordem de [assets].
List<FundReport> buildFundReports({
  required List<Asset> assets,
  required List<PortfolioTransaction> transactions,
  required Map<int, FundTerm> terms,
  required List<FundQuota> quotas,
  required DateTime today,
}) {
  final reports = <FundReport>[];
  for (final asset in assets.where((a) => a.assetClass == AssetClass.fundo)) {
    final id = asset.id;
    if (id == null) continue;
    final term = terms[id];
    final txs = transactions.where((t) => t.assetId == id).toList()
      ..sort(_byDateThenId);

    // Custo médio corrente: a compra soma (valor + taxa); venda e come-cotas
    // tiram cotas ao custo médio, que não muda.
    final position = CostPosition();
    DateTime? firstBuy;
    final events = <FundEvent>[];
    final deductions = <PortfolioTransaction>[];
    for (final tx in txs) {
      final quantity = tx.quantity ?? 0;
      switch (tx.type) {
        case TransactionType.compra:
          final cost = tx.totalValue + (tx.fee ?? 0);
          position.add(quantity, cost);
          if (firstBuy == null || tx.date.isBefore(firstBuy)) firstBuy = tx.date;
          events.add(
            FundEvent.buy(
              date: tx.date,
              id: tx.id ?? 0,
              quantity: quantity,
              cost: cost,
            ),
          );
        case TransactionType.comeCotas:
          position.take(quantity);
          deductions.add(tx);
          events.add(
            FundEvent.comeCotas(
              date: tx.date,
              id: tx.id ?? 0,
              quantity: quantity,
              quotaValue: _quotaOf(tx),
            ),
          );
        case TransactionType.venda:
          position.take(quantity);
          events.add(
            FundEvent.redeem(
              date: tx.date,
              id: tx.id ?? 0,
              quantity: quantity,
              quotaValue: _quotaOf(tx),
            ),
          );
        case TransactionType.aporte:
        case TransactionType.retirada:
        case TransactionType.provento:
        case TransactionType.transferencia:
          break;
      }
    }

    final fundQuotas = quotas.where((q) => q.assetId == id).toList()
      ..sort((a, b) => b.date.compareTo(a.date));
    final holding = FundHolding(
      assetId: id,
      ticker: asset.ticker,
      name: asset.name,
      term: term,
      quantity: position.quantity,
      averageCost: position.quantity > costEps
          ? position.cost / position.quantity
          : 0,
      firstBuy: firstBuy,
      quotas: [for (final q in fundQuotas) QuotaValue(q.date, q.value)],
      deductions: [for (final d in deductions) QuotaValue(d.date, _quotaOf(d))],
    );
    reports.add(
      FundReport(
        asset: asset,
        term: term,
        quantity: position.quantity,
        quotas: fundQuotas,
        deductions: deductions.reversed.toList(),
        estimate: estimateComeCotas([holding], today).first,
        redemptions: term == null ? const [] : estimateRedemptions(events, term),
      ),
    );
  }
  return reports;
}
