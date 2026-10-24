// Compensação de prejuízo entre fundos abertos. A perda apurada num resgate abate os rendimentos de resgates e de
// come-cotas POSTERIORES, no mesmo fundo ou em outro da mesma administradora (ou do mesmo intermediário por conta e
// ordem), desde que da mesma classificação (longo prazo, curto prazo; fundos de ações à parte). Não é retroativa
// (IN RFB 1.585/2015, art. 15).
//
// Premissas (todas a conferir na Receita; as regras vêm de fontes secundárias):
// - Grupo = (administradora normalizada, tipo do fundo). Fundo sem administradora informada fica isolado: compensa só consigo.
// - Perda de um resgate = soma, por lote, de `max(0, IOF − rendimento nominal)`. Os lotes seguem independentes, como em
//   `estimate_redemptions`: ganho e perda do mesmo resgate não se anulam.
// - O crédito de um resgate é o das perdas de datas ESTRITAMENTE anteriores do grupo ("posteriores", na lei); no mesmo dia
//   não compensa.
// - O crédito abate o rendimento dos lotes do resgate, do mais antigo ao mais novo, mas só até zerar o IR a pagar do lote:
//   o que o come-cotas já cobriu não gasta crédito. O IOF não é abatido.
// - O saldo do grupo abate o ganho da estimativa do próximo come-cotas (fundos em ordem de ticker, sem usar o mesmo crédito
//   duas vezes). Descontos de come-cotas JÁ registrados não consomem crédito (não dá para saber quanto a administradora
//   abateu), e o crédito não tem validade modelada (as fontes divergem sobre o prazo).

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::Serialize;

use super::come_cotas::{ComeCotasEstimate, EstimateStatus, FundTerm};
use super::fund_redemption::RedemptionEstimate;
use crate::domain::cost_basis::EPS;

/// Um fundo na compensação: quem é, a que grupo pertence e os resgates já estimados (alterados no lugar).
pub struct CompFund<'a> {
    pub asset_id: i32,
    pub ticker: &'a str,
    pub term: FundTerm,
    pub administrator: Option<&'a str>,
    pub redemptions: &'a mut [RedemptionEstimate],
}

/// O saldo de prejuízo a compensar de um grupo.
#[derive(Clone, Debug, Serialize)]
pub struct LossCredit {
    /// A administradora como o usuário escreveu; `None` = fundo sem administradora (grupo de um fundo só).
    pub administrator: Option<String>,
    pub term: FundTerm,
    pub tickers: Vec<String>,
    pub asset_ids: Vec<i32>,
    /// Rendimento ainda abatível, em BRL.
    pub amount: f64,
}

/// Administradora sem espaços sobrando e em minúsculas; vazia vira `None`.
pub fn normalize_administrator(raw: Option<&str>) -> Option<String> {
    raw.map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()).filter(|s| !s.is_empty())
}

fn group_key(fund: &CompFund<'_>) -> String {
    let term = format!("{:?}", fund.term);
    match normalize_administrator(fund.administrator) {
        Some(administrator) => format!("adm:{administrator}|{term}"),
        None => format!("asset:{}|{term}", fund.asset_id),
    }
}

struct Pool {
    administrator: Option<String>,
    term: FundTerm,
    tickers: BTreeSet<String>,
    asset_ids: Vec<i32>,
    amount: f64,
}

/// Abate o IR a pagar dos lotes do resgate com o crédito disponível. Devolve quanto usou.
fn apply_credit(redemption: &mut RedemptionEstimate, available: &mut f64) -> f64 {
    let mut used = 0.0;
    for lot in redemption.lots.iter_mut() {
        if *available <= EPS {
            break;
        }
        if lot.rate <= 0.0 || lot.ir <= EPS {
            continue;
        }
        // Rendimento que ainda gera IR a pagar; abater mais que isso não reduz nada.
        let cut = (lot.ir / lot.rate).min(*available);
        lot.ir = (lot.ir - cut * lot.rate).max(0.0);
        *available -= cut;
        used += cut;
    }
    if used > EPS {
        redemption.ir = redemption.lots.iter().map(|l| l.ir).sum();
        redemption.net = redemption.gross - redemption.iof - redemption.ir;
        redemption.loss_credit_used = used;
    }
    used
}

/// Compensa os prejuízos entre os resgates de `funds` (todos os fundos que têm tipo, mesmo sem resgate) e devolve o saldo
/// de cada grupo. Cada resgate recebe `loss_created`, `loss_credit_used` e o `ir` já abatido.
pub fn compensate(funds: &mut [CompFund<'_>]) -> Vec<LossCredit> {
    let keys: Vec<String> = funds.iter().map(group_key).collect();
    let mut pools: BTreeMap<String, Pool> = BTreeMap::new();
    for (fund, key) in funds.iter().zip(&keys) {
        let pool = pools.entry(key.clone()).or_insert_with(|| Pool {
            administrator: fund.administrator.map(str::trim).filter(|a| !a.is_empty()).map(str::to_string),
            term: fund.term,
            tickers: BTreeSet::new(),
            asset_ids: Vec::new(),
            amount: 0.0,
        });
        pool.tickers.insert(fund.ticker.to_string());
        pool.asset_ids.push(fund.asset_id);
    }

    // (data, id da transação, fundo, resgate): o crédito de um dia só vale a partir do dia seguinte.
    let mut order: Vec<(NaiveDate, i32, usize, usize)> = Vec::new();
    for (f, fund) in funds.iter().enumerate() {
        for (r, redemption) in fund.redemptions.iter().enumerate() {
            order.push((redemption.date, redemption.tx_id, f, r));
        }
    }
    order.sort();

    let mut start = 0;
    while start < order.len() {
        let date = order[start].0;
        let end = start + order[start..].iter().take_while(|o| o.0 == date).count();
        for &(_, _, f, r) in &order[start..end] {
            let pool = pools.get_mut(&keys[f]).expect("o grupo existe");
            apply_credit(&mut funds[f].redemptions[r], &mut pool.amount);
        }
        for &(_, _, f, r) in &order[start..end] {
            let pool = pools.get_mut(&keys[f]).expect("o grupo existe");
            pool.amount += funds[f].redemptions[r].loss_created;
        }
        start = end;
    }

    pools
        .into_values()
        .filter(|p| p.amount > EPS)
        .map(|p| LossCredit {
            administrator: p.administrator,
            term: p.term,
            tickers: p.tickers.into_iter().collect(),
            asset_ids: p.asset_ids,
            amount: p.amount,
        })
        .collect()
}

/// Abate o ganho das estimativas de come-cotas com o saldo dos grupos em `credits`. Estimativas sem imposto, sem
/// grupo com saldo ou de fundo de ações (isento) ficam como estão.
pub fn apply_loss_credit(estimates: &mut [ComeCotasEstimate], credits: &[LossCredit]) {
    let mut remaining: Vec<f64> = credits.iter().map(|c| c.amount).collect();
    let mut order: Vec<usize> = (0..estimates.len()).collect();
    order.sort_by(|&a, &b| estimates[a].ticker.cmp(&estimates[b].ticker));
    for i in order {
        let estimate = &mut estimates[i];
        if estimate.status != EstimateStatus::Estimated || estimate.tax <= EPS || estimate.rate <= 0.0 {
            continue;
        }
        let Some(group) = credits.iter().position(|c| c.asset_ids.contains(&estimate.asset_id)) else { continue };
        let cut = estimate.gain.min(remaining[group]);
        if cut <= EPS {
            continue;
        }
        remaining[group] -= cut;
        estimate.loss_credit_used = cut;
        estimate.gain -= cut;
        estimate.tax = estimate.gain * estimate.rate;
        estimate.quotas_deducted = match estimate.quota_value {
            Some(quota) if quota > 0.0 => estimate.tax / quota,
            _ => 0.0,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax::come_cotas::{self, FundHolding, QuotaValue};
    use crate::domain::tax::fund_redemption::{estimate_redemptions, FundEvent, FundEventKind};
    use crate::domain::tax::rules::test_rule_set;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn buy(id: i32, date: &str, quantity: f64, cost: f64) -> FundEvent {
        FundEvent { date: d(date), id, kind: FundEventKind::Buy { quantity, cost } }
    }

    fn redeem(id: i32, date: &str, quantity: f64, quota_value: f64) -> FundEvent {
        FundEvent { date: d(date), id, kind: FundEventKind::Redeem { tx_id: id, quantity, quota_value } }
    }

    /// Um fundo de teste: `(asset_id, ticker, tipo, administradora, resgates estimados)`.
    struct Fixture {
        asset_id: i32,
        ticker: &'static str,
        term: FundTerm,
        administrator: Option<&'static str>,
        redemptions: Vec<RedemptionEstimate>,
    }

    fn fixture(
        asset_id: i32,
        ticker: &'static str,
        term: FundTerm,
        administrator: Option<&'static str>,
        events: &[FundEvent],
    ) -> Fixture {
        let redemptions = estimate_redemptions(events, term, &test_rule_set()).unwrap();
        Fixture { asset_id, ticker, term, administrator, redemptions }
    }

    fn run(fixtures: &mut [Fixture]) -> Vec<LossCredit> {
        let mut funds: Vec<CompFund<'_>> = fixtures
            .iter_mut()
            .map(|f| CompFund {
                asset_id: f.asset_id,
                ticker: f.ticker,
                term: f.term,
                administrator: f.administrator,
                redemptions: &mut f.redemptions,
            })
            .collect();
        compensate(&mut funds)
    }

    /// Fundo que perde: 100 cotas a 10,00 resgatadas a `quota` em 01/06/2025 (151 dias, sem IOF).
    fn loser(asset_id: i32, ticker: &'static str, term: FundTerm, admin: Option<&'static str>, quota: f64) -> Fixture {
        fixture(asset_id, ticker, term, admin, &[buy(1, "2025-01-01", 100.0, 1_000.0), redeem(2, "2025-06-01", 100.0, quota)])
    }

    /// Fundo que ganha: 100 cotas a 10,00 resgatadas a 12,00 em `date` (243 dias em 01/09/2025: faixa de 20%).
    fn winner(asset_id: i32, ticker: &'static str, term: FundTerm, admin: Option<&'static str>, date: &str) -> Fixture {
        fixture(asset_id, ticker, term, admin, &[buy(1, "2025-01-01", 100.0, 1_000.0), redeem(2, date, 100.0, 12.0)])
    }

    #[test]
    fn a_loss_in_one_fund_wipes_out_the_later_ir_of_another_fund_of_the_same_administrator() {
        // X perde 100 × (8 − 10) = 200; Y ganha 200 a 20% = 40 de IR, que a perda zera (40 ÷ 0,20 = 200 de crédito)
        let mut set = [
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0),
            winner(2, "YPTO11", FundTerm::LongTerm, Some("xp "), "2025-09-01"),
        ];
        let credits = run(&mut set);
        let (x, y) = (&set[0].redemptions[0], &set[1].redemptions[0]);
        assert!(close(x.loss_created, 200.0) && close(x.ir, 0.0), "{x:?}");
        assert!(close(y.ir_before_compensation, 40.0), "{y:?}");
        assert!(close(y.ir, 0.0) && close(y.loss_credit_used, 200.0) && close(y.net, 1_200.0), "{y:?}");
        assert!(credits.is_empty(), "o crédito foi todo usado: {credits:?}");
    }

    #[test]
    fn a_partial_credit_reduces_the_ir_proportionally() {
        // perda de 50 (100 × (9,5 − 10)); o ganho de 200 paga (200 − 50) × 20% = 30
        let mut set = [
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 9.5),
            winner(2, "YPTO11", FundTerm::LongTerm, Some("XP"), "2025-09-01"),
        ];
        run(&mut set);
        let y = &set[1].redemptions[0];
        assert!(close(y.ir, 30.0) && close(y.loss_credit_used, 50.0), "{y:?}");
        assert!(close(y.net, 1_200.0 - 30.0), "{y:?}");
    }

    #[test]
    fn different_administrators_or_terms_do_not_offset_each_other() {
        let mut other_admin = [
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0),
            winner(2, "YPTO11", FundTerm::LongTerm, Some("BTG"), "2025-09-01"),
        ];
        let credits = run(&mut other_admin);
        assert!(close(other_admin[1].redemptions[0].ir, 40.0));
        assert!(close(credits.iter().map(|c| c.amount).sum::<f64>(), 200.0), "o crédito fica no grupo da XP: {credits:?}");

        // curto prazo: 22,5% até 180 dias; 243 dias = 20%, mesmo IR de 40
        let mut other_term = [
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0),
            winner(2, "YPTO11", FundTerm::ShortTerm, Some("XP"), "2025-09-01"),
        ];
        run(&mut other_term);
        assert!(close(other_term[1].redemptions[0].ir, 40.0) && close(other_term[1].redemptions[0].loss_credit_used, 0.0));
    }

    #[test]
    fn a_gain_on_the_same_day_or_before_the_loss_gets_no_credit() {
        // o ganho de Y é no mesmo dia da perda de X (01/06/2025)
        let mut same_day = [
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0),
            fixture(2, "YPTO11", FundTerm::LongTerm, Some("XP"), &[buy(1, "2025-01-01", 100.0, 1_000.0), redeem(2, "2025-06-01", 100.0, 12.0)]),
        ];
        run(&mut same_day);
        // 151 dias: 22,5% sobre 200 = 45, sem abatimento
        assert!(close(same_day[1].redemptions[0].ir, 45.0) && close(same_day[1].redemptions[0].loss_credit_used, 0.0));

        // o ganho vem ANTES da perda: não é retroativo
        let mut before = [
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0),
            fixture(2, "YPTO11", FundTerm::LongTerm, Some("XP"), &[buy(1, "2025-01-01", 100.0, 1_000.0), redeem(2, "2025-05-01", 100.0, 12.0)]),
        ];
        let credits = run(&mut before);
        assert!(close(before[1].redemptions[0].loss_credit_used, 0.0));
        assert!(close(credits[0].amount, 200.0));
    }

    #[test]
    fn the_credit_is_used_lot_by_lot_from_the_oldest_and_runs_out() {
        // Y: lotes de 50 cotas (01/01 e 01/03/2025); resgate de 100 a 12,00 em 01/09/2025: os dois na faixa de 20%
        // (243 e 184 dias), rendimento 100 cada, IR 20 cada. Crédito 120: lote 1 usa 100 (IR 0), lote 2 usa 20 (IR 16)
        let mut set = [
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.8),
            fixture(
                2,
                "YPTO11",
                FundTerm::LongTerm,
                Some("XP"),
                &[buy(1, "2025-01-01", 50.0, 500.0), buy(2, "2025-03-01", 50.0, 500.0), redeem(3, "2025-09-01", 100.0, 12.0)],
            ),
        ];
        // X perde 100 × (8,8 − 10) = 120
        let credits = run(&mut set);
        let y = &set[1].redemptions[0];
        assert!(close(set[0].redemptions[0].loss_created, 120.0));
        assert!(close(y.lots[0].ir, 0.0) && close(y.lots[1].ir, 16.0), "{y:?}");
        assert!(close(y.ir, 16.0) && close(y.loss_credit_used, 120.0) && close(y.ir_before_compensation, 40.0), "{y:?}");
        assert!(credits.is_empty());
    }

    #[test]
    fn the_credit_is_spent_once_across_later_redemptions() {
        // perda de 100 (100 × (9 − 10)); duas vendas de ganho de 100 cada (IR de 20 a 20%): a 1ª usa 100, a 2ª nada
        let mut set = [
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 9.0),
            fixture(
                2,
                "YPTO11",
                FundTerm::LongTerm,
                Some("XP"),
                &[
                    buy(1, "2025-01-01", 100.0, 1_000.0),
                    redeem(2, "2025-09-01", 50.0, 14.0),
                    redeem(3, "2025-09-02", 50.0, 14.0),
                ],
            ),
        ];
        let credits = run(&mut set);
        let (first, second) = (&set[1].redemptions[0], &set[1].redemptions[1]);
        // cada resgate: 50 × (14 − 10) = 200 de rendimento, IR de 40; o crédito de 100 zera 100 de base na 1ª (IR 20)
        assert!(close(first.loss_credit_used, 100.0) && close(first.ir, 20.0), "{first:?}");
        assert!(close(second.loss_credit_used, 0.0) && close(second.ir, 40.0), "{second:?}");
        assert!(credits.is_empty());
    }

    #[test]
    fn a_fund_without_an_administrator_offsets_only_itself() {
        // Z perde num resgate e ganha noutro, sem administradora: compensa consigo mesmo; W, sem administradora, não
        let mut set = [
            fixture(
                1,
                "ZPTO11",
                FundTerm::LongTerm,
                None,
                &[
                    buy(1, "2025-01-01", 100.0, 1_000.0),
                    buy(2, "2025-02-01", 100.0, 1_000.0),
                    redeem(3, "2025-05-01", 100.0, 8.0),
                    redeem(4, "2025-09-01", 100.0, 12.0),
                ],
            ),
            winner(2, "WPTO11", FundTerm::LongTerm, None, "2025-09-01"),
        ];
        run(&mut set);
        let z = &set[0].redemptions;
        assert!(close(z[0].loss_created, 200.0));
        // 2º lote: 01/02 → 01/09 = 212 dias, 20% sobre 200 = 40, zerado pelo crédito de 200
        assert!(close(z[1].ir_before_compensation, 40.0) && close(z[1].ir, 0.0), "{z:?}");
        assert!(close(set[1].redemptions[0].ir, 40.0), "W não recebe crédito de Z");
    }

    #[test]
    fn the_come_cotas_already_paid_is_not_wasted_credit() {
        // lote de 100 cotas com rendimento 200 (IR 40) e come-cotas pago de 40: o IR a pagar já é zero, nada de crédito gasto
        let mut set = [
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0),
            fixture(
                2,
                "YPTO11",
                FundTerm::LongTerm,
                Some("XP"),
                &[
                    buy(1, "2025-01-01", 100.0, 1_000.0),
                    FundEvent { date: d("2025-05-30"), id: 2, kind: FundEventKind::ComeCotas { quantity: 3.0, quota_value: 12.0 } },
                    redeem(3, "2025-09-01", 97.0, 12.0),
                ],
            ),
        ];
        let credits = run(&mut set);
        let y = &set[1].redemptions[0];
        // rendimento nominal = 97 × 2 + 3 × 2 = 200, IR devido 40; come-cotas pago = 3 × 12 = 36 → IR a pagar 4
        assert!(close(y.ir_before_compensation, 4.0), "{y:?}");
        // o crédito só gasta o necessário para zerar os 4 de IR: 4 ÷ 0,20 = 20
        assert!(close(y.ir, 0.0) && close(y.loss_credit_used, 20.0), "{y:?}");
        assert!(close(credits[0].amount, 180.0), "{credits:?}");
    }

    #[test]
    fn without_losses_nothing_changes() {
        let mut set = [
            winner(1, "XPTO11", FundTerm::LongTerm, Some("XP"), "2025-09-01"),
            winner(2, "YPTO11", FundTerm::LongTerm, Some("XP"), "2025-10-01"),
        ];
        let credits = run(&mut set);
        for fixture in &set {
            let r = &fixture.redemptions[0];
            assert!(close(r.ir, r.ir_before_compensation) && close(r.loss_credit_used, 0.0) && close(r.loss_created, 0.0));
        }
        assert!(credits.is_empty());
    }

    #[test]
    fn the_administrator_is_compared_without_case_or_extra_spaces() {
        assert_eq!(normalize_administrator(Some("  XP   Investimentos ")), Some("xp investimentos".to_string()));
        assert_eq!(normalize_administrator(Some("   ")), None);
        assert_eq!(normalize_administrator(None), None);
    }

    fn holding(asset_id: i32, ticker: &str, quantity: f64, quota: f64) -> FundHolding {
        FundHolding {
            asset_id,
            ticker: ticker.to_string(),
            name: ticker.to_string(),
            term: Some(FundTerm::LongTerm),
            quantity,
            average_cost: 10.0,
            first_buy: Some(d("2026-07-01")),
            quotas: vec![QuotaValue { date: d("2026-10-20"), value: quota }],
            deductions: Vec::new(),
        }
    }

    #[test]
    fn the_group_balance_reduces_the_next_come_cotas_without_double_use() {
        // dois fundos do mesmo grupo, ganho de 200 cada (15% = 30 de imposto cada); saldo de 250
        let holdings = [holding(1, "AAA11", 100.0, 12.0), holding(2, "BBB11", 100.0, 12.0)];
        let mut estimates = come_cotas::estimate(&holdings, &test_rule_set(), d("2026-10-22")).unwrap();
        let credits = vec![LossCredit {
            administrator: Some("XP".to_string()),
            term: FundTerm::LongTerm,
            tickers: vec!["AAA11".to_string(), "BBB11".to_string()],
            asset_ids: vec![1, 2],
            amount: 250.0,
        }];

        apply_loss_credit(&mut estimates, &credits);

        // AAA11 (1º por ticker) usa 200 e zera; BBB11 usa os 50 que sobraram: ganho 150, imposto 22,50, 1,875 cotas
        let (a, b) = (&estimates[0], &estimates[1]);
        assert!(close(a.loss_credit_used, 200.0) && close(a.tax, 0.0) && close(a.tax_before_compensation, 30.0), "{a:?}");
        assert!(close(b.loss_credit_used, 50.0) && close(b.gain, 150.0) && close(b.tax, 22.5), "{b:?}");
        assert!(close(b.quotas_deducted, 1.875) && close(b.tax_before_compensation, 30.0), "{b:?}");
    }

    #[test]
    fn the_come_cotas_estimate_ignores_other_groups_and_exempt_funds() {
        let mut equity = holding(3, "CCC11", 100.0, 12.0);
        equity.term = Some(FundTerm::Equity);
        let holdings = [holding(1, "AAA11", 100.0, 12.0), equity];
        let mut estimates = come_cotas::estimate(&holdings, &test_rule_set(), d("2026-10-22")).unwrap();
        let credits = vec![LossCredit {
            administrator: Some("BTG".to_string()),
            term: FundTerm::LongTerm,
            tickers: vec!["ZZZ11".to_string()],
            asset_ids: vec![9],
            amount: 500.0,
        }];

        apply_loss_credit(&mut estimates, &credits);

        assert!(close(estimates[0].loss_credit_used, 0.0) && close(estimates[0].tax, 30.0));
        assert_eq!(estimates[1].status, EstimateStatus::Exempt);
    }
}
