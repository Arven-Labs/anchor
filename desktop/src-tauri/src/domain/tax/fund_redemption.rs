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
//   IR = max(0, (nominal − IOF) × alíquota − come-cotas pago); o excesso de come-cotas vira aviso (compensação não modelada).
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
    /// No mesmo dia: aplicação, come-cotas, resgate.
    fn order_key(&self) -> (NaiveDate, u8, i32) {
        let rank = match self.kind {
            FundEventKind::Buy { .. } => 0,
            FundEventKind::ComeCotas { .. } => 1,
            FundEventKind::Redeem { .. } => 2,
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
}

struct Lot {
    date: NaiveDate,
    quantity: f64,
    cost_per_quota: f64,
    /// Ganho já tributado pelos come-cotas, ligado às cotas que restam no lote.
    cc_gain: f64,
    /// Come-cotas já pago, ligado às cotas que restam no lote.
    cc_paid: f64,
}

/// Alíquota da tabela para `days` dias de aplicação.
fn table_rate(term: FundTerm, days: i64, rules: &RuleSet, on: NaiveDate) -> Result<f64, AppError> {
    let key = match term {
        FundTerm::Equity => rules::FUND_IR_EQUITY,
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

/// Estima o IOF e o IR de cada resgate do fundo.
pub fn estimate_redemptions(
    events: &[FundEvent],
    term: FundTerm,
    rules: &RuleSet,
) -> Result<Vec<RedemptionEstimate>, AppError> {
    let mut ordered: Vec<&FundEvent> = events.iter().collect();
    ordered.sort_by_key(|e| e.order_key());

    let mut lots: Vec<Lot> = Vec::new();
    let mut estimates = Vec::new();
    for event in ordered {
        match event.kind {
            FundEventKind::Buy { quantity, cost } => {
                if quantity > EPS {
                    lots.push(Lot {
                        date: event.date,
                        quantity,
                        cost_per_quota: cost / quantity,
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
                    lot.cc_gain += share * (quota_value - lot.cost_per_quota);
                    lot.cc_paid += share * quota_value;
                    lot.quantity -= share;
                }
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
                    let fraction = take / lot.quantity;
                    let days = (event.date - lot.date).num_days();

                    let redeem_gain = take * (quota_value - lot.cost_per_quota);
                    let cc_gain = lot.cc_gain * fraction;
                    let cc_paid = lot.cc_paid * fraction;
                    let nominal = redeem_gain + cc_gain;
                    // O IOF não existe em fundo de ações.
                    let iof = if term == FundTerm::Equity { 0.0 } else { iof_rate_for_days(days) * redeem_gain.max(0.0) };
                    let rate = table_rate(term, days, rules, event.date)?;
                    let ir_due = (nominal - iof).max(0.0) * rate;
                    let ir = (ir_due - cc_paid).max(0.0);
                    let credit_used = cc_paid.min(ir_due);

                    cost += take * lot.cost_per_quota;
                    iof_total += iof;
                    ir_total += ir;
                    credit_total += credit_used;
                    excess_total += (cc_paid - ir_due).max(0.0);
                    loss_total += (iof - nominal).max(0.0);
                    parts.push(LotPart { lot_date: lot.date, quotas: take, days, rate, gain: nominal, iof, ir });

                    lot.quantity -= take;
                    lot.cc_gain -= cc_gain;
                    lot.cc_paid -= cc_paid;
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
                });
            }
        }
    }
    Ok(estimates)
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
}
