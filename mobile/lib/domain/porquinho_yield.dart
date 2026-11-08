import 'dart:math' as math;

import 'darf_calendar.dart' show easter;
import 'finance_error.dart';
import 'general_finance.dart' show toIsoDay;
import 'iof.dart';

// Estimativa do rendimento de cada resgate do Porquinho (CDB de liquidez diária ≈ 100% do CDI)
// a partir só do que o extrato mostra: as aplicações e o que o banco pagou em cada resgate.
// Porta de `desktop/src-tauri/src/domain/porquinho_yield.rs` e de `estimate_in`
// (`commands/porquinho_estimate.rs`).
//
// O CDI diário não está no app (só a Selic meta diária), então vale `meta − 0,10 p.p.`, a relação
// histórica entre as duas. O título rende nos dias úteis, 252 ao ano: o fator de um dia é
// `1 + ((1 + cdi/100)^(1/252) − 1) × pct`. No desktop, nos 23 resgates de 2025 que a B3 explica, a
// 100% do CDI o erro mediano foi de 0,4% por resgate.
//
// Cada resgate consome as aplicações mais antigas primeiro (PEPS). O valor pago `X` é líquido de IOF e
// IR, e cada lote rende `k = 1 + g × (1 − iof) × (1 − ir)` por real de principal (g = fator − 1), então o
// principal de cada lote sai de `X ÷ k` e o resto da conta é direta. IOF pela tabela do decreto
// (`iof.dart`) e IR de 22,5% sobre o que sobra do IOF (a tabela regressiva não é modelada, como no desktop).

/// IR retido no resgate do CDB: primeira faixa da tabela regressiva (até 180 dias), alíquota única.
const porquinhoIrRate = 0.225;

/// Diferença histórica entre a Selic meta e o CDI, em pontos percentuais ao ano.
const cdiBelowSelicMetaPp = 0.10;

/// Maior fração do CDI aceita (2,0 = 200%).
const maxPctOfCdi = 2.0;

const _businessDaysPerYear = 252.0;

/// Rendimento mínimo, em reais, para valer uma proposta.
const _minYield = 0.01;

enum LegKind {
  /// Dinheiro que entrou no porquinho: vira um lote.
  application,

  /// Dinheiro que saiu do porquinho: o que o banco pagou, líquido de IOF e IR.
  redemption,
}

class Leg {
  final DateTime date;
  final LegKind kind;
  final double amount;

  const Leg({required this.date, required this.kind, required this.amount});
}

/// Meta Selic em vigor a partir de [date], em % ao ano.
class SelicPoint {
  final DateTime date;
  final double meta;

  const SelicPoint(this.date, this.meta);
}

class RedemptionEstimate {
  /// Posição do resgate na lista de fluxos recebida.
  final int legIndex;
  final DateTime date;

  /// O que o banco pagou (`principal + netYield`).
  final double paid;
  final double principal;
  final double grossYield;
  final double iof;
  final double ir;
  final double netYield;

  /// Dias aplicados, ponderados pelo principal de cada lote (só da parte com aplicação conhecida).
  final int? holdingDays;

  /// Toda a quantia paga tem aplicação nos fluxos recebidos; falso quando o resgate passa do que se conhece.
  final bool fullyCovered;

  const RedemptionEstimate({
    required this.legIndex,
    required this.date,
    required this.paid,
    required this.principal,
    required this.grossYield,
    required this.iof,
    required this.ir,
    required this.netYield,
    required this.holdingDays,
    required this.fullyCovered,
  });
}

double _round2(double value) => (value * 100).roundToDouble() / 100;

/// Feriado nacional: fixo, ou móvel contado a partir da Páscoa. O Dia da Consciência Negra (20/11) é
/// nacional desde 2024 (Lei 14.759/2023). Porta de `is_national_holiday` do desktop; difere do
/// `isBusinessDay` de `darf_calendar.dart`, que também folga em 31/12 e em 20/11 de todo ano.
bool _isNationalHoliday(DateTime date) {
  const fixed = [(1, 1), (4, 21), (5, 1), (9, 7), (10, 12), (11, 2), (11, 15), (12, 25)];
  if (fixed.contains((date.month, date.day))) return true;
  if (date.year >= 2024 && date.month == 11 && date.day == 20) return true;
  final sunday = easter(date.year);
  // Carnaval (segunda e terça), Sexta-feira Santa e Corpus Christi.
  return const [-48, -47, -2, 60].any((offset) {
    final holiday = sunday.add(Duration(days: offset));
    return holiday.year == date.year && holiday.month == date.month && holiday.day == date.day;
  });
}

bool _isCdiBusinessDay(DateTime date) =>
    date.weekday != DateTime.saturday && date.weekday != DateTime.sunday && !_isNationalHoliday(date);

/// Meta Selic em vigor no dia (a última ≤ dia); `null` antes do primeiro ponto. [selic] vem ordenada por data.
double? metaOn(List<SelicPoint> selic, DateTime day) {
  var low = 0;
  var high = selic.length;
  while (low < high) {
    final mid = (low + high) ~/ 2;
    if (selic[mid].date.isAfter(day)) {
      high = mid;
    } else {
      low = mid + 1;
    }
  }
  return low == 0 ? null : selic[low - 1].meta;
}

double _dailyCdi(double metaPct) =>
    math.pow(1 + (metaPct - cdiBelowSelicMetaPp) / 100, 1 / _businessDaysPerYear).toDouble() - 1;

/// Fator de rendimento de [from] (exclusive) até [to] (inclusive), nos dias úteis, a [pctOfCdi] do CDI
/// (1,0 = 100%). `null` quando falta a meta Selic em algum dia do trecho.
double? accrualFactor(List<SelicPoint> selic, DateTime from, DateTime to, double pctOfCdi) {
  var factor = 1.0;
  var day = from.add(const Duration(days: 1));
  while (!day.isAfter(to)) {
    if (_isCdiBusinessDay(day)) {
      final meta = metaOn(selic, day);
      if (meta == null) return null;
      factor *= 1 + _dailyCdi(meta) * pctOfCdi;
    }
    day = day.add(const Duration(days: 1));
  }
  return factor;
}

class _Lot {
  final DateTime date;
  double remaining;

  _Lot(this.date, this.remaining);
}

/// Estima o rendimento de cada resgate. [legs] vem em ordem cronológica; no mesmo dia, as aplicações antes
/// dos resgates ficam a cargo de quem monta a lista. Falta de Selic num trecho é um erro, nunca uma estimativa
/// com buraco.
List<RedemptionEstimate> estimateRedemptions({
  required List<Leg> legs,
  required List<SelicPoint> selic,
  required double pctOfCdi,
  double irRate = porquinhoIrRate,
}) {
  final lots = <_Lot>[];
  final out = <RedemptionEstimate>[];
  for (var legIndex = 0; legIndex < legs.length; legIndex++) {
    final leg = legs[legIndex];
    if (leg.kind == LegKind.application) {
      lots.add(_Lot(leg.date, leg.amount));
      continue;
    }
    var remainingPaid = leg.amount;
    var gross = 0.0;
    var iof = 0.0;
    var ir = 0.0;
    var weightedDays = 0.0;
    var knownPrincipal = 0.0;
    for (final lot in lots) {
      if (remainingPaid <= 1e-9) break;
      if (lot.remaining <= 1e-9 || lot.date.isAfter(leg.date)) continue;
      final factor = accrualFactor(selic, lot.date, leg.date, pctOfCdi);
      if (factor == null) {
        throw FinanceArgumentError(FinanceErrorCode.selicNotCovered, {
          'from': toIsoDay(lot.date),
          'to': toIsoDay(leg.date),
        });
      }
      final days = leg.date.difference(lot.date).inDays;
      final iofRate = iofRateForDays(days);
      final gain = factor - 1;
      final netMultiplier = 1 + gain * (1 - iofRate) * (1 - irRate);
      final take = math.min(lot.remaining, remainingPaid / netMultiplier);
      lot.remaining -= take;
      remainingPaid -= take * netMultiplier;
      knownPrincipal += take;
      gross += take * gain;
      iof += take * gain * iofRate;
      ir += take * gain * (1 - iofRate) * irRate;
      weightedDays += days * take;
    }
    final fullyCovered = remainingPaid <= 0.005;
    final grossRounded = _round2(gross);
    final iofRounded = _round2(iof);
    final irRounded = _round2(ir);
    final netYield = _round2(grossRounded - iofRounded - irRounded);
    // O que passou dos lotes conhecidos é principal sem aplicação nos fluxos: sem rendimento estimado, e
    // `principal` é sempre o pago menos o rendimento líquido, para a soma fechar no centavo.
    out.add(
      RedemptionEstimate(
        legIndex: legIndex,
        date: leg.date,
        paid: leg.amount,
        principal: _round2(leg.amount - netYield),
        grossYield: grossRounded,
        iof: iofRounded,
        ir: irRounded,
        netYield: netYield,
        holdingDays: knownPrincipal > 1e-9 ? math.max(0, (weightedDays / knownPrincipal).round()) : null,
        fullyCovered: fullyCovered,
      ),
    );
  }
  return out;
}

/// O que a estimativa propõe lançar e o que deixou de fora.
class YieldProposals {
  final List<RedemptionEstimate> proposals;
  final int redemptions;

  /// Resgates de um dia que já tem rendimento lançado.
  final int skippedCoveredDay;

  /// Rendimento estimado abaixo de R$ 0,01 (por exemplo, resgate no mesmo dia da aplicação).
  final int skippedNoYield;

  const YieldProposals({
    required this.proposals,
    required this.redemptions,
    required this.skippedCoveredDay,
    required this.skippedNoYield,
  });
}

/// Porta de `estimate_in`: estima e separa o que vira proposta. [coveredDays] (`YYYY-MM-DD`) são os dias da
/// conta que já têm rendimento lançado.
YieldProposals proposeYields({
  required List<Leg> legs,
  required List<SelicPoint> selic,
  required double pctOfCdi,
  required Set<String> coveredDays,
}) {
  if (!pctOfCdi.isFinite || pctOfCdi <= 0 || pctOfCdi > maxPctOfCdi) {
    throw FinanceArgumentError(FinanceErrorCode.pctOfCdiOutOfRange);
  }
  final redemptions = legs.where((leg) => leg.kind == LegKind.redemption).length;
  if (redemptions == 0) {
    return const YieldProposals(proposals: [], redemptions: 0, skippedCoveredDay: 0, skippedNoYield: 0);
  }
  final proposals = <RedemptionEstimate>[];
  var skippedCovered = 0;
  var skippedNoYield = 0;
  for (final estimate in estimateRedemptions(legs: legs, selic: selic, pctOfCdi: pctOfCdi)) {
    if (coveredDays.contains(toIsoDay(estimate.date))) {
      skippedCovered++;
    } else if (estimate.grossYield < _minYield) {
      skippedNoYield++;
    } else {
      proposals.add(estimate);
    }
  }
  return YieldProposals(
    proposals: proposals,
    redemptions: redemptions,
    skippedCoveredDay: skippedCovered,
    skippedNoYield: skippedNoYield,
  );
}
