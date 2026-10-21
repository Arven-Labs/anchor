// IR de ativos no exterior (Lei 14.754/2023): apuração anual, sem DARF mensal.
// Porta de `desktop/src-tauri/src/domain/tax/foreign.rs`, separada da cripto
// porque a base é o resultado do ano inteiro (ganho de capital + proventos) em
// reais, com crédito do imposto retido lá fora.
//
// Regras (constantes abaixo; no desktop a alíquota fica numa tabela editável):
// - Vale para fatos a partir de 2024. Venda de antes disso não é apurada, mas
//   as compras de antes de 2024 ainda formam o custo médio.
// - Cada operação e cada provento é convertido para reais pelo dólar do próprio
//   dia (`usdBrl`); o custo médio já nasce em reais. Ativo em BRL vale 1. Outra
//   moeda, ou sem câmbio na data, deixa o item de fora e avisa o ano.
// - Ganhos e perdas do ano se compensam e os proventos brutos somam ao
//   resultado. Resultado negativo vira prejuízo que abate a base dos anos
//   seguintes. Não há isenção de R$ 35 mil nem DARF mensal.
// - Imposto = base × 15%. O imposto retido no exterior abate o devido, até o
//   limite dele.
// - A Receita usa a PTAX; aqui vale o dólar do `FxHistory` (Yahoo), então o
//   valor é aproximado.
//
// ATENÇÃO: os valores vêm de fontes secundárias, as mesmas do desktop;
// conferir na Receita antes de confiar. O mobile não tem desdobramento nem
// bonificação, então só compra, venda e provento entram.

import '../models/asset.dart';
import '../models/portfolio_transaction.dart';
import 'cost_basis.dart';
import 'fx_history.dart';

const foreignTaxRate = 0.15;

/// Primeiro ano-calendário da apuração anual da Lei 14.754.
const foreignFirstYear = 2024;

const foreignAssetClasses = {
  AssetClass.acaoInternacional,
  AssetClass.reit,
  AssetClass.etfUs,
};

/// Uma venda, com o custo médio em reais já baixado.
class ForeignSale {
  final int? txId;
  final int assetId;
  final String ticker;
  final String currency;
  final DateTime date;

  /// Quantidade coberta pelo histórico de compras conhecido.
  final double quantity;

  /// Câmbio do dia da venda (1 para ativo em BRL).
  final double fxRate;

  /// Valor de venda bruto, em BRL (só da parte com custo conhecido).
  final double saleValue;

  /// Valor de venda − taxa, em BRL.
  final double proceeds;
  final double cost;
  final double gain;

  /// Vendeu mais do que o histórico de compras conhecido: o ganho só
  /// considera a parte com custo conhecido.
  final bool costUnknown;

  const ForeignSale({
    required this.txId,
    required this.assetId,
    required this.ticker,
    required this.currency,
    required this.date,
    required this.quantity,
    required this.fxRate,
    required this.saleValue,
    required this.proceeds,
    required this.cost,
    required this.gain,
    required this.costUnknown,
  });
}

class ForeignDividendItem {
  final int? txId;
  final int assetId;
  final String ticker;
  final String currency;
  final DateTime date;
  final double fxRate;

  /// Bruto, em BRL.
  final double gross;

  /// Retido no exterior, em BRL.
  final double withheld;

  const ForeignDividendItem({
    required this.txId,
    required this.assetId,
    required this.ticker,
    required this.currency,
    required this.date,
    required this.fxRate,
    required this.gross,
    required this.withheld,
  });
}

class ForeignYear {
  final int year;
  final double capitalGain;
  final double dividends;

  /// Imposto retido no exterior no ano, em BRL (o que poderia virar crédito).
  final double withheld;

  /// Prejuízo de anos anteriores disponível para abater.
  final double lossCarriedIn;
  final double taxableBase;
  final double rate;
  final double taxBeforeCredit;
  final double creditUsed;
  final double taxDue;
  final double lossCarriedOut;
  final List<ForeignSale> sales;
  final List<ForeignDividendItem> dividendItems;

  const ForeignYear({
    required this.year,
    required this.capitalGain,
    required this.dividends,
    required this.withheld,
    required this.lossCarriedIn,
    required this.taxableBase,
    required this.rate,
    required this.taxBeforeCredit,
    required this.creditUsed,
    required this.taxDue,
    required this.lossCarriedOut,
    required this.sales,
    required this.dividendItems,
  });
}

enum ForeignTaxWarningKind {
  /// Vendeu mais do que o histórico de compras conhecido (`ticker`, `date`).
  costUnknown,

  /// Operação ou provento do ano (`year`) sem câmbio na data (ou em moeda que
  /// não é BRL nem USD): ficou de fora da apuração.
  fxMissing,
}

class ForeignTaxWarning {
  final ForeignTaxWarningKind kind;
  final String? ticker;
  final DateTime? date;
  final int? year;

  const ForeignTaxWarning.costUnknown({
    required String this.ticker,
    required DateTime this.date,
  }) : kind = ForeignTaxWarningKind.costUnknown,
       year = null;

  const ForeignTaxWarning.fxMissing({required int this.year})
    : kind = ForeignTaxWarningKind.fxMissing,
      ticker = null,
      date = null;
}

class ForeignTaxAssessment {
  final List<ForeignYear> years;
  final List<ForeignTaxWarning> warnings;

  const ForeignTaxAssessment({required this.years, required this.warnings});
}

/// Câmbio para BRL do dia: 1 para BRL, `usdBrl` para USD, `null` sem cotação
/// (série ausente, vazia ou outra moeda).
double? _rateToBrl(String currency, DateTime date, FxHistory? usdBrl) {
  if (currency == 'BRL') return 1.0;
  if (currency != 'USD') return null;
  final rate = usdBrl?.rateOn(date);
  return rate != null && rate.isFinite && rate > 0 ? rate : null;
}

DateTime _day(DateTime date) => DateTime(date.year, date.month, date.day);

/// Apura ano a ano o IR dos ativos do exterior. As transações podem vir em
/// qualquer ordem e misturar outras classes e tipos (são ignorados:
/// transferência, aporte e retirada não mexem em custo nem em venda).
ForeignTaxAssessment assessForeignTax(
  List<Asset> assets,
  List<PortfolioTransaction> transactions, {
  FxHistory? usdBrl,
}) {
  final warnings = <ForeignTaxWarning>[];
  final fxMissingYears = <int>{};
  final foreignById = {
    for (final asset in assets)
      if (foreignAssetClasses.contains(asset.assetClass) && asset.id != null)
        asset.id!: asset,
  };

  final trades = <(int, PortfolioTransaction)>[];
  final dividendTxs = <PortfolioTransaction>[];
  var index = 0;
  for (final tx in transactions) {
    if (!foreignById.containsKey(tx.assetId)) continue;
    switch (tx.type) {
      case TransactionType.compra:
      case TransactionType.venda:
        trades.add((index++, tx));
      case TransactionType.provento:
        dividendTxs.add(tx);
      default:
        break;
    }
  }
  // Por ativo, dentro do dia as compras antes das vendas (a ordem de
  // gravação desempata), como na apuração do desktop.
  trades.sort((a, b) {
    final x = a.$2, y = b.$2;
    final byAsset = x.assetId!.compareTo(y.assetId!);
    if (byAsset != 0) return byAsset;
    final byDay = _day(x.date).compareTo(_day(y.date));
    if (byDay != 0) return byDay;
    final xSale = x.type == TransactionType.venda ? 1 : 0;
    final ySale = y.type == TransactionType.venda ? 1 : 0;
    if (xSale != ySale) return xSale.compareTo(ySale);
    final byId = (x.id ?? 0).compareTo(y.id ?? 0);
    return byId != 0 ? byId : a.$1.compareTo(b.$1);
  });

  final salesByYear = <int, List<ForeignSale>>{};
  final positions = <int, CostPosition>{};
  for (final (_, tx) in trades) {
    final asset = foreignById[tx.assetId]!;
    final id = asset.id!;
    final fx = _rateToBrl(asset.currency, tx.date, usdBrl);
    if (fx == null) {
      fxMissingYears.add(tx.date.year);
      continue;
    }
    final quantity = tx.quantity ?? 0;
    final position = positions.putIfAbsent(id, CostPosition.new);
    if (tx.type == TransactionType.compra) {
      position.add(quantity, (tx.totalValue + (tx.fee ?? 0)) * fx);
      continue;
    }

    final (cost, covered) = position.take(quantity);
    final costUnknown = covered + costEps < quantity;
    final share = quantity > costEps ? covered / quantity : 0.0;
    if (tx.date.year < foreignFirstYear) continue;
    if (costUnknown) {
      warnings.add(
        ForeignTaxWarning.costUnknown(ticker: asset.ticker, date: tx.date),
      );
    }
    final fee = tx.fee ?? 0;
    final proceeds = (tx.totalValue - fee) * fx * share;
    salesByYear
        .putIfAbsent(tx.date.year, () => [])
        .add(
          ForeignSale(
            txId: tx.id,
            assetId: id,
            ticker: asset.ticker,
            currency: asset.currency,
            date: tx.date,
            quantity: covered,
            fxRate: fx,
            saleValue: tx.totalValue * fx * share,
            proceeds: proceeds,
            cost: cost,
            gain: proceeds - cost,
            costUnknown: costUnknown,
          ),
        );
  }

  final dividendsByYear = <int, List<ForeignDividendItem>>{};
  for (final tx in dividendTxs.where((t) => t.date.year >= foreignFirstYear)) {
    final asset = foreignById[tx.assetId]!;
    final fx = _rateToBrl(asset.currency, tx.date, usdBrl);
    if (fx == null) {
      fxMissingYears.add(tx.date.year);
      continue;
    }
    dividendsByYear
        .putIfAbsent(tx.date.year, () => [])
        .add(
          ForeignDividendItem(
            txId: tx.id,
            assetId: asset.id!,
            ticker: asset.ticker,
            currency: asset.currency,
            date: tx.date,
            fxRate: fx,
            gross: tx.totalValue * fx,
            withheld: (tx.foreignTaxWithheld ?? 0) * fx,
          ),
        );
  }

  final years = <ForeignYear>[];
  var lossCarried = 0.0;
  final yearKeys = {...salesByYear.keys, ...dividendsByYear.keys}.toList()
    ..sort();
  for (final year in yearKeys) {
    final sales = (salesByYear[year] ?? [])
      ..sort((a, b) {
        final byDate = a.date.compareTo(b.date);
        return byDate != 0 ? byDate : (a.txId ?? 0).compareTo(b.txId ?? 0);
      });
    final items = (dividendsByYear[year] ?? [])
      ..sort((a, b) {
        final byDate = a.date.compareTo(b.date);
        return byDate != 0 ? byDate : (a.txId ?? 0).compareTo(b.txId ?? 0);
      });

    final capitalGain = sales.fold(0.0, (sum, s) => sum + s.gain);
    final dividends = items.fold(0.0, (sum, i) => sum + i.gross);
    final withheld = items.fold(0.0, (sum, i) => sum + i.withheld);
    final netResult = capitalGain + dividends;

    final lossCarriedIn = lossCarried;
    final double taxableBase;
    final double lossCarriedOut;
    if (netResult >= 0) {
      final used = lossCarriedIn < netResult ? lossCarriedIn : netResult;
      taxableBase = netResult - used;
      lossCarriedOut = lossCarriedIn - used;
    } else {
      taxableBase = 0;
      lossCarriedOut = lossCarriedIn - netResult;
    }
    lossCarried = lossCarriedOut;

    final taxBeforeCredit = taxableBase * foreignTaxRate;
    final creditUsed = withheld < taxBeforeCredit ? withheld : taxBeforeCredit;
    years.add(
      ForeignYear(
        year: year,
        capitalGain: capitalGain,
        dividends: dividends,
        withheld: withheld,
        lossCarriedIn: lossCarriedIn,
        taxableBase: taxableBase,
        rate: foreignTaxRate,
        taxBeforeCredit: taxBeforeCredit,
        creditUsed: creditUsed,
        taxDue: taxBeforeCredit - creditUsed,
        lossCarriedOut: lossCarriedOut,
        sales: sales,
        dividendItems: items,
      ),
    );
  }

  for (final year in fxMissingYears.toList()..sort()) {
    warnings.add(ForeignTaxWarning.fxMissing(year: year));
  }
  return ForeignTaxAssessment(years: years, warnings: warnings);
}
