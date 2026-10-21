// IR de cripto (ganho de capital, exchange nacional, pessoa física). Porta de
// `desktop/src-tauri/src/domain/tax/crypto.rs`, separada de qualquer apuração
// de ações porque o DARF tem código próprio (4600), o mínimo e o saldo
// acumulado são próprios e nada aqui compensa prejuízo nem tem IRRF.
//
// Regras (constantes abaixo; no desktop ficam numa tabela editável):
// - Vendas do mês somando até o limite (R$ 35 mil, todas as moedas juntas) →
//   ganho 100% isento. Acima dele, o ganho **do mês inteiro** é tributado
//   (tudo ou nada).
// - Alíquota marginal sobre o ganho tributável acumulado no ano: 15% até
//   R$ 5 mi, 17,5% até R$ 10 mi, 20% até R$ 30 mi, 22,5% acima.
// - Prejuízo do mês dá imposto zero e não compensa nem carrega.
// - Custo médio corrente por ativo (a venda baixa o custo; o "preço médio" da
//   tela de posições não baixa, por isso o cálculo é próprio). A taxa da
//   compra soma ao custo; a da venda reduz o ganho (valor − taxa), mas não o
//   total que conta para o limite de isenção, que é o valor bruto.
// - Ativo que não está em BRL (exchange estrangeira) segue outra regra
//   (Lei 14.754) e só avisa.
//
// ATENÇÃO: os valores vêm de fontes secundárias (blogs e consultorias), as
// mesmas do desktop; conferir na Receita (GCAP) antes de confiar.

import '../models/asset.dart';
import '../models/portfolio_transaction.dart';
import 'darf_calendar.dart';

const cryptoMonthlyExemption = 35000.0;
const darfMinimum = 10.0;
const darfCodeCrypto = 4600;

/// Faixas `(teto do ganho tributável acumulado no ano, alíquota)`; cada faixa
/// tributa só a parte que cabe nela.
const cryptoTiers = <(double, double)>[
  (5000000.0, 0.15),
  (10000000.0, 0.175),
  (30000000.0, 0.20),
  (double.infinity, 0.225),
];

const _eps = 1e-9;

/// Uma venda de cripto, com o custo médio já baixado.
class CryptoSale {
  final int? txId;
  final int assetId;
  final String ticker;
  final DateTime date;

  /// Quantidade coberta pelo histórico de compras conhecido.
  final double quantity;

  /// Valor de venda (base do limite de isenção), só da parte com custo conhecido.
  final double saleValue;

  /// Valor de venda − taxa (proporcional à parte coberta): a base do ganho.
  final double proceeds;
  final double cost;
  final double gain;

  /// Vendeu mais do que o histórico de compras conhecido: o ganho só
  /// considera a parte com custo conhecido.
  final bool costUnknown;

  const CryptoSale({
    required this.txId,
    required this.assetId,
    required this.ticker,
    required this.date,
    required this.quantity,
    required this.saleValue,
    required this.proceeds,
    required this.cost,
    required this.gain,
    required this.costUnknown,
  });
}

class CryptoMonth {
  /// `YYYY-MM`
  final String period;

  /// Total vendido no mês (base do limite de isenção).
  final double salesTotal;
  final bool exempt;

  /// Ganho do mês (ganhos − perdas).
  final double gain;

  /// Alíquota efetiva do mês (a da 1ª faixa quando não há ganho tributável).
  final double rate;
  final double tax;

  /// Imposto abaixo do mínimo de DARF vindo de meses anteriores.
  final double carriedIn;

  /// Valor do DARF do mês (0 se ficou abaixo do mínimo).
  final double due;
  final double carriedOut;

  /// Último dia útil do mês seguinte (UTC).
  final DateTime dueDate;
  final int darfCode;
  final List<CryptoSale> sales;

  const CryptoMonth({
    required this.period,
    required this.salesTotal,
    required this.exempt,
    required this.gain,
    required this.rate,
    required this.tax,
    required this.carriedIn,
    required this.due,
    required this.carriedOut,
    required this.dueDate,
    required this.darfCode,
    required this.sales,
  });
}

enum CryptoTaxWarningKind {
  /// Vendeu mais do que o histórico de compras conhecido (`ticker`, `date`).
  costUnknown,

  /// Venda de cripto fora de BRL no mês (`period`): não é apurada aqui.
  salesNonBrl,
}

class CryptoTaxWarning {
  final CryptoTaxWarningKind kind;
  final String? ticker;
  final DateTime? date;
  final String? period;

  const CryptoTaxWarning.costUnknown({
    required String this.ticker,
    required DateTime this.date,
  }) : kind = CryptoTaxWarningKind.costUnknown,
       period = null;

  const CryptoTaxWarning.salesNonBrl({required String this.period})
    : kind = CryptoTaxWarningKind.salesNonBrl,
      ticker = null,
      date = null;
}

class CryptoTaxAssessment {
  final List<CryptoMonth> months;
  final List<CryptoTaxWarning> warnings;

  const CryptoTaxAssessment({required this.months, required this.warnings});
}

String _periodOf(DateTime date) =>
    '${date.year.toString().padLeft(4, '0')}-${date.month.toString().padLeft(2, '0')}';

/// Imposto de `taxable` de ganho que começa em `before` (ganho tributável já
/// acumulado no ano) sobre as faixas; a alíquota é marginal.
double _tieredTax(double before, double taxable) {
  final end = before + taxable;
  var floor = 0.0;
  var tax = 0.0;
  for (final (ceiling, rate) in cryptoTiers) {
    final from = before > floor ? before : floor;
    final to = end < ceiling ? end : ceiling;
    if (to > from) tax += (to - from) * rate;
    floor = ceiling;
  }
  return tax;
}

/// Apura mês a mês o IR das vendas de cripto em BRL. As transações podem vir
/// em qualquer ordem e misturar outras classes e tipos (são ignorados:
/// transferência, aporte, retirada e provento não mexem em custo nem em venda).
CryptoTaxAssessment assessCryptoTax(
  List<Asset> assets,
  List<PortfolioTransaction> transactions,
) {
  final warnings = <CryptoTaxWarning>[];
  final nonBrlPeriods = <String>{};
  final cryptoById = {
    for (final asset in assets)
      if (asset.assetClass == AssetClass.cripto && asset.id != null)
        asset.id!: asset,
  };

  final trades = <(int, PortfolioTransaction)>[];
  var index = 0;
  for (final tx in transactions) {
    final asset = cryptoById[tx.assetId];
    final isTrade =
        tx.type == TransactionType.compra || tx.type == TransactionType.venda;
    if (asset == null || !isTrade) continue;
    if (asset.currency != 'BRL') {
      if (tx.type == TransactionType.venda) {
        nonBrlPeriods.add(_periodOf(tx.date));
      }
      continue;
    }
    trades.add((index++, tx));
  }
  // Por ativo, dentro do dia as compras antes das vendas (a ordem de
  // gravação desempata), como na apuração do desktop.
  trades.sort((a, b) {
    final x = a.$2, y = b.$2;
    final byAsset = x.assetId!.compareTo(y.assetId!);
    if (byAsset != 0) return byAsset;
    final byDay = DateTime(
      x.date.year,
      x.date.month,
      x.date.day,
    ).compareTo(DateTime(y.date.year, y.date.month, y.date.day));
    if (byDay != 0) return byDay;
    final xSale = x.type == TransactionType.venda ? 1 : 0;
    final ySale = y.type == TransactionType.venda ? 1 : 0;
    if (xSale != ySale) return xSale.compareTo(ySale);
    final byId = (x.id ?? 0).compareTo(y.id ?? 0);
    return byId != 0 ? byId : a.$1.compareTo(b.$1);
  });

  final byMonth = <String, List<CryptoSale>>{};
  final quantityOf = <int, double>{};
  final costOf = <int, double>{};
  for (final (_, tx) in trades) {
    final asset = cryptoById[tx.assetId]!;
    final id = asset.id!;
    final quantity = tx.quantity ?? 0;
    if (tx.type == TransactionType.compra) {
      quantityOf[id] = (quantityOf[id] ?? 0) + quantity;
      costOf[id] = (costOf[id] ?? 0) + tx.totalValue + (tx.fee ?? 0);
      continue;
    }

    final held = quantityOf[id] ?? 0;
    final available = held > 0 ? held : 0.0;
    final covered = quantity < available ? quantity : available;
    final average = held > _eps ? (costOf[id] ?? 0) / held : 0.0;
    final cost = average * covered;
    quantityOf[id] = held - covered;
    costOf[id] = (costOf[id] ?? 0) - cost;
    // Posição zerada → o custo médio recomeça do zero na próxima compra.
    if (quantityOf[id]! <= _eps) {
      quantityOf[id] = 0;
      costOf[id] = 0;
    }

    final costUnknown = covered + _eps < quantity;
    final share = quantity > _eps ? covered / quantity : 0.0;
    if (costUnknown) {
      warnings.add(
        CryptoTaxWarning.costUnknown(ticker: asset.ticker, date: tx.date),
      );
    }
    final saleValue = tx.totalValue * share;
    final proceeds = (tx.totalValue - (tx.fee ?? 0)) * share;
    byMonth
        .putIfAbsent(_periodOf(tx.date), () => [])
        .add(
          CryptoSale(
            txId: tx.id,
            assetId: id,
            ticker: asset.ticker,
            date: tx.date,
            quantity: covered,
            saleValue: saleValue,
            proceeds: proceeds,
            cost: cost,
            gain: proceeds - cost,
            costUnknown: costUnknown,
          ),
        );
  }

  final months = <CryptoMonth>[];
  var carried = 0.0;
  final taxableByYear = <int, double>{};
  for (final period in byMonth.keys.toList()..sort()) {
    final sales = byMonth[period]!;
    final year = int.parse(period.substring(0, 4));
    final month = int.parse(period.substring(5, 7));
    final salesTotal = sales.fold(0.0, (sum, s) => sum + s.saleValue);
    final gain = sales.fold(0.0, (sum, s) => sum + s.gain);
    final exempt = salesTotal <= cryptoMonthlyExemption + _eps;
    final taxable = exempt ? 0.0 : (gain > 0 ? gain : 0.0);

    // A tabela corre sobre o ganho tributável acumulado no ano: este mês
    // ocupa as faixas a partir de onde os meses anteriores pararam.
    final yearTotal = taxableByYear[year] ?? 0.0;
    final tax = _tieredTax(yearTotal, taxable);
    taxableByYear[year] = yearTotal + taxable;
    final rate = taxable > _eps ? tax / taxable : cryptoTiers.first.$2;

    final carriedIn = carried;
    final payable = tax + carriedIn;
    final belowMinimum = payable + _eps < darfMinimum;
    final due = belowMinimum ? 0.0 : payable;
    final carriedOut = belowMinimum ? payable : 0.0;
    carried = carriedOut;

    months.add(
      CryptoMonth(
        period: period,
        salesTotal: salesTotal,
        exempt: exempt,
        gain: gain,
        rate: rate,
        tax: tax,
        carriedIn: carriedIn,
        due: due,
        carriedOut: carriedOut,
        dueDate: darfDueDate(year, month),
        darfCode: darfCodeCrypto,
        sales: sales,
      ),
    );
  }

  for (final period in nonBrlPeriods.toList()..sort()) {
    warnings.add(CryptoTaxWarning.salesNonBrl(period: period));
  }
  return CryptoTaxAssessment(months: months, warnings: warnings);
}
