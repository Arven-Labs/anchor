// Junta, por fundo, o que a tela de IR de fundos mostra: a estimativa do próximo
// come-cotas e o IR/IOF estimado de cada resgate. Porta da montagem de
// `desktop/src-tauri/src/commands/fund.rs`; as contas ficam em `come_cotas.dart` e
// `fund_redemption.dart`.

import '../models/asset.dart';
import '../models/fund.dart';
import '../models/portfolio_transaction.dart';
import 'come_cotas.dart';
import 'cost_basis.dart';
import 'fund_compensation.dart';
import 'fund_redemption.dart';

/// Como está o estoque de 2023 de um fundo.
enum EstoqueStatus {
  /// Só fundo fechado ou exclusivo de longo ou curto prazo tem estoque.
  notApplicable,

  /// O usuário não informou como o estoque foi tributado (ou o contesta): nada é
  /// estimado.
  notTaxed,

  /// Falta a cota de 31/12/2023 (ou de até 10 dias antes).
  needsQuota,
  estimated,
}

/// Referência do estoque (Lei 14.754, arts. 27 e 28).
final _estoqueDate = DateTime.utc(2023, 12, 31);

/// A cota do estoque é a última lançada até a referência, no máximo 10 dias antes (o
/// último dia útil de 2023 foi 29/12).
const _estoqueQuotaWindowDays = 10;

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

  /// Resgates estimados, em ordem de data, já com o crédito de prejuízo abatido. Vazio
  /// enquanto o fundo não tem tipo.
  final List<RedemptionEstimate> redemptions;

  /// Administradora ou intermediário informado, como o usuário escreveu; agrupa os
  /// fundos que compensam prejuízo entre si.
  final String? administrator;

  /// Aberto, fechado ou exclusivo (Lei 14.754); só os dois últimos têm estoque.
  final FundStructure structure;

  /// Como o estoque de 2023 foi tributado, quando o usuário informou.
  final EstoqueOption? estoqueOption;
  final EstoqueStatus estoqueStatus;

  /// O imposto do estoque, só quando [estoqueStatus] é `estimated`.
  final EstoqueEstimate? estoque;

  /// Come-cotas pago acima do IR devido, somado nos resgates e amortizações. Só
  /// informação (possível restituição): não entra no IR nem no líquido.
  double get refundableComeCotas =>
      redemptions.fold(0.0, (sum, r) => sum + r.excessComeCotas);

  const FundReport({
    required this.asset,
    required this.term,
    required this.quantity,
    required this.quotas,
    required this.deductions,
    required this.estimate,
    required this.redemptions,
    this.administrator,
    this.structure = FundStructure.open,
    this.estoqueOption,
    this.estoqueStatus = EstoqueStatus.notApplicable,
    this.estoque,
  });

  FundReport withRedemptions(List<RedemptionEstimate> redemptions) =>
      FundReport(
        asset: asset,
        term: term,
        quantity: quantity,
        quotas: quotas,
        deductions: deductions,
        estimate: estimate,
        redemptions: redemptions,
        administrator: administrator,
        structure: structure,
        estoqueOption: estoqueOption,
        estoqueStatus: estoqueStatus,
        estoque: estoque,
      );
}

/// O que a tela de IR de fundos mostra: um relatório por fundo e o prejuízo que sobrou
/// a compensar em cada grupo.
class FundTaxResult {
  final List<FundReport> reports;
  final List<LossCredit> lossCredits;

  const FundTaxResult({required this.reports, required this.lossCredits});
}

/// Valor da cota de uma transação de fundo: o preço unitário (o guard do desktop
/// garante `total = quantidade × preço`), ou o total dividido pelas cotas.
double _quotaOf(PortfolioTransaction tx) {
  final unit = tx.unitPrice;
  if (unit != null) return unit;
  final quantity = tx.quantity ?? 0;
  return quantity > 0 ? tx.totalValue / quantity : 0;
}

/// Por que uma amortização não cabe no histórico do fundo; `null` se cabe. Mesmo guard
/// do desktop (`ensure_amortization_fits`): precisa de cotas na data e o valor não passa
/// do valor da posição (cotas × cota), com folga de `max(0,011; 0,1%)`. [assetTxs] são as
/// transações do fundo; [ignoreId] é a que está sendo editada.
String? amortizationProblem(
  List<PortfolioTransaction> assetTxs, {
  required DateTime date,
  required double quotaValue,
  required double amount,
  int? ignoreId,
}) {
  final day = DateTime.utc(date.year, date.month, date.day);
  var quantity = 0.0;
  for (final tx in assetTxs) {
    if (ignoreId != null && tx.id == ignoreId) continue;
    if (DateTime.utc(tx.date.year, tx.date.month, tx.date.day).isAfter(day)) {
      continue;
    }
    switch (tx.type) {
      case TransactionType.compra:
        quantity += tx.quantity ?? 0;
      case TransactionType.venda:
      case TransactionType.comeCotas:
        quantity -= tx.quantity ?? 0;
      default:
        break;
    }
  }
  if (quantity <= costEps) {
    return 'A amortização precisa de cotas em carteira nessa data.';
  }
  final positionValue = quantity * quotaValue;
  final tolerance = positionValue * 0.001 > 0.011
      ? positionValue * 0.001
      : 0.011;
  if (amount > positionValue + tolerance) {
    return 'O valor amortizado (${amount.toStringAsFixed(2)}) passa do valor da '
        'posição nessa data (${positionValue.toStringAsFixed(2)}).';
  }
  return null;
}

int _byDateThenId(PortfolioTransaction a, PortfolioTransaction b) {
  final byDate = a.date.compareTo(b.date);
  return byDate != 0 ? byDate : (a.id ?? 0).compareTo(b.id ?? 0);
}

/// A cota de 31/12/2023 do estoque: a última lançada até a data, no máximo 10 dias
/// antes. `null` se não há.
double? _estoqueQuota(List<FundQuota> quotas) {
  FundQuota? best;
  for (final quota in quotas) {
    final day = DateTime.utc(quota.date.year, quota.date.month, quota.date.day);
    final delta = _estoqueDate.difference(day).inDays;
    if (delta < 0 || delta > _estoqueQuotaWindowDays) continue;
    if (best == null || quota.date.isAfter(best.date)) best = quota;
  }
  return best?.value;
}

/// Um relatório por ativo da classe `fundo`, na ordem de [assets], com o prejuízo dos
/// resgates compensado entre os fundos do mesmo grupo ([administrators] e o tipo).
FundTaxResult buildFundReports({
  required List<Asset> assets,
  required List<PortfolioTransaction> transactions,
  required Map<int, FundTerm> terms,
  required List<FundQuota> quotas,
  required DateTime today,
  Map<int, String> administrators = const {},
  Map<int, FundStructure> structures = const {},
  Map<int, EstoqueOption> estoqueOptions = const {},
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
        case TransactionType.amortizacao:
          // As cotas ficam e o custo médio cai na fração devolvida.
          position.amortize(tx.totalValue, tx.unitPrice ?? 0);
          events.add(
            FundEvent.amortize(
              date: tx.date,
              id: tx.id ?? 0,
              amount: tx.totalValue,
              quotaValue: tx.unitPrice ?? 0,
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

    // Estoque de rendimento até 31/12/2023 (Lei 14.754, arts. 27 e 28): só fundo
    // fechado ou exclusivo de longo ou curto prazo.
    final structure = structures[id] ?? FundStructure.open;
    final estoqueOption = estoqueOptions[id];
    final hasEstoque =
        structure != FundStructure.open &&
        (term == FundTerm.longTerm || term == FundTerm.shortTerm);
    final referenceQuota = _estoqueQuota(fundQuotas);
    if (hasEstoque && estoqueOption != null && referenceQuota != null) {
      events.add(
        FundEvent.estoque(
          date: _estoqueDate,
          quotaValue: referenceQuota,
          rate: estoqueOption == EstoqueOption.option8
              ? fundEstoqueRateOption
              : fundEstoqueRateGeneral,
        ),
      );
    }
    final fundEstimate = term == null ? null : estimateFund(events, term);
    final estoqueStatus = !hasEstoque
        ? EstoqueStatus.notApplicable
        : estoqueOption == null
        ? EstoqueStatus.notTaxed
        : fundEstimate?.estoque == null
        ? EstoqueStatus.needsQuota
        : EstoqueStatus.estimated;

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
      taxedBasisPerQuota: fundEstimate?.taxedBasisPerQuota,
    );
    reports.add(
      FundReport(
        asset: asset,
        term: term,
        quantity: position.quantity,
        quotas: fundQuotas,
        deductions: deductions.reversed.toList(),
        estimate: estimateComeCotas([holding], today).first,
        redemptions: fundEstimate?.redemptions ?? const [],
        administrator: administrators[id],
        structure: structure,
        estoqueOption: estoqueOption,
        estoqueStatus: estoqueStatus,
        estoque: fundEstimate?.estoque,
      ),
    );
  }

  // Compensação de prejuízo entre os fundos do mesmo grupo: ajusta o IR dos resgates e
  // sobra um saldo por grupo, que ainda abate o ganho da estimativa do próximo
  // come-cotas (as estimativas são as mesmas dos relatórios e mudam no lugar).
  final compFunds = [
    for (final report in reports)
      if (report.term != null)
        CompFund(
          assetId: report.asset.id!,
          ticker: report.asset.ticker,
          term: report.term!,
          administrator: report.administrator,
          redemptions: report.redemptions,
        ),
  ];
  final result = compensate(compFunds);
  final compensated = {
    for (var i = 0; i < compFunds.length; i++)
      compFunds[i].assetId: result.redemptions[i],
  };
  final adjusted = [
    for (final report in reports)
      compensated[report.asset.id] == null
          ? report
          : report.withRedemptions(compensated[report.asset.id]!),
  ];
  applyLossCredit([for (final r in adjusted) r.estimate], result.credits);
  return FundTaxResult(reports: adjusted, lossCredits: result.credits);
}
