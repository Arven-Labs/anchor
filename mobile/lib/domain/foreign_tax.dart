// IR de ativos no exterior (Lei 14.754/2023): apuração anual, sem DARF mensal.
// Porta de `desktop/src-tauri/src/domain/tax/foreign.rs`, separada da cripto
// porque a base é o resultado do ano inteiro (ganho de capital + proventos) em
// reais, com crédito do imposto retido lá fora.
//
// Regras (constantes abaixo; no desktop a alíquota fica numa tabela editável):
// - Vale para fatos a partir de 2024. Venda de antes disso não é apurada, mas
//   as compras de antes de 2024 ainda formam o custo médio.
// - Cada operação e cada provento é convertido para reais pelo câmbio do
//   próprio dia; o custo médio já nasce em reais. A lei (art. 15) manda a PTAX
//   de fechamento para venda (`ptax`, uma série por moeda); o imposto retido no
//   exterior usa a de compra. Sem PTAX, vale o câmbio de mercado (`fx`) e o
//   ano avisa que é aproximado. Ativo em BRL vale 1. Sem nenhum dos dois, o
//   item fica de fora e o ano avisa.
// - Ganhos e perdas do ano se compensam e os proventos brutos somam ao
//   resultado. Resultado negativo vira prejuízo que abate a base dos anos
//   seguintes. Não há isenção de R$ 35 mil nem DARF mensal.
// - Imposto = base × 15%. O imposto retido no exterior abate o devido, até o
//   limite dele.
// - Dia sem PTAX (fim de semana, feriado) usa a última anterior, até 10 dias
//   (`PtaxSeries`).
//
// ATENÇÃO: os valores vêm de fontes secundárias, as mesmas do desktop;
// conferir na Receita antes de confiar. O mobile não tem desdobramento nem
// bonificação, então só compra, venda e provento entram.

import '../models/asset.dart';
import '../models/portfolio_transaction.dart';
import 'cost_basis.dart';
import 'fx_history.dart';
import 'ptax_series.dart';

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

  /// PTAX ou câmbio de mercado (de reserva).
  final RateSource fxSource;

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
    required this.fxSource,
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

  /// Câmbio do dia do provento (venda).
  final double fxRate;
  final RateSource fxSource;

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
    required this.fxSource,
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

  /// Operação ou provento do ano (`year`) sem câmbio na data (ou em moeda sem
  /// série): ficou de fora da apuração.
  fxMissing,

  /// Operação ou provento do ano (`year`) convertido pelo câmbio de mercado
  /// porque a PTAX faltava: o valor do ano é aproximado.
  fxApproximate,
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

  const ForeignTaxWarning.fxApproximate({required int this.year})
    : kind = ForeignTaxWarningKind.fxApproximate,
      ticker = null,
      date = null;
}

class ForeignTaxAssessment {
  final List<ForeignYear> years;
  final List<ForeignTaxWarning> warnings;

  const ForeignTaxAssessment({required this.years, required this.warnings});
}

/// Câmbio para BRL do dia e de onde veio: 1 para BRL (exato, conta como PTAX
/// para não marcar o ano como aproximado); a PTAX da moeda em `ptax` quando
/// houver; senão a série de mercado em `fx`; `null` sem nenhuma das duas.
(double, RateSource)? _rateToBrl(
  String currency,
  DateTime date,
  RateKind kind,
  Map<String, PtaxSeries> ptax,
  Map<String, FxHistory> fx,
) {
  if (currency == 'BRL') return (1.0, RateSource.ptax);
  final official = ptax[currency]?.rate(date, kind);
  if (official != null) return (official, RateSource.ptax);
  final market = fx[currency]?.rateOn(date);
  if (market != null && market.isFinite && market > 0) {
    return (market, RateSource.market);
  }
  return null;
}

DateTime _day(DateTime date) => DateTime(date.year, date.month, date.day);

/// Posição de um ativo do exterior em 31/12 de `year`, para o informe de Bens e
/// Direitos: quantidade e custo de aquisição em reais (convertido pela cotação
/// de venda do dia de cada compra).
class ForeignHolding {
  final int year;
  final int assetId;

  /// `null` = operações sem custódia informada.
  final int? custodiaId;
  final String ticker;
  final String name;
  final String currency;
  final double quantity;

  /// Custo médio do ativo × a quantidade desta custódia, em BRL (as
  /// custódias somam o custo da posição).
  final double costBrl;

  /// Alguma compra usou o câmbio de mercado porque a PTAX faltava.
  final bool fxApproximate;

  /// Operação sem câmbio na data, ou venda maior que as compras conhecidas:
  /// o custo está subestimado.
  final bool incomplete;

  /// Grupo, código e país da ficha, como o usuário informou no ativo (`null` = não
  /// informado).
  final String? dirpfGroup;
  final String? dirpfCode;
  final String? dirpfCountry;

  const ForeignHolding({
    required this.year,
    required this.assetId,
    this.custodiaId,
    required this.ticker,
    required this.name,
    required this.currency,
    required this.quantity,
    required this.costBrl,
    required this.fxApproximate,
    required this.incomplete,
    this.dirpfGroup,
    this.dirpfCode,
    this.dirpfCountry,
  });
}

/// Foto da posição de cada ativo do exterior em cada 31/12, de 2023 até
/// `throughYear` (padrão: o ano corrente). Porta de `foreign_holdings` do
/// desktop; ano sem posição não gera linha, e as marcas `fxApproximate` e
/// `incomplete` valem daí em diante.
List<ForeignHolding> foreignHoldings(
  List<Asset> assets,
  List<PortfolioTransaction> transactions, {
  Map<String, PtaxSeries> ptax = const {},
  Map<String, FxHistory> fx = const {},
  int? throughYear,
}) {
  final lastYear = throughYear ?? DateTime.now().year;
  final foreignById = {
    for (final asset in assets)
      if (foreignAssetClasses.contains(asset.assetClass) && asset.id != null)
        asset.id!: asset,
  };
  final byAsset = <int, List<(int, PortfolioTransaction)>>{};
  final transfers = <PortfolioTransaction>[];
  var index = 0;
  for (final tx in transactions) {
    final id = tx.assetId;
    if (id == null || !foreignById.containsKey(id)) continue;
    if (tx.type == TransactionType.transferencia) {
      transfers.add(tx);
    } else if (tx.type == TransactionType.compra ||
        tx.type == TransactionType.venda) {
      byAsset.putIfAbsent(id, () => []).add((index++, tx));
    }
  }
  // Só ativo com compra ou venda tem posição; a transferência só move
  // quantidade entre custódias.
  for (final tx in transfers) {
    byAsset[tx.assetId]?.add((index++, tx));
  }

  // No mesmo dia: compra, depois transferência, depois venda.
  int rank(PortfolioTransaction tx) => switch (tx.type) {
    TransactionType.venda => 2,
    TransactionType.transferencia => 1,
    _ => 0,
  };

  final holdings = <ForeignHolding>[];
  for (final entry in byAsset.entries) {
    final asset = foreignById[entry.key]!;
    final events = entry.value
      ..sort((a, b) {
        final x = a.$2, y = b.$2;
        final byDay = _day(x.date).compareTo(_day(y.date));
        if (byDay != 0) return byDay;
        final byRank = rank(x).compareTo(rank(y));
        if (byRank != 0) return byRank;
        final byId = (x.id ?? 0).compareTo(y.id ?? 0);
        return byId != 0 ? byId : a.$1.compareTo(b.$1);
      });

    final position = CostPosition();
    // Quantidade por custódia (`null` = sem custódia); o custo é do ativo.
    final byCustodia = <int?, double>{};
    var approximate = false;
    var incomplete = false;
    var nextYear = foreignFirstYear - 1;
    void snapshot(int year) {
      if (position.quantity <= costEps) return;
      // Saldo negativo numa custódia, ou custódias que não somam a posição:
      // o rateio não fecha.
      final sum = byCustodia.values.fold(0.0, (a, b) => a + b);
      final uneven =
          byCustodia.values.any((q) => q < -1e-6) ||
          (sum - position.quantity).abs() > 1e-6;
      var emitted = false;
      for (final MapEntry(key: custodiaId, value: quantity)
          in byCustodia.entries) {
        if (quantity <= costEps) continue;
        emitted = true;
        holdings.add(
          ForeignHolding(
            year: year,
            assetId: entry.key,
            custodiaId: custodiaId,
            ticker: asset.ticker,
            name: asset.name,
            currency: asset.currency,
            quantity: quantity,
            costBrl: position.cost * quantity / position.quantity,
            fxApproximate: approximate,
            incomplete: incomplete || uneven,
            dirpfGroup: asset.dirpfGroup,
            dirpfCode: asset.dirpfCode,
            dirpfCountry: asset.dirpfCountry,
          ),
        );
      }
      if (!emitted) {
        // Venda sem saldo zerou ou negativou todas as custódias, mas a
        // posição existe: mostra-a inteira.
        holdings.add(
          ForeignHolding(
            year: year,
            assetId: entry.key,
            ticker: asset.ticker,
            name: asset.name,
            currency: asset.currency,
            quantity: position.quantity,
            costBrl: position.cost,
            fxApproximate: approximate,
            incomplete: true,
            dirpfGroup: asset.dirpfGroup,
            dirpfCode: asset.dirpfCode,
            dirpfCountry: asset.dirpfCountry,
          ),
        );
      }
    }

    for (final (_, tx) in events) {
      // Os 31/12 que já passaram pegam a posição de antes deste evento.
      while (nextYear < tx.date.year && nextYear <= lastYear) {
        snapshot(nextYear);
        nextYear++;
      }
      if (tx.type == TransactionType.transferencia) {
        final moved = tx.quantity ?? 0;
        byCustodia[tx.custodiaId] = (byCustodia[tx.custodiaId] ?? 0) - moved;
        byCustodia[tx.transferToCustodiaId] =
            (byCustodia[tx.transferToCustodiaId] ?? 0) + moved;
        continue;
      }
      final converted = _rateToBrl(
        asset.currency,
        tx.date,
        RateKind.sell,
        ptax,
        fx,
      );
      if (converted == null) {
        incomplete = true;
        continue;
      }
      final (rate, source) = converted;
      final quantity = tx.quantity ?? 0;
      if (tx.type == TransactionType.compra) {
        if (source == RateSource.market) approximate = true;
        position.add(quantity, (tx.totalValue + (tx.fee ?? 0)) * rate);
        byCustodia[tx.custodiaId] = (byCustodia[tx.custodiaId] ?? 0) + quantity;
      } else {
        final (_, covered) = position.take(quantity);
        if (covered + costEps < quantity) incomplete = true;
        byCustodia[tx.custodiaId] = (byCustodia[tx.custodiaId] ?? 0) - quantity;
      }
    }
    while (nextYear <= lastYear) {
      snapshot(nextYear);
      nextYear++;
    }
  }
  holdings.sort((a, b) {
    final byYear = a.year.compareTo(b.year);
    if (byYear != 0) return byYear;
    final byAsset = a.assetId.compareTo(b.assetId);
    // Sem custódia primeiro, como no desktop.
    return byAsset != 0
        ? byAsset
        : (a.custodiaId ?? -1).compareTo(b.custodiaId ?? -1);
  });
  return holdings;
}

/// Apura ano a ano o IR dos ativos do exterior. As transações podem vir em
/// qualquer ordem e misturar outras classes e tipos (são ignorados:
/// transferência, aporte e retirada não mexem em custo nem em venda).
ForeignTaxAssessment assessForeignTax(
  List<Asset> assets,
  List<PortfolioTransaction> transactions, {
  Map<String, PtaxSeries> ptax = const {},
  Map<String, FxHistory> fx = const {},
}) {
  final warnings = <ForeignTaxWarning>[];
  final fxMissingYears = <int>{};
  final fxApproximateYears = <int>{};
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
    final converted = _rateToBrl(
      asset.currency,
      tx.date,
      RateKind.sell,
      ptax,
      fx,
    );
    if (converted == null) {
      fxMissingYears.add(tx.date.year);
      continue;
    }
    final (rate, source) = converted;
    if (source == RateSource.market) fxApproximateYears.add(tx.date.year);
    final quantity = tx.quantity ?? 0;
    final position = positions.putIfAbsent(id, CostPosition.new);
    if (tx.type == TransactionType.compra) {
      position.add(quantity, (tx.totalValue + (tx.fee ?? 0)) * rate);
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
    final proceeds = (tx.totalValue - fee) * rate * share;
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
            fxRate: rate,
            fxSource: source,
            saleValue: tx.totalValue * rate * share,
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
    final converted = _rateToBrl(
      asset.currency,
      tx.date,
      RateKind.sell,
      ptax,
      fx,
    );
    if (converted == null) {
      fxMissingYears.add(tx.date.year);
      continue;
    }
    final (rate, source) = converted;
    // O imposto retido usa a cotação para compra; sem ela, a de venda do mesmo dia.
    final (withheldRate, withheldSource) =
        _rateToBrl(asset.currency, tx.date, RateKind.buy, ptax, fx) ??
        converted;
    if (source == RateSource.market ||
        ((tx.foreignTaxWithheld ?? 0) > 0 &&
            withheldSource == RateSource.market)) {
      fxApproximateYears.add(tx.date.year);
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
            fxRate: rate,
            fxSource: source,
            gross: tx.totalValue * rate,
            withheld: (tx.foreignTaxWithheld ?? 0) * withheldRate,
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

  for (final year in fxApproximateYears.toList()..sort()) {
    warnings.add(ForeignTaxWarning.fxApproximate(year: year));
  }
  for (final year in fxMissingYears.toList()..sort()) {
    warnings.add(ForeignTaxWarning.fxMissing(year: year));
  }
  return ForeignTaxAssessment(years: years, warnings: warnings);
}
