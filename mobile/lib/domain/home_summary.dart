// Números dos cards da aba Início, calculados das posições já cotadas. Tudo na moeda
// base: a carteira mistura moedas (BRL, USD...), então cada posição é convertida
// antes de somar. Puro, sem I/O.

import '../models/asset.dart';
import '../services/live_positions.dart';
import 'general_finance.dart' show convertToBase;

class AllocationSlice {
  final AssetClass assetClass;

  /// Valor da classe na moeda base.
  final double value;

  /// Fração do total, de 0 a 1.
  final double share;

  const AllocationSlice({
    required this.assetClass,
    required this.value,
    required this.share,
  });
}

class HomeSummary {
  final String base;
  final double totalValue;

  /// Custo das posições que entram no lucro (as mesmas do [profit]).
  final double costBasis;

  /// `null` quando nenhuma posição pôde entrar no lucro (não é o mesmo que zero).
  final double? profit;

  /// `null` sem lucro calculável ou com custo zero.
  final double? profitPercent;
  final List<AllocationSlice> allocation;

  /// Posições abertas que ficaram fora do valor: sem cotação ou sem câmbio.
  final int itemsMissing;

  /// Posições abertas que deveriam entrar no lucro e ficaram fora (sem cotação, sem
  /// preço médio ou sem câmbio). Ativos manuais não contam: ficam fora por desenho.
  final int profitExcluded;

  const HomeSummary({
    required this.base,
    required this.totalValue,
    required this.costBasis,
    required this.profit,
    required this.profitPercent,
    required this.allocation,
    required this.itemsMissing,
    required this.profitExcluded,
  });
}

/// O lucro soma valor e custo só das posições que têm os dois (e câmbio): somar o
/// custo de uma posição sem cotação, sem o valor dela, mostraria um prejuízo que
/// não existe. O valor total e a alocação usam o mesmo valor de `portfolioValuedItems`
/// (a empresa não listada vale a última avaliação manual). Ativos manuais (imóvel,
/// empresa não listada) entram no valor e na alocação, mas não no lucro: o custo
/// deles não é comparável com a avaliação.
HomeSummary computeHomeSummary(
  Iterable<LivePositionRow> rows,
  String base,
  Map<String, double> ratesToBrl,
) {
  var totalValue = 0.0;
  var profitValue = 0.0;
  var costBasis = 0.0;
  var profitCount = 0;
  var itemsMissing = 0;
  var profitExcluded = 0;
  final byClass = <AssetClass, double>{};

  for (final row in rows) {
    final position = row.position;
    if (position.netQuantity <= 1e-9) continue;
    final asset = position.asset;
    final currency = asset.currency;

    final localValue = asset.assetClass == AssetClass.empresaNaoListada
        ? row.manualTotalValue
        : row.marketValue;
    final value = localValue == null
        ? null
        : convertToBase(localValue, currency, base, ratesToBrl);
    if (value == null) {
      itemsMissing++;
    } else {
      totalValue += value;
      byClass[asset.assetClass] = (byClass[asset.assetClass] ?? 0) + value;
    }

    if (asset.assetClass.isManualAsset) continue;
    final average = position.averageBuyPrice;
    final cost = average == null
        ? null
        : convertToBase(position.netQuantity * average, currency, base, ratesToBrl);
    if (value == null || cost == null) {
      profitExcluded++;
    } else {
      profitValue += value;
      costBasis += cost;
      profitCount++;
    }
  }

  final profit = profitCount == 0 ? null : profitValue - costBasis;
  final slices = [
    for (final entry in byClass.entries)
      if (entry.value > 0 && totalValue > 0)
        AllocationSlice(
          assetClass: entry.key,
          value: entry.value,
          share: entry.value / totalValue,
        ),
  ]..sort((a, b) => b.value.compareTo(a.value));

  return HomeSummary(
    base: base,
    totalValue: totalValue,
    costBasis: costBasis,
    profit: profit,
    profitPercent: profit == null || costBasis == 0
        ? null
        : profit / costBasis * 100,
    allocation: slices,
    itemsMissing: itemsMissing,
    profitExcluded: profitExcluded,
  );
}
