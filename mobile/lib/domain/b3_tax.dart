// IR de renda variável na B3 (pessoa física): ações, units, ETFs, BDRs e FIIs.
// Porta de `desktop/src-tauri/src/domain/tax/assessment.rs` (sem opções,
// desdobramento/bonificação, simulador de venda nem pagamento de DARF: o celular
// não tem esses dados).
//
// Regras (constantes abaixo; no desktop ficam numa tabela editável):
// - Gavetas de prejuízo separadas: comum (ações, units, ETFs, BDRs), day trade
//   e FII. O prejuízo de uma gaveta só compensa ganho da mesma gaveta, sem prazo.
// - Ações (não units) vendidas em swing somando até R$ 20 mil brutos no mês:
//   ganho isento. Acima disso, o ganho do mês todo é tributado. O ganho isento
//   não consome prejuízo acumulado. Units, ETFs e BDRs nunca são isentos.
// - Alíquotas: 15% comum, 20% day trade, 20% FII (FII não tem day trade à parte).
// - Day trade = mesmo ativo, mesma data e mesma custódia (o app não guarda a
//   hora); casa min(compras, vendas) do grupo, o resto segue o custo médio.
// - Custo médio corrente por ativo; a taxa da compra soma ao custo e a da venda
//   reduz o ganho. Dentro do dia, as compras de swing entram antes das vendas.
// - IRRF estimado: 0,005% do valor da venda (swing e FII) ou 1% do ganho
//   positivo (day trade). É crédito: abate o imposto e o saldo passa ao mês
//   seguinte.
// - DARF abaixo do mínimo não é gerado e soma ao mês seguinte; vence no último
//   dia útil do mês seguinte (código 6015).
//
// ATENÇÃO: estimativa. Os valores vêm da mesma fonte do desktop; conferir na
// Receita e na nota de corretagem antes de pagar.

import '../models/asset.dart';
import '../models/portfolio_transaction.dart';
import 'cost_basis.dart';
import 'crypto_tax.dart' show darfMinimum;
import 'darf_calendar.dart';

const b3StockExemptionMonthlySales = 20000.0;
const b3CommonRate = 0.15;
const b3DayTradeRate = 0.20;
const b3FiiRate = 0.20;
const b3IrrfCommonRate = 0.00005;
const b3IrrfDayTradeRate = 0.01;
const darfCodeB3 = 6015;

const _eps = 1e-9;

/// Classes que esta apuração cobre; as demais (cripto, exterior, fundos, renda
/// fixa) têm apuração própria ou nenhuma.
const b3AssetClasses = {
  AssetClass.acaoBr,
  AssetClass.etfBr,
  AssetClass.bdr,
  AssetClass.fii,
};

enum B3Bucket { common, dayTrade, fii }

class B3Sale {
  final int? txId;
  final int assetId;
  final String ticker;
  final DateTime date;
  final B3Bucket bucket;
  final bool isUnit;

  /// Só a venda swing de ação (não unit) conta para o limite de R$ 20 mil.
  final bool exemptionEligible;

  /// Quantidade realmente coberta pelo histórico de compras.
  final double quantity;

  /// Valor bruto vendido (base do limite de isenção e do IRRF).
  final double saleValue;

  /// Valor líquido da taxa de venda.
  final double proceeds;
  final double cost;
  final double gain;
  final bool costUnknown;

  const B3Sale({
    required this.txId,
    required this.assetId,
    required this.ticker,
    required this.date,
    required this.bucket,
    required this.isUnit,
    required this.exemptionEligible,
    required this.quantity,
    required this.saleValue,
    required this.proceeds,
    required this.cost,
    required this.gain,
    required this.costUnknown,
  });
}

class B3BucketResult {
  final B3Bucket bucket;

  /// Ganho − perda da gaveta no mês (já sem o ganho isento, na gaveta comum).
  final double result;
  final double lossUsed;

  /// Prejuízo que sobra na gaveta depois deste mês.
  final double lossCarried;
  final double taxable;
  final double rate;
  final double tax;

  const B3BucketResult({
    required this.bucket,
    required this.result,
    required this.lossUsed,
    required this.lossCarried,
    required this.taxable,
    required this.rate,
    required this.tax,
  });
}

class B3Month {
  /// `YYYY-MM`
  final String period;

  /// Valor bruto vendido em swing de ações (base do limite de isenção).
  final double stockSalesTotal;
  final bool stockExempt;
  final double exemptGain;
  final List<B3BucketResult> buckets;
  final double grossTax;
  final double irrfWithheld;
  final double irrfUsed;

  /// Imposto abaixo do mínimo de DARF vindo de meses anteriores.
  final double carriedIn;

  /// Valor do DARF do mês (0 se ficou abaixo do mínimo).
  final double due;
  final double carriedOut;

  /// Último dia útil do mês seguinte (UTC).
  final DateTime dueDate;
  final int darfCode;
  final List<B3Sale> sales;

  const B3Month({
    required this.period,
    required this.stockSalesTotal,
    required this.stockExempt,
    required this.exemptGain,
    required this.buckets,
    required this.grossTax,
    required this.irrfWithheld,
    required this.irrfUsed,
    required this.carriedIn,
    required this.due,
    required this.carriedOut,
    required this.dueDate,
    required this.darfCode,
    required this.sales,
  });
}

enum B3TaxWarningKind { costUnknown }

class B3TaxWarning {
  final B3TaxWarningKind kind;
  final String ticker;
  final DateTime date;

  /// Vendeu mais do que as compras registradas cobrem.
  const B3TaxWarning.costUnknown({required this.ticker, required this.date})
    : kind = B3TaxWarningKind.costUnknown;
}

class B3TaxAssessment {
  final List<B3Month> months;
  final List<B3TaxWarning> warnings;

  const B3TaxAssessment({required this.months, required this.warnings});
}

/// Unit: ação da B3 cujo ticker termina em 11 (TAEE11, KLBN11).
bool _isUnit(Asset asset) =>
    asset.assetClass == AssetClass.acaoBr &&
    asset.ticker.toUpperCase().endsWith('11');

String _periodOf(DateTime date) =>
    '${date.year.toString().padLeft(4, '0')}-${date.month.toString().padLeft(2, '0')}';

DateTime _dayOf(DateTime date) => DateTime(date.year, date.month, date.day);

class _DayGroup {
  double buyQty = 0;
  double buyCost = 0;
  double sellQty = 0;
  double sellGross = 0;
  double sellProceeds = 0;
  int? firstSellTx;
}

/// Apura mês a mês o IR das vendas na B3. As transações podem vir em qualquer
/// ordem e misturar outras classes e tipos (são ignorados).
B3TaxAssessment assessB3Tax(
  List<Asset> assets,
  List<PortfolioTransaction> transactions,
) {
  final byId = {
    for (final asset in assets)
      if (b3AssetClasses.contains(asset.assetClass) && asset.id != null)
        asset.id!: asset,
  };

  // Por ativo, dia a dia; o id da transação desempata (e a ordem de chegada
  // quando não há id).
  final trades = <(int, PortfolioTransaction)>[];
  var index = 0;
  for (final tx in transactions) {
    final isTrade =
        tx.type == TransactionType.compra || tx.type == TransactionType.venda;
    if (!isTrade || !byId.containsKey(tx.assetId)) continue;
    trades.add((index++, tx));
  }
  trades.sort((a, b) {
    final byAsset = a.$2.assetId!.compareTo(b.$2.assetId!);
    if (byAsset != 0) return byAsset;
    final byDay = _dayOf(a.$2.date).compareTo(_dayOf(b.$2.date));
    if (byDay != 0) return byDay;
    final byTx = (a.$2.id ?? 0).compareTo(b.$2.id ?? 0);
    return byTx != 0 ? byTx : a.$1.compareTo(b.$1);
  });

  final warnings = <B3TaxWarning>[];
  final sales = <B3Sale>[];

  final perAsset = <int, List<PortfolioTransaction>>{};
  for (final (_, tx) in trades) {
    perAsset.putIfAbsent(tx.assetId!, () => []).add(tx);
  }

  for (final entry in perAsset.entries) {
    final asset = byId[entry.key]!;
    final isFii = asset.assetClass == AssetClass.fii;
    final unit = _isUnit(asset);
    final exemptionEligible =
        asset.assetClass == AssetClass.acaoBr && !unit;
    final position = CostPosition();

    final perDay = <DateTime, List<PortfolioTransaction>>{};
    for (final tx in entry.value) {
      perDay.putIfAbsent(_dayOf(tx.date), () => []).add(tx);
    }

    for (final dayEntry in perDay.entries) {
      final date = dayEntry.key;
      final groups = <int?, _DayGroup>{};
      for (final tx in dayEntry.value) {
        final group = groups.putIfAbsent(tx.custodiaId, _DayGroup.new);
        final quantity = tx.quantity ?? 0;
        final fee = tx.fee ?? 0;
        if (tx.type == TransactionType.compra) {
          group.buyQty += quantity;
          group.buyCost += tx.totalValue + fee;
        } else {
          group.sellQty += quantity;
          group.sellGross += tx.totalValue;
          group.sellProceeds += tx.totalValue - fee;
          group.firstSellTx ??= tx.id;
        }
      }

      final swingBuys = <(double, double)>[];
      final swingSells = <(int?, double, double, double)>[];

      for (final group in groups.values) {
        final matched = group.buyQty < group.sellQty
            ? group.buyQty
            : group.sellQty;
        if (matched > _eps) {
          final buyShare = matched / group.buyQty;
          final sellShare = matched / group.sellQty;
          final cost = group.buyCost * buyShare;
          final proceeds = group.sellProceeds * sellShare;
          sales.add(
            B3Sale(
              txId: group.firstSellTx,
              assetId: entry.key,
              ticker: asset.ticker,
              date: date,
              bucket: isFii ? B3Bucket.fii : B3Bucket.dayTrade,
              isUnit: unit,
              exemptionEligible: false,
              quantity: matched,
              saleValue: group.sellGross * sellShare,
              proceeds: proceeds,
              cost: cost,
              gain: proceeds - cost,
              costUnknown: false,
            ),
          );
        }
        if (group.buyQty - matched > _eps) {
          swingBuys.add((
            group.buyQty - matched,
            group.buyCost * (1 - matched / group.buyQty),
          ));
        }
        if (group.sellQty - matched > _eps) {
          final rest = 1 - matched / group.sellQty;
          swingSells.add((
            group.firstSellTx,
            group.sellQty - matched,
            group.sellGross * rest,
            group.sellProceeds * rest,
          ));
        }
      }

      for (final (quantity, cost) in swingBuys) {
        position.add(quantity, cost);
      }
      for (final (txId, quantity, gross, proceedsAll) in swingSells) {
        final (cost, covered) = position.take(quantity);
        final costUnknown = covered + costEps < quantity;
        final share = quantity > costEps ? covered / quantity : 0.0;
        if (costUnknown) {
          warnings.add(B3TaxWarning.costUnknown(ticker: asset.ticker, date: date));
        }
        final proceeds = proceedsAll * share;
        sales.add(
          B3Sale(
            txId: txId,
            assetId: entry.key,
            ticker: asset.ticker,
            date: date,
            bucket: isFii ? B3Bucket.fii : B3Bucket.common,
            isUnit: unit,
            exemptionEligible: exemptionEligible,
            quantity: covered,
            saleValue: gross * share,
            proceeds: proceeds,
            cost: cost,
            gain: proceeds - cost,
            costUnknown: costUnknown,
          ),
        );
      }
    }
  }

  final byMonth = <String, List<B3Sale>>{};
  for (final sale in sales) {
    byMonth.putIfAbsent(_periodOf(sale.date), () => []).add(sale);
  }

  final months = <B3Month>[];
  final lossCarried = <B3Bucket, double>{};
  var irrfCredit = 0.0;
  var carried = 0.0;

  for (final period in byMonth.keys.toList()..sort()) {
    final monthSales = byMonth[period]!;
    final year = int.parse(period.substring(0, 4));
    final month = int.parse(period.substring(5, 7));

    final eligible = monthSales.where((s) => s.exemptionEligible);
    final stockSalesTotal = eligible.fold(0.0, (sum, s) => sum + s.saleValue);
    final stockExempt = stockSalesTotal <= b3StockExemptionMonthlySales + _eps;
    final eligibleNet = eligible.fold(0.0, (sum, s) => sum + s.gain);
    final exemptGain = stockExempt && eligibleNet > 0 ? eligibleNet : 0.0;

    final buckets = <B3BucketResult>[];
    var grossTax = 0.0;
    for (final bucket in B3Bucket.values) {
      final inBucket = monthSales.where((s) => s.bucket == bucket).toList();
      if (inBucket.isEmpty) continue;
      var result = inBucket.fold(0.0, (sum, s) => sum + s.gain);
      if (bucket == B3Bucket.common) result -= exemptGain;
      final carriedLoss = lossCarried[bucket] ?? 0.0;
      double lossUsed;
      double taxable;
      if (result > 0) {
        lossUsed = result < carriedLoss ? result : carriedLoss;
        lossCarried[bucket] = carriedLoss - lossUsed;
        taxable = result - lossUsed;
      } else {
        lossUsed = 0;
        taxable = 0;
        lossCarried[bucket] = carriedLoss - result;
      }
      final rate = switch (bucket) {
        B3Bucket.common => b3CommonRate,
        B3Bucket.dayTrade => b3DayTradeRate,
        B3Bucket.fii => b3FiiRate,
      };
      final tax = taxable * rate;
      grossTax += tax;
      buckets.add(
        B3BucketResult(
          bucket: bucket,
          result: result,
          lossUsed: lossUsed,
          lossCarried: lossCarried[bucket]!,
          taxable: taxable,
          rate: rate,
          tax: tax,
        ),
      );
    }

    final irrfWithheld = monthSales.fold(0.0, (sum, s) {
      return sum +
          (s.bucket == B3Bucket.dayTrade
              ? (s.gain > 0 ? s.gain : 0.0) * b3IrrfDayTradeRate
              : s.saleValue * b3IrrfCommonRate);
    });
    irrfCredit += irrfWithheld;
    final irrfUsed = grossTax < irrfCredit ? grossTax : irrfCredit;
    irrfCredit -= irrfUsed;

    final carriedIn = carried;
    final payable = grossTax - irrfUsed + carriedIn;
    final belowMinimum = payable + _eps < darfMinimum;
    final due = belowMinimum ? 0.0 : payable;
    final carriedOut = belowMinimum ? payable : 0.0;
    carried = carriedOut;

    months.add(
      B3Month(
        period: period,
        stockSalesTotal: stockSalesTotal,
        stockExempt: stockExempt,
        exemptGain: exemptGain,
        buckets: buckets,
        grossTax: grossTax,
        irrfWithheld: irrfWithheld,
        irrfUsed: irrfUsed,
        carriedIn: carriedIn,
        due: due,
        carriedOut: carriedOut,
        dueDate: darfDueDate(year, month),
        darfCode: darfCodeB3,
        sales: monthSales,
      ),
    );
  }

  return B3TaxAssessment(months: months, warnings: warnings);
}
