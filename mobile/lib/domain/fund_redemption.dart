// IR e IOF estimados no resgate de cotas de fundo aberto. Porta de
// `desktop/src-tauri/src/domain/tax/fund_redemption.rs`. É informativo: o imposto sai
// do valor do resgate na fonte, então não há DARF nem pagamento.
//
// Regras:
// - Lotes FIFO por data de compra: o resgate consome as aplicações mais antigas
//   primeiro. O prazo de cada lote é a data do resgate menos a da compra, e o
//   come-cotas não o reinicia.
// - Tabela por tipo: longo prazo 22,5% até 180 dias, 20% de 181 a 360, 17,5% de 361 a
//   720, 15% acima; curto prazo 22,5% até 180 e 20% acima; ações 15% fixo (sem
//   come-cotas e sem IOF).
// - IOF (longo e curto prazo): `iofRateForDays` sobre o rendimento do lote com menos
//   de 30 dias, antes do IR.
// - Crédito do come-cotas (pressuposto): o IR da vida do lote é `rendimento nominal ×
//   alíquota da tabela`, e o come-cotas já pago abate dele. O come-cotas retira cotas
//   proporcionalmente à quantidade de cada lote aberto (aproximação). Rendimento
//   nominal = rendimento do resgate + a parte do ganho que os come-cotas do lote já
//   tributaram. IR = max(0, (nominal − IOF) × alíquota − come-cotas pago); o excesso
//   de come-cotas vira aviso (compensação não modelada).
//
// ATENÇÃO: as regras vêm de fontes secundárias; conferir na Receita antes de confiar.

import '../models/fund.dart';
import 'cost_basis.dart' show costEps;
import 'iof.dart';

/// Alíquotas da tabela; espelham `FUND_IR_*` de `rules.rs` do desktop.
const fundIrLongUpTo180 = 0.225;
const fundIrLong181To360 = 0.20;
const fundIrLong361To720 = 0.175;
const fundIrLongOver720 = 0.15;
const fundIrShortUpTo180 = 0.225;
const fundIrShortOver180 = 0.20;
const fundIrEquity = 0.15;

enum FundEventKind {
  /// Aplicação: cotas compradas e o custo total (valor + taxa).
  buy,

  /// Cotas retiradas pelo come-cotas e o valor da cota do dia.
  comeCotas,

  /// Resgate (venda): cotas e o valor da cota no resgate.
  redeem,
}

/// Um fato do fundo, em qualquer ordem (a ordenação é feita aqui). [amount] é o
/// custo total na aplicação e o valor da cota no come-cotas e no resgate.
class FundEvent {
  final DateTime date;

  /// Desempate no mesmo dia e tipo (o id da transação).
  final int id;
  final FundEventKind kind;
  final double quantity;
  final double amount;

  const FundEvent.buy({
    required this.date,
    required this.id,
    required this.quantity,
    required double cost,
  }) : kind = FundEventKind.buy,
       amount = cost;

  const FundEvent.comeCotas({
    required this.date,
    required this.id,
    required this.quantity,
    required double quotaValue,
  }) : kind = FundEventKind.comeCotas,
       amount = quotaValue;

  const FundEvent.redeem({
    required this.date,
    required this.id,
    required this.quantity,
    required double quotaValue,
  }) : kind = FundEventKind.redeem,
       amount = quotaValue;
}

/// A parte de um resgate que saiu de um lote.
class LotPart {
  final DateTime lotDate;
  final double quotas;
  final int days;
  final double rate;

  /// Rendimento nominal da parte (rendimento do resgate + o já tributado pelo
  /// come-cotas).
  final double gain;
  final double iof;
  final double ir;

  const LotPart({
    required this.lotDate,
    required this.quotas,
    required this.days,
    required this.rate,
    required this.gain,
    required this.iof,
    required this.ir,
  });

  /// Só o IR muda: é o que a compensação de prejuízo abate (`fund_compensation.dart`).
  LotPart copyWith({double? ir}) => LotPart(
    lotDate: lotDate,
    quotas: quotas,
    days: days,
    rate: rate,
    gain: gain,
    iof: iof,
    ir: ir ?? this.ir,
  );
}

class RedemptionEstimate {
  final int txId;
  final DateTime date;
  final double quotas;
  final double quotaValue;

  /// Valor bruto do resgate (cotas × valor da cota).
  final double gross;

  /// Custo das cotas resgatadas.
  final double cost;
  final double iof;
  final double ir;

  /// Come-cotas já pago que abateu o IR deste resgate.
  final double comeCotasCredit;
  final List<LotPart> lots;

  /// Cotas do resgate sem lote de compra conhecido (o custo está incompleto).
  final double uncoveredQuotas;

  /// Come-cotas pago acima do IR devido: o reembolso não é modelado.
  final double excessComeCotas;

  /// Prejuízo deste resgate: a soma dos lotes com rendimento negativo depois do IOF. É
  /// crédito para resgates e come-cotas posteriores do mesmo grupo.
  final double lossCreated;

  /// Crédito de prejuízo de resgates anteriores que abateu o IR deste resgate (zero até
  /// `compensate` rodar).
  final double lossCreditUsed;

  /// IR antes de descontar esse crédito (igual a [ir] até `compensate` rodar).
  final double irBeforeCompensation;

  const RedemptionEstimate({
    required this.txId,
    required this.date,
    required this.quotas,
    required this.quotaValue,
    required this.gross,
    required this.cost,
    required this.iof,
    required this.ir,
    required this.comeCotasCredit,
    required this.lots,
    required this.uncoveredQuotas,
    required this.excessComeCotas,
    required this.lossCreated,
    this.lossCreditUsed = 0,
    required this.irBeforeCompensation,
  });

  /// O resgate com os lotes (e o IR total) já abatidos pelo crédito de prejuízo.
  RedemptionEstimate withLossCredit({
    required List<LotPart> lots,
    required double lossCreditUsed,
  }) => RedemptionEstimate(
    txId: txId,
    date: date,
    quotas: quotas,
    quotaValue: quotaValue,
    gross: gross,
    cost: cost,
    iof: iof,
    ir: lots.fold<double>(0, (sum, lot) => sum + lot.ir),
    comeCotasCredit: comeCotasCredit,
    lots: lots,
    uncoveredQuotas: uncoveredQuotas,
    excessComeCotas: excessComeCotas,
    lossCreated: lossCreated,
    lossCreditUsed: lossCreditUsed,
    irBeforeCompensation: irBeforeCompensation,
  );

  /// Bruto − IOF − IR.
  double get net => gross - iof - ir;
}

class _Lot {
  final DateTime date;
  double quantity;
  final double costPerQuota;

  /// Ganho já tributado pelos come-cotas, ligado às cotas que restam no lote.
  double ccGain = 0;

  /// Come-cotas já pago, ligado às cotas que restam no lote.
  double ccPaid = 0;

  _Lot(this.date, this.quantity, this.costPerQuota);
}

/// Alíquota da tabela para [days] dias de aplicação.
double _tableRate(FundTerm term, int days) {
  switch (term) {
    case FundTerm.equity:
      return fundIrEquity;
    case FundTerm.longTerm:
      if (days <= 180) return fundIrLongUpTo180;
      if (days <= 360) return fundIrLong181To360;
      if (days <= 720) return fundIrLong361To720;
      return fundIrLongOver720;
    case FundTerm.shortTerm:
      return days <= 180 ? fundIrShortUpTo180 : fundIrShortOver180;
  }
}

DateTime _day(DateTime date) => DateTime.utc(date.year, date.month, date.day);

/// No mesmo dia: aplicação, come-cotas, resgate.
int _compare(FundEvent a, FundEvent b) {
  final byDate = _day(a.date).compareTo(_day(b.date));
  if (byDate != 0) return byDate;
  final byKind = a.kind.index.compareTo(b.kind.index);
  if (byKind != 0) return byKind;
  return a.id.compareTo(b.id);
}

double _min(double a, double b) => a < b ? a : b;
double _positive(double value) => value > 0 ? value : 0;

/// Estima o IOF e o IR de cada resgate do fundo.
List<RedemptionEstimate> estimateRedemptions(
  List<FundEvent> events,
  FundTerm term,
) {
  final ordered = [...events]..sort(_compare);
  final lots = <_Lot>[];
  final estimates = <RedemptionEstimate>[];

  for (final event in ordered) {
    switch (event.kind) {
      case FundEventKind.buy:
        if (event.quantity > costEps) {
          lots.add(
            _Lot(_day(event.date), event.quantity, event.amount / event.quantity),
          );
        }
      case FundEventKind.comeCotas:
        // Retira as cotas dos lotes abertos na proporção da quantidade de cada um.
        final total = lots.fold<double>(0, (sum, lot) => sum + lot.quantity);
        if (total <= costEps) continue;
        final taken = _min(event.quantity, total);
        for (final lot in lots) {
          final share = taken * lot.quantity / total;
          lot.ccGain += share * (event.amount - lot.costPerQuota);
          lot.ccPaid += share * event.amount;
          lot.quantity -= share;
        }
      case FundEventKind.redeem:
        final quotaValue = event.amount;
        var remaining = event.quantity;
        final parts = <LotPart>[];
        var cost = 0.0, iofTotal = 0.0, irTotal = 0.0;
        var creditTotal = 0.0, excessTotal = 0.0;
        var lossTotal = 0.0;
        for (final lot in lots.where((l) => l.quantity > costEps)) {
          if (remaining <= costEps) break;
          final take = _min(remaining, lot.quantity);
          final fraction = take / lot.quantity;
          final days = _day(event.date).difference(lot.date).inDays;

          final redeemGain = take * (quotaValue - lot.costPerQuota);
          final ccGain = lot.ccGain * fraction;
          final ccPaid = lot.ccPaid * fraction;
          final nominal = redeemGain + ccGain;
          // O IOF não existe em fundo de ações.
          final iof = term == FundTerm.equity
              ? 0.0
              : iofRateForDays(days) * _positive(redeemGain);
          final rate = _tableRate(term, days);
          final irDue = _positive(nominal - iof) * rate;
          final ir = _positive(irDue - ccPaid);

          cost += take * lot.costPerQuota;
          iofTotal += iof;
          irTotal += ir;
          creditTotal += _min(ccPaid, irDue);
          excessTotal += _positive(ccPaid - irDue);
          lossTotal += _positive(iof - nominal);
          parts.add(
            LotPart(
              lotDate: lot.date,
              quotas: take,
              days: days,
              rate: rate,
              gain: nominal,
              iof: iof,
              ir: ir,
            ),
          );

          lot.quantity -= take;
          lot.ccGain -= ccGain;
          lot.ccPaid -= ccPaid;
          remaining -= take;
        }
        estimates.add(
          RedemptionEstimate(
            txId: event.id,
            date: _day(event.date),
            quotas: event.quantity,
            quotaValue: quotaValue,
            gross: event.quantity * quotaValue,
            cost: cost,
            iof: iofTotal,
            ir: irTotal,
            comeCotasCredit: creditTotal,
            lots: parts,
            uncoveredQuotas: _positive(remaining),
            excessComeCotas: excessTotal,
            lossCreated: lossTotal,
            irBeforeCompensation: irTotal,
          ),
        );
    }
  }
  return estimates;
}
