import '../models/asset.dart';
import '../models/portfolio_transaction.dart';
import 'fx_history.dart';
import 'profitability_error.dart';
import 'twr.dart';

// Porta de `compute_profitability` (`desktop/src-tauri/src/commands/profitability.rs`, Fase
// 10.3) sem banco: tudo entra por parâmetro. A carteira medida é a sub-carteira das classes
// com preço histórico do Yahoo; compra, venda e provento dela são os fluxos externos do
// Dietz Modificado (o app não tem sub-ledger de caixa, então uma compra equivale a um
// aporte). Bruta de imposto: o mobile não registra DARF pago.

/// Classes com fechamento mensal pelo Yahoo (as mesmas do desktop, menos cripto, que o
/// mobile ainda não coleta).
const profitabilityAssetClasses = {
  AssetClass.acaoBr,
  AssetClass.fii,
  AssetClass.etfBr,
  AssetClass.etfRendaFixa,
  AssetClass.bdr,
  AssetClass.acaoInternacional,
  AssetClass.reit,
  AssetClass.etfUs,
  AssetClass.metal,
};

class MonthlyReturn {
  final String yearMonth;
  final double bmv;
  final double emv;
  final double cfTotal;
  final double rMonthPct;
  final double rCumulativePct;

  const MonthlyReturn({
    required this.yearMonth,
    required this.bmv,
    required this.emv,
    required this.cfTotal,
    required this.rMonthPct,
    required this.rCumulativePct,
  });
}

/// Ativo que ficou fora do cálculo por ter desdobramento depois da 1ª transação dele: sem
/// registrar o evento, a quantidade antiga × preço ajustado erraria o valor sem aviso.
class ExcludedAsset {
  final Asset asset;
  final DateTime splitDate;

  const ExcludedAsset({required this.asset, required this.splitDate});
}

class ProfitabilityResult {
  final List<MonthlyReturn> months;
  final List<ExcludedAsset> excluded;

  const ProfitabilityResult({required this.months, required this.excluded});
}

DateTime _day(DateTime d) => DateTime.utc(d.year, d.month, d.day);

int _daysInMonth(int year, int month) => DateTime.utc(year, month + 1, 0).day;

/// Dia 1 ≈ quase todo o mês "trabalhado"; último dia ≈ 0 (dia 10 de 31 → 21/31).
double _flowWeight(DateTime date) {
  final days = _daysInMonth(date.year, date.month);
  return date.day / days;
}

String _monthKey(int year, int month) => '${year.toString().padLeft(4, '0')}-${month.toString().padLeft(2, '0')}';

/// Rentabilidade mês a mês da carteira medida, em reais.
///
/// [monthCloses] é `ticker → 'AAAA-MM' → fechamento` (do Yahoo, já ajustado a splits);
/// [splits] é `ticker → datas de desdobramento`; [fxToBrl] é `moeda → série em reais` (BRL
/// não precisa de série). [today] fecha o mês corrente: o último mês usa o preço mais
/// recente e o câmbio de hoje, não do último dia do calendário.
///
/// Recusa com [ProfitabilityError] (nada de número parcial): ativo em carteira sem
/// fechamento do mês, ou moeda sem série de câmbio.
ProfitabilityResult computeProfitability({
  required List<Asset> assets,
  required List<PortfolioTransaction> transactions,
  required Map<String, Map<String, double>> monthCloses,
  required Map<String, List<DateTime>> splits,
  required Map<String, FxHistory> fxToBrl,
  required DateTime today,
}) {
  final byId = {for (final a in assets) a.id!: a};

  const relevantTypes = {TransactionType.compra, TransactionType.venda, TransactionType.provento};
  final indexed = <(int, PortfolioTransaction)>[
    for (var i = 0; i < transactions.length; i++)
      if (transactions[i].assetId != null &&
          relevantTypes.contains(transactions[i].type) &&
          byId[transactions[i].assetId] != null &&
          profitabilityAssetClasses.contains(byId[transactions[i].assetId]!.assetClass))
        (i, transactions[i]),
  ]..sort((a, b) {
      final byDate = _day(a.$2.date).compareTo(_day(b.$2.date));
      return byDate != 0 ? byDate : a.$1.compareTo(b.$1);
    });

  // Ativo com split na data da 1ª transação ou depois: fora (a data igual fica do lado
  // conservador, porque não se sabe de que lado do evento a compra caiu).
  final firstDayByAsset = <int, DateTime>{};
  for (final (_, tx) in indexed) {
    firstDayByAsset.putIfAbsent(tx.assetId!, () => _day(tx.date));
  }
  final excluded = <ExcludedAsset>[];
  final excludedIds = <int>{};
  for (final entry in firstDayByAsset.entries) {
    final asset = byId[entry.key]!;
    final after = [
      for (final s in splits[asset.ticker] ?? const <DateTime>[])
        if (_day(s).isAfter(entry.value)) s,
    ]..sort();
    if (after.isNotEmpty) {
      excluded.add(ExcludedAsset(asset: asset, splitDate: after.first));
      excludedIds.add(entry.key);
    }
  }
  excluded.sort((a, b) => a.asset.ticker.compareTo(b.asset.ticker));

  final relevant = [
    for (final (_, tx) in indexed)
      if (!excludedIds.contains(tx.assetId)) tx,
  ];
  if (relevant.isEmpty) return ProfitabilityResult(months: const [], excluded: excluded);

  double toBrl(double value, String currency, DateTime date) {
    if (currency == 'BRL') return value;
    final rate = fxToBrl[currency]?.rateOn(date);
    if (rate == null) throw ProfitabilityError(ProfitabilityErrorCode.missingFx, {'currency': currency});
    return value * rate;
  }

  final todayDay = _day(today);
  final first = relevant.first.date;
  final holdings = <int, double>{};
  var bmv = 0.0;
  final monthlyR = <double>[];
  final months = <MonthlyReturn>[];

  var year = first.year;
  var month = first.month;
  while (year < todayDay.year || (year == todayDay.year && month <= todayDay.month)) {
    final key = _monthKey(year, month);
    final flows = <CashFlow>[];

    for (final tx in relevant) {
      if (tx.date.year != year || tx.date.month != month) continue;
      final asset = byId[tx.assetId]!;
      final weight = _flowWeight(tx.date);
      final value = toBrl(tx.totalValue, asset.currency, tx.date);
      final fee = toBrl(tx.fee ?? 0.0, asset.currency, tx.date);
      switch (tx.type) {
        case TransactionType.compra:
          holdings[asset.id!] = (holdings[asset.id!] ?? 0.0) + (tx.quantity ?? 0.0);
          flows.add(CashFlow(amount: flowAmount(FlowKind.contribution, value), weight: weight));
        case TransactionType.venda:
          holdings[asset.id!] = (holdings[asset.id!] ?? 0.0) - (tx.quantity ?? 0.0);
          flows.add(CashFlow(amount: flowAmount(FlowKind.withdrawal, value - fee), weight: weight));
        case TransactionType.provento:
          flows.add(CashFlow(amount: flowAmount(FlowKind.income, value), weight: weight));
        default:
          break;
      }
    }

    final monthEnd = DateTime.utc(year, month, _daysInMonth(year, month));
    final target = monthEnd.isBefore(todayDay) ? monthEnd : todayDay;
    var emv = 0.0;
    for (final entry in holdings.entries) {
      if (entry.value.abs() < 1e-9) continue;
      final asset = byId[entry.key]!;
      final price = monthCloses[asset.ticker]?[key];
      if (price == null) {
        throw ProfitabilityError(ProfitabilityErrorCode.missingMonthPrice, {
          'ticker': asset.ticker,
          'month': key,
        });
      }
      emv += toBrl(entry.value * price, asset.currency, target);
    }

    double r;
    if (bmv == 0.0 && emv == 0.0 && flows.isEmpty) {
      r = 0.0;
    } else {
      try {
        r = modifiedDietz(bmv, emv, flows);
      } on ProfitabilityError catch (e) {
        // Nada investido durante o mês (ex.: compra no último dia sem saldo anterior):
        // o mês rende 0 em vez de derrubar a série.
        if (e.code == ProfitabilityErrorCode.zeroDenominator) rethrow;
        r = 0.0;
      }
    }

    monthlyR.add(r);
    months.add(
      MonthlyReturn(
        yearMonth: key,
        bmv: bmv,
        emv: emv,
        cfTotal: flows.fold(0.0, (sum, cf) => sum + cf.amount),
        rMonthPct: r * 100.0,
        rCumulativePct: chain(monthlyR) * 100.0,
      ),
    );
    bmv = emv;

    month += 1;
    if (month == 13) {
      month = 1;
      year += 1;
    }
  }

  return ProfitabilityResult(months: months, excluded: excluded);
}
