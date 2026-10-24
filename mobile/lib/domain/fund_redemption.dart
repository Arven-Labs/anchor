// IR e IOF estimados no resgate de cotas de fundo aberto. Porta de
// `desktop/src-tauri/src/domain/tax/fund_redemption.rs`. É informativo: o imposto sai
// do valor do resgate na fonte, então não há DARF nem pagamento.
//
// Regras:
// - Lotes FIFO por data de compra: o resgate consome as aplicações mais antigas
//   primeiro. O prazo de cada lote é a data do resgate menos a da compra, e o
//   come-cotas não o reinicia.
// - Tabela por tipo: longo prazo 22,5% até 180 dias, 20% de 181 a 360, 17,5% de 361 a
//   720, 15% acima; curto prazo 22,5% até 180 e 20% acima; ações e entidade de
//   investimento 15% fixo (sem come-cotas e sem IOF).
// - Crédito de prejuízo usado num come-cotas registrado (campo opcional da transação):
//   o rendimento que ele cobriu sai do ganho já tributado do lote, para não ser
//   tributado de novo no resgate; o imposto pago segue o valor das cotas retiradas.
// - Fundos fechados e exclusivos (Lei 14.754/2023): a amortização devolve capital sem
//   tirar cotas: cada lote devolve a fração `valor ÷ (cota × cotas em carteira)` e é
//   tributado como um resgate dessa fração; o custo por cota e o ganho já tributado do
//   lote caem nessa fração. O estoque de 2023 (arts. 27 e 28) entra como um "come-cotas
//   em dinheiro" em 31/12/2023: sobe o ganho já tributado do lote e o imposto pago vira
//   crédito no resgate.
// - IOF (longo e curto prazo): `iofRateForDays` sobre o rendimento do lote com menos
//   de 30 dias, antes do IR.
// - Crédito do come-cotas (pressuposto): o IR da vida do lote é `rendimento nominal ×
//   alíquota da tabela`, e o come-cotas já pago abate dele. O come-cotas retira cotas
//   proporcionalmente à quantidade de cada lote aberto (aproximação). Rendimento
//   nominal = rendimento do resgate + a parte do ganho que os come-cotas do lote já
//   tributaram. IR = max(0, (nominal − IOF) × alíquota − come-cotas pago); o excesso
//   de come-cotas é só informação (possível restituição por PER/DCOMP, regra não
//   confirmada): não entra no IR nem no líquido.
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

/// Entidade de investimento (Lei 14.754, arts. 18 e 24): 15% fixo, sem IOF.
const fundIrInvestmentEntity = 0.15;

/// Estoque de rendimento até 31/12/2023 (Lei 14.754, arts. 27 e 28): 15% pela regra
/// geral, 8% pela opção. Espelham `FUND_ESTOQUE_RATE_*` de `rules.rs` do desktop.
const fundEstoqueRateGeneral = 0.15;
const fundEstoqueRateOption = 0.08;

enum FundEventKind {
  /// Aplicação: cotas compradas e o custo total (valor + taxa).
  buy,

  /// Cotas retiradas pelo come-cotas e o valor da cota do dia.
  comeCotas,

  /// Resgate (venda): cotas e o valor da cota no resgate.
  redeem,

  /// Amortização de fundo fechado ou exclusivo: o valor recebido e o valor da cota do
  /// dia. As cotas ficam; o custo (e o ganho já tributado) de cada lote cai na fração
  /// `valor ÷ (cota × cotas em carteira)`.
  amortize,

  /// Estoque de rendimento até 31/12/2023: o imposto saiu em dinheiro, sobre a alta da
  /// cota (o valor patrimonial em 31/12/2023) contra o custo de cada lote aberto. Não
  /// tira cotas.
  estoque,
}

/// Um fato do fundo, em qualquer ordem (a ordenação é feita aqui). [amount] é o
/// custo total na aplicação e o valor da cota no come-cotas e no resgate; na
/// amortização é o valor recebido (a cota fica em [quotaValue]).
class FundEvent {
  final DateTime date;

  /// Desempate no mesmo dia e tipo (o id da transação).
  final int id;
  final FundEventKind kind;
  final double quantity;
  final double amount;

  /// A cota do dia, na amortização e no estoque (zero nos outros fatos).
  final double quotaValue;

  /// A alíquota do estoque (zero nos outros fatos).
  final double rate;

  /// No come-cotas, o rendimento (BRL) que o crédito de prejuízo cobriu ali; `null` =
  /// não informado, o mesmo que 0 para a conta (nulo nos outros fatos).
  final double? lossCreditUsed;

  const FundEvent.buy({
    required this.date,
    required this.id,
    required this.quantity,
    required double cost,
  }) : kind = FundEventKind.buy,
       amount = cost,
       quotaValue = 0,
       rate = 0,
       lossCreditUsed = null;

  const FundEvent.comeCotas({
    required this.date,
    required this.id,
    required this.quantity,
    required double quotaValue,
    this.lossCreditUsed,
  }) : kind = FundEventKind.comeCotas,
       amount = quotaValue,
       quotaValue = 0,
       rate = 0;

  const FundEvent.redeem({
    required this.date,
    required this.id,
    required this.quantity,
    required double quotaValue,
  }) : kind = FundEventKind.redeem,
       amount = quotaValue,
       quotaValue = 0,
       rate = 0,
       lossCreditUsed = null;

  const FundEvent.amortize({
    required this.date,
    required this.id,
    required this.amount,
    required this.quotaValue,
  }) : kind = FundEventKind.amortize,
       quantity = 0,
       rate = 0,
       lossCreditUsed = null;

  const FundEvent.estoque({
    required this.date,
    required this.quotaValue,
    required this.rate,
  }) : kind = FundEventKind.estoque,
       id = 0,
       quantity = 0,
       amount = 0,
       lossCreditUsed = null;
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

  /// É uma amortização (as cotas ficam): [quotas] são as cotas equivalentes ao valor
  /// recebido e [gross] é esse valor.
  final bool isAmortization;

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
    this.isAmortization = false,
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
    isAmortization: isAmortization,
  );

  /// Bruto − IOF − IR.
  double get net => gross - iof - ir;
}

/// O imposto do estoque de rendimento até 31/12/2023 (Lei 14.754, arts. 27 e 28),
/// pago em dinheiro pelo cotista.
class EstoqueEstimate {
  final DateTime date;

  /// Valor patrimonial da cota em 31/12/2023.
  final double quotaValue;

  /// 15% (regra geral) ou 8% (opção).
  final double rate;

  /// Ganho acumulado até a data: soma, por lote aberto, de `cotas × max(0, cota −
  /// custo por cota)`.
  final double base;
  final double tax;

  const EstoqueEstimate({
    required this.date,
    required this.quotaValue,
    required this.rate,
    required this.base,
    required this.tax,
  });
}

/// O que o motor devolve para um fundo: os resgates e amortizações, e o estoque quando
/// houve o evento.
class FundEstimate {
  final List<RedemptionEstimate> redemptions;
  final EstoqueEstimate? estoque;

  /// Custo por cota das cotas que restam, somando o estoque de 2023 já tributado (art.
  /// 17 §2: preço + parcelas já tributadas − parcelas amortizadas). `null` sem o evento
  /// do estoque ou sem cotas abertas.
  final double? taxedBasisPerQuota;

  const FundEstimate({
    required this.redemptions,
    this.estoque,
    this.taxedBasisPerQuota,
  });
}

class _Lot {
  final DateTime date;
  double quantity;
  double costPerQuota;

  /// Parte de [ccGain] que veio do estoque de 2023: some na mesma fração das cotas
  /// (resgate, come-cotas) e do custo (amortização).
  double estoqueGain = 0;

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
    case FundTerm.investmentEntity:
      return fundIrInvestmentEntity;
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

/// No mesmo dia: aplicação, estoque, come-cotas, resgate ou amortização.
int _rank(FundEventKind kind) {
  switch (kind) {
    case FundEventKind.buy:
      return 0;
    case FundEventKind.estoque:
      return 1;
    case FundEventKind.comeCotas:
      return 2;
    case FundEventKind.redeem:
    case FundEventKind.amortize:
      return 3;
  }
}

int _compare(FundEvent a, FundEvent b) {
  final byDate = _day(a.date).compareTo(_day(b.date));
  if (byDate != 0) return byDate;
  final byKind = _rank(a.kind).compareTo(_rank(b.kind));
  if (byKind != 0) return byKind;
  return a.id.compareTo(b.id);
}

double _min(double a, double b) => a < b ? a : b;
double _positive(double value) => value > 0 ? value : 0;

/// O resultado de realizar `take` cotas de um lote a uma cota: prazo, alíquota,
/// rendimento nominal, IOF, IR e o que o come-cotas (ou o estoque) já pago fez com ele.
class _LotOutcome {
  final int days;
  final double rate;
  final double nominal;
  final double iof;
  final double ir;
  final double creditUsed;
  final double excess;
  final double loss;
  final double cost;

  /// A parte do ganho já tributado e do imposto já pago que sai junto com estas cotas.
  final double ccGain;
  final double ccPaid;

  const _LotOutcome({
    required this.days,
    required this.rate,
    required this.nominal,
    required this.iof,
    required this.ir,
    required this.creditUsed,
    required this.excess,
    required this.loss,
    required this.cost,
    required this.ccGain,
    required this.ccPaid,
  });
}

_LotOutcome _settleLot(
  _Lot lot,
  double take,
  double quotaValue,
  DateTime on,
  FundTerm term,
) {
  final fraction = take / lot.quantity;
  final days = _day(on).difference(lot.date).inDays;

  final redeemGain = take * (quotaValue - lot.costPerQuota);
  final ccGain = lot.ccGain * fraction;
  final ccPaid = lot.ccPaid * fraction;
  final nominal = redeemGain + ccGain;
  // Sem IOF em fundo de ações nem em entidade de investimento (alíquota fixa).
  final iof = term.isFlatRate
      ? 0.0
      : iofRateForDays(days) * _positive(redeemGain);
  final rate = _tableRate(term, days);
  final irDue = _positive(nominal - iof) * rate;
  return _LotOutcome(
    days: days,
    rate: rate,
    nominal: nominal,
    iof: iof,
    ir: _positive(irDue - ccPaid),
    creditUsed: _min(ccPaid, irDue),
    excess: _positive(ccPaid - irDue),
    loss: _positive(iof - nominal),
    cost: take * lot.costPerQuota,
    ccGain: ccGain,
    ccPaid: ccPaid,
  );
}

/// Estima o IOF e o IR de cada resgate e amortização do fundo (ver [estimateFund]).
List<RedemptionEstimate> estimateRedemptions(
  List<FundEvent> events,
  FundTerm term,
) => estimateFund(events, term).redemptions;

/// Estima o IOF e o IR de cada resgate e amortização do fundo, e o imposto do estoque
/// de 2023 quando há o evento.
FundEstimate estimateFund(List<FundEvent> events, FundTerm term) {
  final ordered = [...events]..sort(_compare);
  final lots = <_Lot>[];
  final estimates = <RedemptionEstimate>[];
  EstoqueEstimate? estoque;

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
        // O rendimento que o crédito de prejuízo cobriu não foi tributado agora e não
        // pode voltar no resgate: sai do ganho já tributado, repartido como as cotas, e
        // no máximo até o ganho das cotas retiradas. O imposto pago (`ccPaid`) segue
        // sendo o valor das cotas retiradas.
        final taxedGain = lots.fold<double>(
          0,
          (sum, lot) =>
              sum +
              (taken * lot.quantity / total) * (event.amount - lot.costPerQuota),
        );
        final credit = _min(event.lossCreditUsed ?? 0, _positive(taxedGain));
        for (final lot in lots) {
          final share = taken * lot.quantity / total;
          lot.estoqueGain *= 1 - share / lot.quantity;
          lot.ccGain +=
              share * (event.amount - lot.costPerQuota) -
              (taken > costEps ? credit * share / taken : 0);
          lot.ccPaid += share * event.amount;
          lot.quantity -= share;
        }
      case FundEventKind.estoque:
        // O imposto saiu em dinheiro, sem tirar cotas: o ganho já tributado e o
        // imposto pago ficam no lote, como no come-cotas, e o resgate futuro os conta
        // (rendimento nominal e crédito).
        var base = 0.0;
        for (final lot in lots.where((l) => l.quantity > costEps)) {
          final gain =
              lot.quantity * _positive(event.quotaValue - lot.costPerQuota);
          base += gain;
          lot.estoqueGain += gain;
          lot.ccGain += gain;
          lot.ccPaid += event.rate * gain;
        }
        estoque = EstoqueEstimate(
          date: _day(event.date),
          quotaValue: event.quotaValue,
          rate: event.rate,
          base: base,
          tax: event.rate * base,
        );
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
          final o = _settleLot(lot, take, quotaValue, event.date, term);

          cost += o.cost;
          iofTotal += o.iof;
          irTotal += o.ir;
          creditTotal += o.creditUsed;
          excessTotal += o.excess;
          lossTotal += o.loss;
          parts.add(
            LotPart(
              lotDate: lot.date,
              quotas: take,
              days: o.days,
              rate: o.rate,
              gain: o.nominal,
              iof: o.iof,
              ir: o.ir,
            ),
          );

          lot.estoqueGain *= 1 - take / lot.quantity;
          lot.quantity -= take;
          lot.ccGain -= o.ccGain;
          lot.ccPaid -= o.ccPaid;
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
      case FundEventKind.amortize:
        // As cotas ficam: cada lote devolve a mesma fração do que tem, calculada sobre
        // o valor da posição.
        final total = lots.fold<double>(0, (sum, lot) => sum + lot.quantity);
        final positionValue = total * event.quotaValue;
        if (total <= costEps || positionValue <= costEps) continue;
        final fraction = _min(event.amount / positionValue, 1);
        final parts = <LotPart>[];
        var cost = 0.0, iofTotal = 0.0, irTotal = 0.0;
        var creditTotal = 0.0, excessTotal = 0.0, lossTotal = 0.0;
        var equivalentQuotas = 0.0;
        for (final lot in lots.where((l) => l.quantity > costEps)) {
          final take = fraction * lot.quantity;
          final o = _settleLot(lot, take, event.quotaValue, event.date, term);

          cost += o.cost;
          iofTotal += o.iof;
          irTotal += o.ir;
          creditTotal += o.creditUsed;
          excessTotal += o.excess;
          lossTotal += o.loss;
          equivalentQuotas += take;
          parts.add(
            LotPart(
              lotDate: lot.date,
              quotas: take,
              days: o.days,
              rate: o.rate,
              gain: o.nominal,
              iof: o.iof,
              ir: o.ir,
            ),
          );

          // O custo e o que já foi tributado caem na mesma fração; a quantidade de
          // cotas não muda.
          lot.costPerQuota *= 1 - fraction;
          lot.estoqueGain *= 1 - fraction;
          lot.ccGain -= o.ccGain;
          lot.ccPaid -= o.ccPaid;
        }
        estimates.add(
          RedemptionEstimate(
            txId: event.id,
            date: _day(event.date),
            quotas: equivalentQuotas,
            quotaValue: event.quotaValue,
            gross: equivalentQuotas * event.quotaValue,
            cost: cost,
            iof: iofTotal,
            ir: irTotal,
            comeCotasCredit: creditTotal,
            lots: parts,
            uncoveredQuotas: 0,
            excessComeCotas: excessTotal,
            lossCreated: lossTotal,
            irBeforeCompensation: irTotal,
            isAmortization: true,
          ),
        );
    }
  }

  final open = lots.where((l) => l.quantity > costEps).toList();
  final openQuotas = open.fold<double>(0, (sum, lot) => sum + lot.quantity);
  final double? taxedBasis = estoque != null && openQuotas > costEps
      ? open.fold<double>(
              0,
              (sum, lot) =>
                  sum + lot.quantity * lot.costPerQuota + lot.estoqueGain,
            ) /
            openQuotas
      : null;
  return FundEstimate(
    redemptions: estimates,
    estoque: estoque,
    taxedBasisPerQuota: taxedBasis,
  );
}
