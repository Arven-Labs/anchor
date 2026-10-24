// IR e IOF estimados no resgate de cotas de fundo aberto. É informativo: o imposto sai do valor do resgate na fonte, então
// não há DARF nem pagamento. A estimativa mostra o que o fundo provavelmente reteve do valor bruto da venda.
//
// Regras (as alíquotas vêm de `tax_rules`; os limites de dias e o resto são código):
// - Lotes FIFO por data de compra: o resgate consome as aplicações mais antigas primeiro. O prazo de cada lote é a data do
//   resgate menos a data da compra, e o come-cotas não o reinicia.
// - Tabela por tipo do fundo: longo prazo 22,5% até 180 dias, 20% de 181 a 360, 17,5% de 361 a 720, 15% acima; curto prazo
//   22,5% até 180 e 20% acima; ações 15% fixo (sem come-cotas e sem IOF).
// - IOF (longo e curto prazo): `iof_rate_for_days` sobre o rendimento do lote com menos de 30 dias, antes do IR.
// - Crédito do come-cotas (pressuposto): o IR da vida do lote é `rendimento nominal × alíquota da tabela`, e o come-cotas já
//   pago abate dele. O come-cotas retira cotas proporcionalmente à quantidade de cada lote aberto (aproximação).
//   Rendimento nominal = rendimento do resgate + a parte do ganho que os come-cotas do lote já tributaram.
//   IR = max(0, (nominal − IOF) × alíquota − come-cotas pago); o excesso de come-cotas vira aviso (reembolso não modelado).
// - Fundos fechados e exclusivos (Lei 14.754/2023): o art. 17 vale igual para eles. A amortização devolve capital sem tirar
//   cotas: cada lote devolve a fração `valor ÷ (cota × cotas em carteira)` e é tributado como um resgate dessa fração; o custo
//   por cota e o ganho já tributado do lote caem nessa fração. Entidades de investimento (art. 18): 15% fixo e sem IOF.
//   O estoque de 2023 (arts. 27 e 28) entra como um "come-cotas em dinheiro" em 31/12/2023: sobe o ganho já tributado do
//   lote e o imposto pago vira crédito no resgate.
//
// ATENÇÃO: as regras vêm de fontes secundárias; conferir na Receita antes de confiar.

use chrono::NaiveDate;
use serde::Serialize;

use super::come_cotas::FundTerm;
use super::iof::iof_rate_for_days;
use super::rules::{self, RuleSet};
use crate::domain::cost_basis::EPS;
use crate::error::AppError;

#[derive(Clone, Copy, Debug)]
pub enum FundEventKind {
    /// Aplicação: cotas compradas e o custo total (valor + taxa).
    Buy { quantity: f64, cost: f64 },
    /// Cotas retiradas pelo come-cotas e o valor da cota do dia.
    ComeCotas { quantity: f64, quota_value: f64 },
    /// Resgate (venda): cotas e o valor da cota no resgate.
    Redeem { tx_id: i32, quantity: f64, quota_value: f64 },
    /// Amortização de fundo fechado: o valor recebido e o valor da cota do dia. As cotas ficam; o custo (e o ganho já
    /// tributado) de cada lote cai na fração `valor ÷ (cota × cotas em carteira)`.
    Amortize { tx_id: i32, amount: f64, quota_value: f64 },
    /// Estoque de rendimento até 31/12/2023 (Lei 14.754, arts. 27 e 28): o imposto saiu em dinheiro, sobre a alta da cota
    /// (`quota_value`, o valor patrimonial em 31/12/2023) contra o custo de cada lote aberto. Não tira cotas.
    Estoque { quota_value: f64, rate: f64 },
}

/// Um fato do fundo, em qualquer ordem (a ordenação é feita aqui).
#[derive(Clone, Copy, Debug)]
pub struct FundEvent {
    pub date: NaiveDate,
    /// Desempate no mesmo dia e tipo (o id da transação).
    pub id: i32,
    pub kind: FundEventKind,
}

impl FundEvent {
    /// No mesmo dia: aplicação, estoque, come-cotas, resgate ou amortização.
    fn order_key(&self) -> (NaiveDate, u8, i32) {
        let rank = match self.kind {
            FundEventKind::Buy { .. } => 0,
            FundEventKind::Estoque { .. } => 1,
            FundEventKind::ComeCotas { .. } => 2,
            FundEventKind::Redeem { .. } | FundEventKind::Amortize { .. } => 3,
        };
        (self.date, rank, self.id)
    }
}

/// A parte de um resgate que saiu de um lote.
#[derive(Clone, Debug, Serialize)]
pub struct LotPart {
    pub lot_date: NaiveDate,
    pub quotas: f64,
    pub days: i64,
    pub rate: f64,
    /// Rendimento nominal da parte (rendimento do resgate + o já tributado pelo come-cotas).
    pub gain: f64,
    pub iof: f64,
    pub ir: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RedemptionEstimate {
    pub tx_id: i32,
    pub date: NaiveDate,
    pub quotas: f64,
    pub quota_value: f64,
    /// Valor bruto do resgate (cotas × valor da cota).
    pub gross: f64,
    /// Custo das cotas resgatadas.
    pub cost: f64,
    pub iof: f64,
    pub ir: f64,
    /// Come-cotas já pago que abateu o IR deste resgate.
    pub come_cotas_credit: f64,
    /// Bruto − IOF − IR.
    pub net: f64,
    pub lots: Vec<LotPart>,
    /// Cotas do resgate sem lote de compra conhecido (o custo está incompleto).
    pub uncovered_quotas: f64,
    /// Come-cotas pago acima do IR devido: o reembolso não é modelado.
    pub excess_come_cotas: f64,
    /// Prejuízo deste resgate: a soma dos lotes com rendimento negativo depois do IOF. É crédito para resgates e
    /// come-cotas posteriores do mesmo grupo (`fund_compensation`).
    pub loss_created: f64,
    /// Crédito de prejuízo de resgates anteriores que abateu a base deste resgate (zero até `fund_compensation` rodar).
    pub loss_credit_used: f64,
    /// IR antes de descontar esse crédito (igual a `ir` até `fund_compensation` rodar).
    pub ir_before_compensation: f64,
    /// É uma amortização (as cotas ficam): `quotas` são as cotas equivalentes ao valor recebido e `gross` é esse valor.
    pub is_amortization: bool,
}

/// O imposto do estoque de rendimento até 31/12/2023 (Lei 14.754, arts. 27 e 28), pago em dinheiro pelo cotista.
#[derive(Clone, Debug, Serialize)]
pub struct EstoqueEstimate {
    pub date: NaiveDate,
    /// Valor patrimonial da cota em 31/12/2023.
    pub quota_value: f64,
    /// 15% (regra geral) ou 8% (opção).
    pub rate: f64,
    /// Ganho acumulado até a data: soma, por lote aberto, de `cotas × max(0, cota − custo por cota)`.
    pub base: f64,
    pub tax: f64,
}

/// O que o motor devolve para um fundo: os resgates e amortizações, e o estoque quando houve o evento.
#[derive(Clone, Debug)]
pub struct FundEstimate {
    pub redemptions: Vec<RedemptionEstimate>,
    pub estoque: Option<EstoqueEstimate>,
    /// Custo por cota das cotas que restam, somando o estoque de 2023 já tributado (art. 17 §2: preço + parcelas já
    /// tributadas − parcelas amortizadas). `None` sem o evento do estoque ou sem cotas abertas.
    pub taxed_basis_per_quota: Option<f64>,
}

struct Lot {
    date: NaiveDate,
    quantity: f64,
    cost_per_quota: f64,
    /// Parte de `cc_gain` que veio do estoque de 2023: some na mesma fração das cotas (resgate, come-cotas) e do custo
    /// (amortização).
    estoque_gain: f64,
    /// Ganho já tributado pelos come-cotas, ligado às cotas que restam no lote.
    cc_gain: f64,
    /// Come-cotas já pago, ligado às cotas que restam no lote.
    cc_paid: f64,
}

/// Alíquota da tabela para `days` dias de aplicação.
fn table_rate(term: FundTerm, days: i64, rules: &RuleSet, on: NaiveDate) -> Result<f64, AppError> {
    let key = match term {
        FundTerm::Equity => rules::FUND_IR_EQUITY,
        FundTerm::InvestmentEntity => rules::FUND_IR_INVESTMENT_ENTITY,
        FundTerm::LongTerm => match days {
            d if d <= 180 => rules::FUND_IR_LONG_UP_TO_180,
            d if d <= 360 => rules::FUND_IR_LONG_181_360,
            d if d <= 720 => rules::FUND_IR_LONG_361_720,
            _ => rules::FUND_IR_LONG_OVER_720,
        },
        FundTerm::ShortTerm => {
            if days <= 180 {
                rules::FUND_IR_SHORT_UP_TO_180
            } else {
                rules::FUND_IR_SHORT_OVER_180
            }
        }
    };
    rules.get(key, on)
}

/// O resultado de realizar `take` cotas de um lote a `quota_value`: prazo, alíquota, rendimento nominal, IOF, IR e o que o
/// come-cotas (ou o estoque) já pago fez com ele.
struct LotOutcome {
    days: i64,
    rate: f64,
    nominal: f64,
    iof: f64,
    ir: f64,
    credit_used: f64,
    excess: f64,
    loss: f64,
    cost: f64,
    /// A parte do ganho já tributado e do imposto já pago que sai junto com estas cotas.
    cc_gain: f64,
    cc_paid: f64,
}

fn settle_lot(
    lot: &Lot,
    take: f64,
    quota_value: f64,
    on: NaiveDate,
    term: FundTerm,
    rules: &RuleSet,
) -> Result<LotOutcome, AppError> {
    let fraction = take / lot.quantity;
    let days = (on - lot.date).num_days();

    let redeem_gain = take * (quota_value - lot.cost_per_quota);
    let cc_gain = lot.cc_gain * fraction;
    let cc_paid = lot.cc_paid * fraction;
    let nominal = redeem_gain + cc_gain;
    // Sem IOF em fundo de ações nem em entidade de investimento (alíquota fixa, sem tabela por prazo).
    let iof = if term.is_flat_rate() { 0.0 } else { iof_rate_for_days(days) * redeem_gain.max(0.0) };
    let rate = table_rate(term, days, rules, on)?;
    let ir_due = (nominal - iof).max(0.0) * rate;
    Ok(LotOutcome {
        days,
        rate,
        nominal,
        iof,
        ir: (ir_due - cc_paid).max(0.0),
        credit_used: cc_paid.min(ir_due),
        excess: (cc_paid - ir_due).max(0.0),
        loss: (iof - nominal).max(0.0),
        cost: take * lot.cost_per_quota,
        cc_gain,
        cc_paid,
    })
}

/// Estima o IOF e o IR de cada resgate do fundo (ver `estimate_fund`). Só os testes precisam do atalho sem o estoque.
#[cfg(test)]
pub fn estimate_redemptions(
    events: &[FundEvent],
    term: FundTerm,
    rules: &RuleSet,
) -> Result<Vec<RedemptionEstimate>, AppError> {
    Ok(estimate_fund(events, term, rules)?.redemptions)
}

/// Estima o IOF e o IR de cada resgate e amortização do fundo, e o imposto do estoque de 2023 quando há o evento.
pub fn estimate_fund(events: &[FundEvent], term: FundTerm, rules: &RuleSet) -> Result<FundEstimate, AppError> {
    let mut ordered: Vec<&FundEvent> = events.iter().collect();
    ordered.sort_by_key(|e| e.order_key());

    let mut lots: Vec<Lot> = Vec::new();
    let mut estimates = Vec::new();
    let mut estoque: Option<EstoqueEstimate> = None;
    for event in ordered {
        match event.kind {
            FundEventKind::Buy { quantity, cost } => {
                if quantity > EPS {
                    lots.push(Lot {
                        date: event.date,
                        quantity,
                        cost_per_quota: cost / quantity,
                        estoque_gain: 0.0,
                        cc_gain: 0.0,
                        cc_paid: 0.0,
                    });
                }
            }
            FundEventKind::ComeCotas { quantity, quota_value } => {
                // Retira as cotas dos lotes abertos na proporção da quantidade de cada um.
                let total: f64 = lots.iter().map(|l| l.quantity).sum();
                if total <= EPS {
                    continue;
                }
                let taken = quantity.min(total);
                for lot in lots.iter_mut() {
                    let share = taken * lot.quantity / total;
                    lot.estoque_gain *= 1.0 - share / lot.quantity;
                    lot.cc_gain += share * (quota_value - lot.cost_per_quota);
                    lot.cc_paid += share * quota_value;
                    lot.quantity -= share;
                }
            }
            FundEventKind::Estoque { quota_value, rate } => {
                // O imposto saiu em dinheiro, sem tirar cotas: o ganho já tributado e o imposto pago ficam no lote, como no
                // come-cotas, e o resgate futuro os conta (rendimento nominal e crédito).
                let mut base = 0.0;
                for lot in lots.iter_mut().filter(|l| l.quantity > EPS) {
                    let gain = lot.quantity * (quota_value - lot.cost_per_quota).max(0.0);
                    base += gain;
                    lot.estoque_gain += gain;
                    lot.cc_gain += gain;
                    lot.cc_paid += rate * gain;
                }
                estoque = Some(EstoqueEstimate { date: event.date, quota_value, rate, base, tax: rate * base });
            }
            FundEventKind::Redeem { tx_id, quantity, quota_value } => {
                let mut remaining = quantity;
                let mut parts = Vec::new();
                let (mut cost, mut iof_total, mut ir_total) = (0.0, 0.0, 0.0);
                let (mut credit_total, mut excess_total) = (0.0, 0.0);
                let mut loss_total = 0.0;
                for lot in lots.iter_mut().filter(|l| l.quantity > EPS) {
                    if remaining <= EPS {
                        break;
                    }
                    let take = remaining.min(lot.quantity);
                    let o = settle_lot(lot, take, quota_value, event.date, term, rules)?;

                    cost += o.cost;
                    iof_total += o.iof;
                    ir_total += o.ir;
                    credit_total += o.credit_used;
                    excess_total += o.excess;
                    loss_total += o.loss;
                    parts.push(LotPart {
                        lot_date: lot.date,
                        quotas: take,
                        days: o.days,
                        rate: o.rate,
                        gain: o.nominal,
                        iof: o.iof,
                        ir: o.ir,
                    });

                    lot.estoque_gain *= 1.0 - take / lot.quantity;
                    lot.quantity -= take;
                    lot.cc_gain -= o.cc_gain;
                    lot.cc_paid -= o.cc_paid;
                    remaining -= take;
                }
                let gross = quantity * quota_value;
                estimates.push(RedemptionEstimate {
                    tx_id,
                    date: event.date,
                    quotas: quantity,
                    quota_value,
                    gross,
                    cost,
                    iof: iof_total,
                    ir: ir_total,
                    come_cotas_credit: credit_total,
                    net: gross - iof_total - ir_total,
                    lots: parts,
                    uncovered_quotas: remaining.max(0.0),
                    excess_come_cotas: excess_total,
                    loss_created: loss_total,
                    loss_credit_used: 0.0,
                    ir_before_compensation: ir_total,
                    is_amortization: false,
                });
            }
            FundEventKind::Amortize { tx_id, amount, quota_value } => {
                // As cotas ficam: cada lote devolve a mesma fração `f` do que tem, calculada sobre o valor da posição.
                let total: f64 = lots.iter().map(|l| l.quantity).sum();
                let position_value = total * quota_value;
                if total <= EPS || position_value <= EPS {
                    continue;
                }
                let fraction = (amount / position_value).min(1.0);
                let mut parts = Vec::new();
                let (mut cost, mut iof_total, mut ir_total) = (0.0, 0.0, 0.0);
                let (mut credit_total, mut excess_total, mut loss_total) = (0.0, 0.0, 0.0);
                let mut equivalent_quotas = 0.0;
                for lot in lots.iter_mut().filter(|l| l.quantity > EPS) {
                    let take = fraction * lot.quantity;
                    let o = settle_lot(lot, take, quota_value, event.date, term, rules)?;

                    cost += o.cost;
                    iof_total += o.iof;
                    ir_total += o.ir;
                    credit_total += o.credit_used;
                    excess_total += o.excess;
                    loss_total += o.loss;
                    equivalent_quotas += take;
                    parts.push(LotPart {
                        lot_date: lot.date,
                        quotas: take,
                        days: o.days,
                        rate: o.rate,
                        gain: o.nominal,
                        iof: o.iof,
                        ir: o.ir,
                    });

                    // O custo e o que já foi tributado caem na mesma fração; a quantidade de cotas não muda.
                    lot.cost_per_quota *= 1.0 - fraction;
                    lot.estoque_gain *= 1.0 - fraction;
                    lot.cc_gain -= o.cc_gain;
                    lot.cc_paid -= o.cc_paid;
                }
                let gross = equivalent_quotas * quota_value;
                estimates.push(RedemptionEstimate {
                    tx_id,
                    date: event.date,
                    quotas: equivalent_quotas,
                    quota_value,
                    gross,
                    cost,
                    iof: iof_total,
                    ir: ir_total,
                    come_cotas_credit: credit_total,
                    net: gross - iof_total - ir_total,
                    lots: parts,
                    uncovered_quotas: 0.0,
                    excess_come_cotas: excess_total,
                    loss_created: loss_total,
                    loss_credit_used: 0.0,
                    ir_before_compensation: ir_total,
                    is_amortization: true,
                });
            }
        }
    }
    let open_quotas: f64 = lots.iter().map(|l| l.quantity).filter(|q| *q > EPS).sum();
    let taxed_basis_per_quota = (estoque.is_some() && open_quotas > EPS).then(|| {
        lots.iter().filter(|l| l.quantity > EPS).map(|l| l.quantity * l.cost_per_quota + l.estoque_gain).sum::<f64>()
            / open_quotas
    });
    Ok(FundEstimate { redemptions: estimates, estoque, taxed_basis_per_quota })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax::rules::test_rule_set;
    use chrono::Duration;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn buy(id: i32, date: &str, quantity: f64, cost: f64) -> FundEvent {
        FundEvent { date: d(date), id, kind: FundEventKind::Buy { quantity, cost } }
    }

    fn come_cotas(id: i32, date: &str, quantity: f64, quota_value: f64) -> FundEvent {
        FundEvent { date: d(date), id, kind: FundEventKind::ComeCotas { quantity, quota_value } }
    }

    fn redeem(id: i32, date: &str, quantity: f64, quota_value: f64) -> FundEvent {
        FundEvent { date: d(date), id, kind: FundEventKind::Redeem { tx_id: id, quantity, quota_value } }
    }

    fn run(events: &[FundEvent], term: FundTerm) -> Vec<RedemptionEstimate> {
        estimate_redemptions(events, term, &test_rule_set()).unwrap()
    }

    /// Resgate total de 100 cotas compradas a 10,00 e resgatadas a 12,00 depois de `days` dias.
    fn after(days: i64, term: FundTerm) -> RedemptionEstimate {
        let bought = d("2025-01-01");
        let redeemed = (bought + Duration::days(days)).to_string();
        run(&[buy(1, "2025-01-01", 100.0, 1_000.0), redeem(2, &redeemed, 100.0, 12.0)], term).remove(0)
    }

    #[test]
    fn a_long_term_fund_follows_the_regressive_table() {
        // rendimento de 100 × (12 − 10) = 200; IOF zero acima de 30 dias
        assert!(close(after(100, FundTerm::LongTerm).ir, 45.0), "22,5% até 180 dias");
        assert!(close(after(200, FundTerm::LongTerm).ir, 40.0), "20% de 181 a 360");
        assert!(close(after(500, FundTerm::LongTerm).ir, 35.0), "17,5% de 361 a 720");
        assert!(close(after(800, FundTerm::LongTerm).ir, 30.0), "15% acima de 720");
        let edge = after(180, FundTerm::LongTerm);
        assert!(close(edge.lots[0].rate, 0.225), "180 dias ainda é a primeira faixa");
        assert!(close(after(181, FundTerm::LongTerm).lots[0].rate, 0.20));
    }

    #[test]
    fn a_short_term_fund_has_two_bands() {
        assert!(close(after(100, FundTerm::ShortTerm).ir, 45.0), "22,5% até 180 dias");
        assert!(close(after(500, FundTerm::ShortTerm).ir, 40.0), "20% acima de 180");
        assert!(close(after(800, FundTerm::ShortTerm).ir, 40.0), "sem faixas de 17,5% e 15%");
    }

    #[test]
    fn the_net_value_is_the_gross_minus_iof_and_ir() {
        let e = after(800, FundTerm::LongTerm);
        assert!(close(e.gross, 1_200.0) && close(e.cost, 1_000.0) && close(e.net, 1_170.0), "{e:?}");
    }

    #[test]
    fn a_partial_redemption_takes_the_oldest_lot_first() {
        // lote A (01/01/2025, 100 a 10,00) e lote B (01/06/2025, 100 a 11,00); resgate de 150 a 12,00 em 01/03/2026
        // A: 424 dias, 17,5% sobre 200 = 35; B: 50 cotas, 273 dias, 20% sobre 50 = 10
        let events = [
            buy(1, "2025-01-01", 100.0, 1_000.0),
            buy(2, "2025-06-01", 100.0, 1_100.0),
            redeem(3, "2026-03-01", 150.0, 12.0),
            redeem(4, "2026-03-02", 50.0, 12.0),
        ];
        let result = run(&events, FundTerm::LongTerm);
        let first = &result[0];
        assert_eq!(first.lots.len(), 2);
        assert!(close(first.lots[0].quotas, 100.0) && close(first.lots[0].ir, 35.0), "{first:?}");
        assert!(close(first.lots[1].quotas, 50.0) && close(first.lots[1].ir, 10.0), "{first:?}");
        assert!(close(first.ir, 45.0) && close(first.cost, 1_550.0), "{first:?}");
        // o que sobrou é o resto do lote B (50 cotas a 11,00): 274 dias, 20% sobre 50 = 10
        let second = &result[1];
        assert_eq!(second.lots.len(), 1);
        assert_eq!(second.lots[0].lot_date, d("2025-06-01"));
        assert!(close(second.lots[0].quotas, 50.0) && close(second.ir, 10.0), "{second:?}");
    }

    #[test]
    fn an_equity_fund_pays_15_percent_and_no_iof() {
        // 5 dias de aplicação: sem IOF, 15% sobre 200
        let e = run(&[buy(1, "2026-03-01", 100.0, 1_000.0), redeem(2, "2026-03-06", 100.0, 12.0)], FundTerm::Equity)
            .remove(0);
        assert!(close(e.iof, 0.0) && close(e.ir, 30.0), "{e:?}");
    }

    #[test]
    fn the_iof_comes_off_the_gain_before_the_ir() {
        // 10 dias: IOF 66% de 200 = 132; IR 22,5% sobre 68 = 15,30; líquido 1.200 − 132 − 15,30 = 1.052,70
        let e = run(&[buy(1, "2026-03-01", 100.0, 1_000.0), redeem(2, "2026-03-11", 100.0, 12.0)], FundTerm::LongTerm)
            .remove(0);
        assert!(close(e.iof, 132.0) && close(e.ir, 15.3) && close(e.net, 1_052.7), "{e:?}");
    }

    #[test]
    fn a_redemption_on_the_day_of_the_purchase_has_the_first_day_iof() {
        let e = run(&[buy(1, "2026-03-01", 10.0, 100.0), redeem(2, "2026-03-01", 10.0, 12.0)], FundTerm::ShortTerm)
            .remove(0);
        assert!(close(e.iof, 0.96 * 20.0), "{e:?}");
    }

    #[test]
    fn the_come_cotas_already_paid_comes_off_the_redemption_ir() {
        // 100 cotas a 10,00; come-cotas em 31/05/2024 de 2,5 cotas a 12,00 (imposto 30, ganho já tributado 5);
        // resgate de 97,5 cotas a 15,00 em 01/03/2026 (789 dias, 15%): rendimento 487,50 + 5 = 492,50;
        // IR 73,875 − 30 = 43,875; líquido 1.462,50 − 43,875 = 1.418,625
        let events = [
            buy(1, "2024-01-02", 100.0, 1_000.0),
            come_cotas(2, "2024-05-31", 2.5, 12.0),
            redeem(3, "2026-03-01", 97.5, 15.0),
        ];
        let e = run(&events, FundTerm::LongTerm).remove(0);
        assert!(close(e.gross, 1_462.5) && close(e.cost, 975.0), "{e:?}");
        assert!(close(e.lots[0].gain, 492.5) && close(e.ir, 43.875), "{e:?}");
        assert!(close(e.come_cotas_credit, 30.0) && close(e.excess_come_cotas, 0.0), "{e:?}");
        assert!(close(e.net, 1_418.625), "{e:?}");
    }

    #[test]
    fn a_come_cotas_larger_than_the_tax_due_is_an_excess_not_a_refund() {
        // mesmo come-cotas (30 pagos), mas o resgate sai a 9,00: rendimento −97,50 + 5 = −92,50, IR zero, 30 de excesso
        let events = [
            buy(1, "2024-01-02", 100.0, 1_000.0),
            come_cotas(2, "2024-05-31", 2.5, 12.0),
            redeem(3, "2026-03-01", 97.5, 9.0),
        ];
        let e = run(&events, FundTerm::LongTerm).remove(0);
        assert!(close(e.ir, 0.0) && close(e.come_cotas_credit, 0.0) && close(e.excess_come_cotas, 30.0), "{e:?}");
        assert!(e.ir.is_sign_positive());
    }

    #[test]
    fn a_loss_pays_no_ir_or_iof() {
        let e = run(&[buy(1, "2026-03-01", 100.0, 1_000.0), redeem(2, "2026-03-05", 100.0, 9.0)], FundTerm::LongTerm)
            .remove(0);
        assert!(close(e.ir, 0.0) && close(e.iof, 0.0) && close(e.net, 900.0), "{e:?}");
        assert!(e.ir.is_sign_positive() && e.iof.is_sign_positive());
    }

    #[test]
    fn a_redemption_above_the_known_lots_is_flagged_and_taxed_only_on_the_covered_part() {
        let e = run(&[buy(1, "2025-01-01", 10.0, 100.0), redeem(2, "2026-06-01", 15.0, 12.0)], FundTerm::LongTerm)
            .remove(0);
        assert!(close(e.uncovered_quotas, 5.0), "{e:?}");
        // 10 cotas × (12 − 10) = 20 a 17,5% (516 dias)
        assert!(close(e.ir, 3.5) && close(e.gross, 180.0), "{e:?}");
    }

    #[test]
    fn the_events_can_come_in_any_order() {
        let events = [redeem(2, "2026-03-01", 100.0, 12.0), buy(1, "2025-01-01", 100.0, 1_000.0)];
        assert!(close(run(&events, FundTerm::LongTerm)[0].ir, 35.0));
    }

    fn amortize(id: i32, date: &str, amount: f64, quota_value: f64) -> FundEvent {
        FundEvent { date: d(date), id, kind: FundEventKind::Amortize { tx_id: id, amount, quota_value } }
    }

    fn estoque(date: &str, quota_value: f64, rate: f64) -> FundEvent {
        FundEvent { date: d(date), id: 0, kind: FundEventKind::Estoque { quota_value, rate } }
    }

    fn fund(events: &[FundEvent], term: FundTerm) -> FundEstimate {
        estimate_fund(events, term, &test_rule_set()).unwrap()
    }

    #[test]
    fn an_amortization_returns_capital_taxes_the_gain_share_and_keeps_the_quotas() {
        // 100 cotas a 10,00 (custo 1.000); amortização de 600 a cota 12,00 em 01/03/2026: o valor da posição é 1.200, então a
        // fração é 0,5 (50 cotas equivalentes); 424 dias (17,5%): rendimento 50 × (12 − 10) = 100, IR 17,50; custo baixado 500
        let events = [buy(1, "2025-01-01", 100.0, 1_000.0), amortize(2, "2026-03-01", 600.0, 12.0)];
        let result = fund(&events, FundTerm::LongTerm);
        let a = &result.redemptions[0];
        assert!(a.is_amortization && close(a.quotas, 50.0) && close(a.gross, 600.0), "{a:?}");
        assert!(close(a.cost, 500.0) && close(a.iof, 0.0) && close(a.ir, 17.5) && close(a.net, 582.5), "{a:?}");
        assert!(close(a.lots[0].rate, 0.175) && a.lots[0].days == 424, "{a:?}");

        // as 100 cotas ficam, com o custo pela metade (5,00 por cota): resgate de tudo a 6,00 em 455 dias rende 100 × 1 = 100
        let mut more = events.to_vec();
        more.push(redeem(3, "2026-04-01", 100.0, 6.0));
        let r = &fund(&more, FundTerm::LongTerm).redemptions[1];
        assert!(!r.is_amortization && close(r.quotas, 100.0) && close(r.cost, 500.0), "{r:?}");
        assert!(close(r.ir, 17.5), "{r:?}");
        assert!(r.uncovered_quotas.abs() < 1e-9);
    }

    #[test]
    fn an_amortization_also_scales_the_come_cotas_already_paid() {
        // 100 cotas a 10,00; come-cotas de 2,5 cotas a 12,00 (ganho já tributado 5, imposto pago 30), sobram 97,5. Amortização
        // de 731,25 a 15,00 em 01/03/2026: posição 97,5 × 15 = 1.462,50, fração 0,5 (48,75 cotas); 789 dias (15%):
        // rendimento 48,75 × 5 = 243,75 + 2,5 já tributado = 246,25; IR devido 36,9375 − 15 já pago = 21,9375
        let events = [
            buy(1, "2024-01-02", 100.0, 1_000.0),
            come_cotas(2, "2024-05-31", 2.5, 12.0),
            amortize(3, "2026-03-01", 731.25, 15.0),
        ];
        let a = &fund(&events, FundTerm::LongTerm).redemptions[0];
        assert!(close(a.quotas, 48.75) && close(a.cost, 487.5), "{a:?}");
        assert!(close(a.lots[0].gain, 246.25) && close(a.ir, 21.9375), "{a:?}");
        assert!(close(a.come_cotas_credit, 15.0) && close(a.excess_come_cotas, 0.0), "{a:?}");
        assert!(close(a.net, 731.25 - 21.9375), "{a:?}");
    }

    #[test]
    fn an_amortization_bigger_than_the_position_is_capped_and_one_without_quotas_is_ignored() {
        // posição de 100 × 12 = 1.200; amortização de 5.000 vira a fração 1 (todo o custo e as 100 cotas equivalentes)
        let capped = fund(&[buy(1, "2025-01-01", 100.0, 1_000.0), amortize(2, "2026-03-01", 5_000.0, 12.0)], FundTerm::LongTerm);
        assert!(close(capped.redemptions[0].quotas, 100.0) && close(capped.redemptions[0].cost, 1_000.0));
        // sem lote aberto não há o que amortizar
        assert!(fund(&[amortize(1, "2026-03-01", 100.0, 12.0)], FundTerm::LongTerm).redemptions.is_empty());
    }

    #[test]
    fn an_investment_entity_pays_a_flat_15_percent_with_no_iof_at_any_term() {
        // 5 dias de aplicação: sem IOF e sem tabela por prazo, 15% sobre o rendimento de 200
        let e = run(&[buy(1, "2026-03-01", 100.0, 1_000.0), redeem(2, "2026-03-06", 100.0, 12.0)], FundTerm::InvestmentEntity)
            .remove(0);
        assert!(close(e.iof, 0.0) && close(e.ir, 30.0), "{e:?}");
        // a mesma coisa numa amortização (50 cotas equivalentes, rendimento 100)
        let a = &fund(
            &[buy(1, "2026-03-01", 100.0, 1_000.0), amortize(2, "2026-03-06", 600.0, 12.0)],
            FundTerm::InvestmentEntity,
        )
        .redemptions[0];
        assert!(close(a.iof, 0.0) && close(a.ir, 15.0), "{a:?}");
    }

    #[test]
    fn the_2023_stock_taxes_the_gain_in_cash_and_counts_as_tax_already_paid() {
        // lote 1: 100 cotas a 10,00 (01/06/2022); lote 2: 50 cotas a 20,00 (acima da cota de 14,00: sem ganho); lote 3:
        // 100 cotas a 10,00 em 01/02/2024 (depois da data: fora). Cota de 31/12/2023 a 14,00: base = 100 × 4 = 400
        let events = [
            buy(1, "2022-06-01", 100.0, 1_000.0),
            buy(2, "2023-01-01", 50.0, 1_000.0),
            buy(3, "2024-02-01", 100.0, 1_000.0),
            estoque("2023-12-31", 14.0, 0.15),
        ];
        let result = fund(&events, FundTerm::LongTerm);
        let s = result.estoque.expect("o estoque");
        assert!(close(s.base, 400.0) && close(s.tax, 60.0) && close(s.rate, 0.15) && close(s.quota_value, 14.0), "{s:?}");

        // resgate das 100 cotas do lote 1 a 15,00 em 01/03/2026 (1.369 dias, 15%): rendimento 500 + 400 já tributados = 900,
        // IR devido 135, menos os 60 pagos do estoque = 75
        let mut more = events.to_vec();
        more.push(redeem(4, "2026-03-01", 100.0, 15.0));
        let r = &fund(&more, FundTerm::LongTerm).redemptions[0];
        assert!(close(r.lots[0].gain, 900.0) && close(r.ir, 75.0) && close(r.come_cotas_credit, 60.0), "{r:?}");
        assert!(close(r.cost, 1_000.0) && close(r.excess_come_cotas, 0.0), "{r:?}");
    }

    #[test]
    fn the_8_percent_option_pays_less_now_and_credits_less_later() {
        let events = [
            buy(1, "2022-06-01", 100.0, 1_000.0),
            estoque("2023-12-31", 14.0, 0.08),
            redeem(2, "2026-03-01", 100.0, 15.0),
        ];
        let result = fund(&events, FundTerm::LongTerm);
        let s = result.estoque.expect("o estoque");
        // 8% de 400 = 32 pagos; IR devido 135 − 32 = 103
        assert!(close(s.tax, 32.0), "{s:?}");
        assert!(close(result.redemptions[0].ir, 103.0), "{:?}", result.redemptions[0]);
    }

    #[test]
    fn the_taxed_basis_adds_the_stock_to_the_cost_and_falls_with_amortizations_only() {
        // 100 cotas a 10,00 e estoque a 14,00: o ganho tributado em 2023 é 4,00 por cota, então o custo vira 14,00
        let mut events = vec![buy(1, "2022-06-01", 100.0, 1_000.0), estoque("2023-12-31", 14.0, 0.15)];
        assert!(close(fund(&events, FundTerm::LongTerm).taxed_basis_per_quota.unwrap(), 14.0));

        // come-cotas de 5 cotas e resgate de 45: as cotas saem com a sua parte, o valor por cota não muda
        events.push(come_cotas(2, "2024-05-31", 5.0, 15.0));
        events.push(redeem(3, "2024-08-01", 45.0, 15.0));
        assert!(close(fund(&events, FundTerm::LongTerm).taxed_basis_per_quota.unwrap(), 14.0));

        // amortização de 300 sobre 50 cotas a 15,00 (posição de 750): fração 0,4, e o custo e o estoque caem 40%
        events.push(amortize(4, "2025-01-02", 300.0, 15.0));
        assert!(close(fund(&events, FundTerm::LongTerm).taxed_basis_per_quota.unwrap(), 14.0 * 0.6));
    }

    #[test]
    fn the_taxed_basis_only_exists_with_the_stock_event_and_open_quotas() {
        let buy_only = [buy(1, "2022-06-01", 100.0, 1_000.0)];
        assert!(fund(&buy_only, FundTerm::LongTerm).taxed_basis_per_quota.is_none());
        // cota de 2023 abaixo do custo: nada foi tributado, a base é o custo
        let low = [buy_only[0], estoque("2023-12-31", 8.0, 0.15)];
        assert!(close(fund(&low, FundTerm::LongTerm).taxed_basis_per_quota.unwrap(), 10.0));
        // sem cotas abertas depois do resgate total
        let sold = [buy_only[0], estoque("2023-12-31", 14.0, 0.15), redeem(2, "2024-03-01", 100.0, 15.0)];
        assert!(fund(&sold, FundTerm::LongTerm).taxed_basis_per_quota.is_none());
    }

    #[test]
    fn the_stock_with_no_open_lot_or_no_gain_is_zero_and_without_the_event_there_is_none() {
        // cota de 31/12/2023 abaixo do custo: base zero, nada de crédito
        let low = fund(&[buy(1, "2022-06-01", 100.0, 1_000.0), estoque("2023-12-31", 8.0, 0.15)], FundTerm::LongTerm);
        assert!(close(low.estoque.unwrap().base, 0.0));
        // sem o evento não há estimativa de estoque e os resgates seguem como antes
        let none = fund(&[buy(1, "2022-06-01", 100.0, 1_000.0), redeem(2, "2026-03-01", 100.0, 15.0)], FundTerm::LongTerm);
        assert!(none.estoque.is_none() && close(none.redemptions[0].ir, 75.0), "{:?}", none.redemptions[0]);
    }
}
