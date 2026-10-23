// Come-cotas dos fundos abertos: IR cobrado do cotista em cotas, na última data útil
// de maio e de novembro, sobre o ganho desde a compra ou desde o último come-cotas.
// Porta de `desktop/src-tauri/src/domain/tax/come_cotas.rs`: aqui só a ESTIMATIVA do
// próximo desconto.
//
// Regras:
// - O usuário diz o tipo de cada fundo: longo prazo (15%), curto prazo (20%) ou
//   ações (sem come-cotas).
// - Base por cota = o maior entre o custo médio e o valor da cota na última data de
//   come-cotas que já passou (se a posição já existia); ganho por cota =
//   max(0, valor atual − base).
// - Imposto = ganho × cotas × alíquota; cotas descontadas = imposto ÷ valor da cota.
// - Usa o último valor de cota lançado (não projeta até a data) e a posição de hoje.
//
// ATENÇÃO: as regras vêm de fontes secundárias; conferir na Receita antes de confiar.

import '../models/fund.dart';
import 'cost_basis.dart' show costEps;
import 'darf_calendar.dart' show lastBusinessDay;

/// Alíquotas do come-cotas; espelham `COME_COTAS_RATE_*` de `rules.rs` do desktop
/// (o mobile não tem `tax_rules`).
const comeCotasRateLong = 0.15;
const comeCotasRateShort = 0.20;

/// Valor de cota com mais dias que isto avisa que a estimativa pode estar velha.
const staleQuotaDays = 35;

/// O valor da cota da última data de come-cotas só serve de base se foi lançado
/// até tantos dias antes dela.
const _basisQuotaWindowDays = 45;

/// Um desconto registrado conta como o da última data de come-cotas se caiu entre
/// tantos dias antes e depois dela.
const _deductionDaysBefore = 5;
const _deductionDaysAfter = 10;

/// Valor de uma cota numa data, em BRL.
class QuotaValue {
  final DateTime date;
  final double value;

  const QuotaValue(this.date, this.value);
}

/// A posição de um fundo, no formato da estimativa.
class FundHolding {
  final int assetId;
  final String ticker;
  final String name;
  final FundTerm? term;
  final double quantity;

  /// Custo médio por cota, em BRL.
  final double averageCost;

  /// Data da primeira compra (para saber se a posição já existia no último
  /// come-cotas).
  final DateTime? firstBuy;

  /// Valores de cota lançados, em qualquer ordem.
  final List<QuotaValue> quotas;

  /// Descontos de come-cotas já registrados (transação `come_cotas`): data e valor
  /// da cota de cada um.
  final List<QuotaValue> deductions;

  const FundHolding({
    required this.assetId,
    required this.ticker,
    required this.name,
    required this.term,
    required this.quantity,
    required this.averageCost,
    required this.firstBuy,
    required this.quotas,
    this.deductions = const [],
  });
}

enum EstimateStatus {
  estimated,

  /// Fundo de ações: sem come-cotas.
  exempt,

  /// O usuário ainda não disse o tipo do fundo.
  noTerm,

  /// Nenhum valor de cota lançado (até hoje).
  noQuota,
  noPosition,
}

enum BasisSource {
  /// Custo médio da cota.
  cost,

  /// Valor da cota no último come-cotas (maior que o custo).
  lastComeCotas,
}

class ComeCotasEstimate {
  final int assetId;
  final String ticker;
  final String name;
  EstimateStatus status = EstimateStatus.noPosition;

  /// Próxima data de come-cotas (última data útil de maio ou de novembro).
  final DateTime referenceDate;
  final double quotas;
  double? quotaValue;
  DateTime? quotaDate;
  double? basisPerQuota;
  BasisSource? basisSource;

  /// Ganho tributável total, em BRL.
  double gain = 0;
  double rate = 0;
  double tax = 0;
  double quotasDeducted = 0;

  /// O último valor de cota lançado é antigo ([staleQuotaDays]).
  bool staleQuota = false;

  /// A última data de come-cotas passou com a posição já existente e sem nenhum
  /// desconto registrado em volta dela: o usuário provavelmente ainda não lançou a
  /// transação `come_cotas`.
  DateTime? lastReferenceUnrecorded;

  ComeCotasEstimate({
    required this.assetId,
    required this.ticker,
    required this.name,
    required this.referenceDate,
    required this.quotas,
  });
}

DateTime _day(DateTime date) => DateTime.utc(date.year, date.month, date.day);

int _daysBetween(DateTime later, DateTime earlier) =>
    _day(later).difference(_day(earlier)).inDays;

/// Datas de come-cotas de um ano: última data útil de maio e de novembro.
List<DateTime> _referenceDates(int year) => [
  lastBusinessDay(year, 5),
  lastBusinessDay(year, 11),
];

/// Primeira data de come-cotas a partir de [today] (inclusive).
DateTime nextReferenceDate(DateTime today) {
  final day = _day(today);
  return [
    ..._referenceDates(day.year),
    ..._referenceDates(day.year + 1),
  ].firstWhere((date) => !date.isBefore(day));
}

/// Última data de come-cotas antes de [today] (estritamente).
DateTime lastReferenceDate(DateTime today) {
  final day = _day(today);
  return [
    ..._referenceDates(day.year - 1),
    ..._referenceDates(day.year),
  ].where((date) => date.isBefore(day)).reduce((a, b) => a.isAfter(b) ? a : b);
}

QuotaValue? _latest(Iterable<QuotaValue> quotas) {
  QuotaValue? best;
  for (final quota in quotas) {
    if (best == null || quota.date.isAfter(best.date)) best = quota;
  }
  return best;
}

/// Estima o próximo come-cotas de cada fundo. Fundo sem tipo, sem valor de cota ou
/// sem posição vem sem imposto e com o motivo em `status`.
List<ComeCotasEstimate> estimateComeCotas(
  List<FundHolding> holdings,
  DateTime today,
) {
  final now = _day(today);
  final referenceDate = nextReferenceDate(now);
  final lastReference = lastReferenceDate(now);
  final estimates = <ComeCotasEstimate>[];

  for (final holding in holdings) {
    final estimate = ComeCotasEstimate(
      assetId: holding.assetId,
      ticker: holding.ticker,
      name: holding.name,
      referenceDate: referenceDate,
      quotas: holding.quantity > 0 ? holding.quantity : 0,
    );
    estimates.add(estimate);
    if (holding.quantity <= costEps) continue;
    final term = holding.term;
    if (term == null) {
      estimate.status = EstimateStatus.noTerm;
      continue;
    }
    // O último valor lançado até hoje (valor com data futura não conta).
    final latest = _latest(holding.quotas.where((q) => !_day(q.date).isAfter(now)));
    if (latest == null) {
      estimate.status = EstimateStatus.noQuota;
      continue;
    }
    estimate.quotaValue = latest.value;
    estimate.quotaDate = _day(latest.date);
    estimate.staleQuota = _daysBetween(now, latest.date) > staleQuotaDays;

    if (term == FundTerm.equity) {
      estimate.status = EstimateStatus.exempt;
      continue;
    }
    final rate = term == FundTerm.longTerm
        ? comeCotasRateLong
        : comeCotasRateShort;

    // Se a posição já existia no último come-cotas, o ganho antes dele já foi
    // tributado: a base sobe para o valor da cota daquela data (quando é maior que
    // o custo). O desconto registrado dá esse valor com exatidão e vale mais que a
    // busca na tabela de cotas.
    final firstBuy = holding.firstBuy;
    final heldThen = firstBuy != null && !_day(firstBuy).isAfter(lastReference);
    final recorded = _latest(
      holding.deductions.where((q) {
        final delta = _daysBetween(q.date, lastReference);
        return delta >= -_deductionDaysBefore && delta <= _deductionDaysAfter;
      }),
    );
    final tableQuota = _latest(
      holding.quotas.where((q) {
        final delta = _daysBetween(lastReference, q.date);
        return delta >= 0 && delta <= _basisQuotaWindowDays;
      }),
    );
    final lastValue = (recorded ?? tableQuota)?.value;
    final BasisSource basisSource;
    final double basis;
    if (lastValue != null &&
        (recorded != null || heldThen) &&
        lastValue > holding.averageCost) {
      basis = lastValue;
      basisSource = BasisSource.lastComeCotas;
    } else {
      basis = holding.averageCost;
      basisSource = BasisSource.cost;
    }
    if (recorded == null && heldThen) {
      estimate.lastReferenceUnrecorded = lastReference;
    }

    final perQuota = latest.value - basis;
    final gain = (perQuota > 0 ? perQuota : 0.0) * holding.quantity;
    final tax = gain * rate;
    estimate
      ..status = EstimateStatus.estimated
      ..basisPerQuota = basis
      ..basisSource = basisSource
      ..gain = gain
      ..rate = rate
      ..tax = tax
      ..quotasDeducted = latest.value > 0 ? tax / latest.value : 0;
  }
  return estimates;
}
