// Compensação de prejuízo entre fundos abertos. A perda apurada num resgate abate os rendimentos de resgates e de
// come-cotas POSTERIORES, no mesmo fundo ou em outro da mesma administradora (ou do mesmo intermediário por conta e
// ordem), desde que da mesma classificação (longo prazo, curto prazo; fundos de ações à parte). Não é retroativa
// (IN RFB 1.585/2015, art. 15).
//
// Premissas (todas a conferir na Receita; as regras vêm de fontes secundárias):
// - Grupo = (administradora normalizada, tipo do fundo). Fundo sem administradora informada fica isolado: compensa só consigo.
// - Perda de um resgate = soma, por lote, da perda legal medida contra o custo atualizado (cotas × max(0, custo atualizado −
//   cota do resgate), IN 1.585 art. 15 §1) ou de `max(0, IOF − rendimento nominal)`, o que for maior (ver
//   `fund_redemption`). Os lotes seguem independentes, como em `estimate_redemptions`: ganho e perda do mesmo resgate não
//   se anulam.
// - O crédito de um resgate é o das perdas de datas ESTRITAMENTE anteriores do grupo ("posteriores", na lei); no mesmo dia
//   não compensa.
// - O crédito abate o rendimento dos lotes do resgate, do mais antigo ao mais novo, mas só até zerar o IR a pagar do lote:
//   o que o come-cotas já cobriu não gasta crédito. O IOF não é abatido.
// - O saldo do grupo abate o ganho da estimativa do próximo come-cotas (fundos em ordem de ticker, sem usar o mesmo crédito
//   duas vezes).
// - Descontos de come-cotas JÁ registrados: o usuário informa (campo opcional da transação) quanto rendimento o crédito de
//   prejuízo cobriu ali, e isso abate o saldo do grupo na data (no máximo o saldo). Sem o valor (nulo), o app não sabe quanto
//   a administradora abateu e só avisa (`CreditNote`, `informed = None`) quando o grupo tinha saldo de perdas de datas
//   estritamente anteriores: o saldo mostrado pode estar acima do real. Valor informado acima do saldo também avisa
//   (`informed = Some`). Valor informado dentro do saldo, inclusive 0, não avisa.
// - O crédito não tem validade. O art. 15 §2 da IN 1.585 manda a administradora guardar o registro das perdas até o fim do
//   ano seguinte ao resgate total em todos os fundos dela, mas isso é garantia mínima de guarda e não limita o direito de
//   compensar (Solução de Consulta Cosit 38/2016, sobre o mesmo parágrafo da IN anterior). O app só avisa (`record_notes`).

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, NaiveDate};
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
    /// Os descontos de come-cotas já registrados do fundo.
    pub come_cotas: Vec<ComeCotasMark>,
}

/// Um desconto de come-cotas registrado: a data e o crédito de prejuízo que o usuário disse ter sido usado nele.
#[derive(Clone, Copy, Debug)]
pub struct ComeCotasMark {
    pub date: NaiveDate,
    /// Rendimento coberto pelo crédito, em BRL; `None` = não informado (0 = nenhum crédito usado).
    pub loss_credit_used: Option<f64>,
}

/// Aviso de um come-cotas registrado em que o crédito de prejuízo do grupo pode não estar certo.
#[derive(Clone, Debug, Serialize)]
pub struct CreditNote {
    pub asset_id: i32,
    /// Data do come-cotas registrado.
    pub date: NaiveDate,
    /// Saldo do grupo imediatamente antes da data (perdas de datas anteriores, já descontado o que resgates gastaram), em BRL.
    pub balance: f64,
    /// `None`: o usuário não informou o crédito usado e o grupo tinha saldo (a administradora pode ter usado parte).
    /// `Some(valor)`: ele informou `valor`, acima do saldo.
    pub informed: Option<f64>,
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

/// Saldo abaixo disto não gera aviso (ruído de arredondamento).
const NOTE_MIN_BALANCE: f64 = 0.005;

/// Compensa os prejuízos entre os resgates de `funds` (todos os fundos que têm tipo, mesmo sem resgate) e devolve o saldo
/// de cada grupo, mais um aviso por come-cotas registrado feito com saldo no grupo. Cada resgate recebe `loss_created`,
/// `loss_credit_used` e o `ir` já abatido.
pub fn compensate_with_notes(funds: &mut [CompFund<'_>]) -> (Vec<LossCredit>, Vec<CreditNote>) {
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

    // (data, fundo, crédito informado) de cada come-cotas registrado. O saldo de uma data é o de antes do bloco dessa data: só
    // perdas de datas estritamente anteriores contam, como no crédito dos resgates. O crédito informado sai do saldo (no máximo
    // o saldo) e um valor acima do saldo, ou nenhum valor com saldo no grupo, gera aviso.
    let mut cc: Vec<(NaiveDate, usize, Option<f64>)> = Vec::new();
    for (f, fund) in funds.iter().enumerate() {
        cc.extend(fund.come_cotas.iter().map(|m| (m.date, f, m.loss_credit_used)));
    }
    cc.sort_by_key(|&(date, f, _)| (date, f));
    let mut notes: Vec<CreditNote> = Vec::new();
    let mut next_cc = 0;
    let mut note_up_to = |limit: Option<NaiveDate>, pools: &mut BTreeMap<String, Pool>, funds: &[CompFund<'_>]| {
        while next_cc < cc.len() && limit.map_or(true, |l| cc[next_cc].0 <= l) {
            let (date, f, used) = cc[next_cc];
            let pool = pools.get_mut(&keys[f]).expect("o grupo existe");
            let balance = pool.amount;
            match used {
                None if balance > NOTE_MIN_BALANCE => {
                    notes.push(CreditNote { asset_id: funds[f].asset_id, date, balance, informed: None });
                }
                None => {}
                Some(used) => {
                    if used > balance + NOTE_MIN_BALANCE {
                        notes.push(CreditNote { asset_id: funds[f].asset_id, date, balance, informed: Some(used) });
                    }
                    pool.amount = (balance - used).max(0.0);
                }
            }
            next_cc += 1;
        }
    };

    let mut start = 0;
    while start < order.len() {
        let date = order[start].0;
        let end = start + order[start..].iter().take_while(|o| o.0 == date).count();
        note_up_to(Some(date), &mut pools, funds);
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
    note_up_to(None, &mut pools, funds);

    let credits = pools
        .into_values()
        .filter(|p| p.amount > EPS)
        .map(|p| LossCredit {
            administrator: p.administrator,
            term: p.term,
            tickers: p.tickers.into_iter().collect(),
            asset_ids: p.asset_ids,
            amount: p.amount,
        })
        .collect();
    (credits, notes)
}

/// Um fundo do Workspace visto pelo aviso de guarda do registro: com ou sem tipo, porque o art. 15 §2 fala de todos os
/// fundos da mesma instituição.
pub struct RecordFund<'a> {
    pub administrator: Option<&'a str>,
    /// Cotas em aberto.
    pub quantity: f64,
    /// Data do último resgate (venda) do fundo.
    pub last_sell: Option<NaiveDate>,
}

/// Aviso de uma administradora de onde o usuário saiu por completo com saldo de crédito de prejuízo: o registro só tem
/// guarda garantida até o fim do ano seguinte ao resgate total (IN RFB 1.585/2015, art. 15 §2). Não limita o direito de
/// compensar (Solução de Consulta Cosit 38/2016): é só um aviso, nenhum número muda.
#[derive(Clone, Debug, Serialize)]
pub struct RecordNote {
    /// A administradora como o usuário escreveu (a do primeiro fundo dela).
    pub administrator: String,
    /// Data do último resgate entre os fundos dela.
    pub redeemed_on: NaiveDate,
    /// 31/12 do ano seguinte ao do resgate total.
    pub guaranteed_until: NaiveDate,
    /// A garantia de guarda já terminou em `today`.
    pub expired: bool,
    /// Saldo de crédito dos grupos dessa administradora (todos os tipos), em BRL.
    pub balance: f64,
}

/// Um aviso por administradora em que todos os fundos lançados estão zerados, houve resgate e sobrou saldo de crédito.
/// Fundo sem administradora não agrupa e não entra. `credits` é o saldo já compensado (`compensate_with_notes`).
pub fn record_notes(funds: &[RecordFund<'_>], credits: &[LossCredit], today: NaiveDate) -> Vec<RecordNote> {
    // administradora normalizada -> (nome escrito, todos zerados, último resgate)
    let mut by_admin: BTreeMap<String, (String, bool, Option<NaiveDate>)> = BTreeMap::new();
    for fund in funds {
        let Some(key) = normalize_administrator(fund.administrator) else { continue };
        let name = fund.administrator.map(|a| a.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
        let entry = by_admin.entry(key).or_insert((name, true, None));
        entry.1 &= fund.quantity.abs() <= EPS;
        entry.2 = entry.2.max(fund.last_sell);
    }
    by_admin
        .into_iter()
        .filter_map(|(key, (administrator, all_closed, last_sell))| {
            let redeemed_on = last_sell.filter(|_| all_closed)?;
            let balance: f64 = credits
                .iter()
                .filter(|c| normalize_administrator(c.administrator.as_deref()).as_deref() == Some(key.as_str()))
                .map(|c| c.amount)
                .sum();
            if balance <= EPS {
                return None;
            }
            let guaranteed_until = NaiveDate::from_ymd_opt(redeemed_on.year() + 1, 12, 31)?;
            Some(RecordNote { administrator, redeemed_on, guaranteed_until, expired: today > guaranteed_until, balance })
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
        come_cotas: Vec<ComeCotasMark>,
    }

    fn fixture(
        asset_id: i32,
        ticker: &'static str,
        term: FundTerm,
        administrator: Option<&'static str>,
        events: &[FundEvent],
    ) -> Fixture {
        let redemptions = estimate_redemptions(events, term, &test_rule_set()).unwrap();
        Fixture { asset_id, ticker, term, administrator, redemptions, come_cotas: Vec::new() }
    }

    /// O mesmo fundo com descontos de come-cotas registrados nas `dates`.
    fn with_come_cotas(mut fixture: Fixture, dates: &[&str]) -> Fixture {
        fixture.come_cotas = dates.iter().map(|s| ComeCotasMark { date: d(s), loss_credit_used: None }).collect();
        fixture
    }

    /// O mesmo fundo com come-cotas registrados em que o usuário informou o crédito usado: `(data, rendimento coberto)`.
    fn with_credit_used(mut fixture: Fixture, marks: &[(&str, f64)]) -> Fixture {
        fixture.come_cotas =
            marks.iter().map(|(s, used)| ComeCotasMark { date: d(s), loss_credit_used: Some(*used) }).collect();
        fixture
    }

    fn run_with_notes(fixtures: &mut [Fixture]) -> (Vec<LossCredit>, Vec<CreditNote>) {
        let mut funds: Vec<CompFund<'_>> = fixtures
            .iter_mut()
            .map(|f| CompFund {
                asset_id: f.asset_id,
                ticker: f.ticker,
                term: f.term,
                administrator: f.administrator,
                redemptions: &mut f.redemptions,
                come_cotas: f.come_cotas.clone(),
            })
            .collect();
        compensate_with_notes(&mut funds)
    }

    fn run(fixtures: &mut [Fixture]) -> Vec<LossCredit> {
        run_with_notes(fixtures).0
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
                    FundEvent { date: d("2025-05-30"), id: 2, kind: FundEventKind::ComeCotas { quantity: 3.0, quota_value: 12.0, loss_credit_used: None } },
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
    fn a_fall_after_a_come_cotas_gives_the_legal_loss_as_credit_to_the_group() {
        // X: 100 cotas a 10,00; come-cotas de 2,5 cotas a 12,00 em 31/03/2025; resgate de 97,5 a 9,00 em 01/06/2025.
        // Perda legal 97,5 × (12 − 9) = 292,50 (pelo custo original seria 92,50). Y ganha 200 (IR de 40 a 20%): o crédito
        // zera o IR de Y, gastando 200, e sobram 292,50 − 200 = 92,50 no grupo
        let come_cotas = FundEvent {
            date: d("2025-03-31"),
            id: 3,
            kind: FundEventKind::ComeCotas { quantity: 2.5, quota_value: 12.0, loss_credit_used: None },
        };
        let mut set = [
            fixture(
                1,
                "XPTO11",
                FundTerm::LongTerm,
                Some("XP"),
                &[buy(1, "2025-01-01", 100.0, 1_000.0), come_cotas, redeem(2, "2025-06-01", 97.5, 9.0)],
            ),
            winner(2, "YPTO11", FundTerm::LongTerm, Some("XP"), "2025-09-01"),
        ];
        let credits = run(&mut set);
        assert!(close(set[0].redemptions[0].loss_created, 292.5), "{:?}", set[0].redemptions[0]);
        let y = &set[1].redemptions[0];
        assert!(close(y.loss_credit_used, 200.0) && close(y.ir, 0.0), "{y:?}");
        assert!(credits.len() == 1 && close(credits[0].amount, 92.5), "{credits:?}");
    }

    #[test]
    fn a_loss_on_an_amortization_is_credit_for_a_later_redemption() {
        // X: 100 cotas a 10,00; amortização de 400 a cota 8,00 em 01/06/2025 (posição 800, fração 0,5, 50 cotas equivalentes):
        // rendimento 50 × (8 − 10) = −100, perda de 100. Y ganha 200 (IR de 40 a 20%): o crédito de 100 abate 100 × 20% = 20
        let amortization = FundEvent {
            date: d("2025-06-01"),
            id: 2,
            kind: FundEventKind::Amortize { tx_id: 2, amount: 400.0, quota_value: 8.0 },
        };
        let mut set = [
            fixture(1, "XPTO11", FundTerm::LongTerm, Some("XP"), &[buy(1, "2025-01-01", 100.0, 1_000.0), amortization]),
            winner(2, "YPTO11", FundTerm::LongTerm, Some("XP"), "2025-09-01"),
        ];
        let credits = run(&mut set);
        let x = &set[0].redemptions[0];
        assert!(x.is_amortization && close(x.loss_created, 100.0) && close(x.ir, 0.0), "{x:?}");
        let y = &set[1].redemptions[0];
        assert!(close(y.loss_credit_used, 100.0) && close(y.ir, 20.0), "{y:?}");
        assert!(credits.is_empty());
    }

    #[test]
    fn an_investment_entity_is_its_own_class_and_does_not_offset_a_long_term_fund() {
        let mut set = [
            loser(1, "FIP11", FundTerm::InvestmentEntity, Some("XP"), 8.0),
            winner(2, "YPTO11", FundTerm::LongTerm, Some("XP"), "2025-09-01"),
        ];
        let credits = run(&mut set);
        assert!(close(set[0].redemptions[0].loss_created, 200.0));
        assert!(close(set[1].redemptions[0].ir, 40.0) && close(set[1].redemptions[0].loss_credit_used, 0.0));
        assert_eq!(credits.len(), 1);
        assert!(credits[0].term == FundTerm::InvestmentEntity && close(credits[0].amount, 200.0));
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

    fn credit(administrator: Option<&str>, term: FundTerm, amount: f64) -> LossCredit {
        LossCredit {
            administrator: administrator.map(str::to_string),
            term,
            tickers: vec!["X".to_string()],
            asset_ids: vec![1],
            amount,
        }
    }

    fn record_fund<'a>(administrator: Option<&'a str>, quantity: f64, last_sell: Option<&str>) -> RecordFund<'a> {
        RecordFund { administrator, quantity, last_sell: last_sell.map(d) }
    }

    #[test]
    fn leaving_an_administrator_with_a_balance_warns_with_the_guarantee_until_the_end_of_the_next_year() {
        // resgate total em 15/11/2025 e saldo de 80 + 20 (longo e curto prazo da mesma administradora): um aviso, guarda
        // até 31/12/2026, ainda não terminada em 22/10/2026
        let funds = [record_fund(Some("XP"), 0.0, Some("2025-06-01")), record_fund(Some(" xp "), 0.0, Some("2025-11-15"))];
        let credits = [credit(Some("XP"), FundTerm::LongTerm, 80.0), credit(Some("XP"), FundTerm::ShortTerm, 20.0)];
        let notes = record_notes(&funds, &credits, d("2026-10-22"));
        assert_eq!(notes.len(), 1, "{notes:?}");
        let n = &notes[0];
        assert_eq!((n.redeemed_on, n.guaranteed_until, n.expired), (d("2025-11-15"), d("2026-12-31"), false));
        assert!(close(n.balance, 100.0) && n.administrator == "XP", "{n:?}");
    }

    #[test]
    fn the_guarantee_ends_after_the_last_day_of_the_next_year() {
        let funds = [record_fund(Some("XP"), 0.0, Some("2025-12-31"))];
        let credits = [credit(Some("XP"), FundTerm::LongTerm, 10.0)];
        // resgate em 31/12/2025: a guarda vai até 31/12/2026 (o ano de 2026 inteiro), e 01/01/2027 já passou
        assert!(!record_notes(&funds, &credits, d("2026-12-31"))[0].expired);
        assert!(record_notes(&funds, &credits, d("2027-01-01"))[0].expired);
    }

    #[test]
    fn no_warning_while_a_fund_of_the_administrator_still_holds_quotas_or_without_a_balance_or_a_redemption() {
        let credits = [credit(Some("XP"), FundTerm::LongTerm, 10.0)];
        let holding_one = [record_fund(Some("XP"), 0.0, Some("2025-06-01")), record_fund(Some("XP"), 5.0, None)];
        assert!(record_notes(&holding_one, &credits, d("2026-10-22")).is_empty());
        let closed = [record_fund(Some("XP"), 0.0, Some("2025-06-01"))];
        assert!(record_notes(&closed, &[], d("2026-10-22")).is_empty(), "sem saldo");
        let never_sold = [record_fund(Some("XP"), 0.0, None)];
        assert!(record_notes(&never_sold, &credits, d("2026-10-22")).is_empty(), "sem resgate");
    }

    #[test]
    fn administrators_are_independent_and_funds_without_one_are_ignored() {
        // XP saiu por completo e tem saldo; Itaú ainda tem cotas; o fundo sem administradora não agrupa
        let funds = [
            record_fund(Some("XP"), 0.0, Some("2025-06-01")),
            record_fund(Some("Itaú"), 3.0, Some("2025-06-01")),
            record_fund(None, 0.0, Some("2025-06-01")),
        ];
        let credits = [
            credit(Some("XP"), FundTerm::LongTerm, 10.0),
            credit(Some("Itaú"), FundTerm::LongTerm, 10.0),
            credit(None, FundTerm::LongTerm, 10.0),
        ];
        let notes = record_notes(&funds, &credits, d("2026-10-22"));
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(notes[0].administrator, "XP");
    }

    fn holding(asset_id: i32, ticker: &str, quantity: f64, quota: f64) -> FundHolding {
        FundHolding {
            asset_id,
            ticker: ticker.to_string(),
            name: ticker.to_string(),
            term: Some(FundTerm::LongTerm),
            quantity,
            average_cost: 10.0,
            taxed_basis_per_quota: None,
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

    #[test]
    fn a_recorded_come_cotas_after_a_loss_gets_a_note_with_the_group_balance() {
        // X perde 200 em 01/06/2025; o come-cotas registrado em 30/11/2025 foi feito com 200 de saldo no grupo
        let mut set = [with_come_cotas(loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0), &["2025-11-30"])];
        let (credits, notes) = run_with_notes(&mut set);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].asset_id == 1 && notes[0].date == d("2025-11-30") && close(notes[0].balance, 200.0), "{notes:?}");
        // o aviso não muda nada: o saldo continua inteiro e o resgate igual
        assert!(close(credits.iter().map(|c| c.amount).sum::<f64>(), 200.0), "{credits:?}");
        assert!(close(set[0].redemptions[0].loss_created, 200.0));
    }

    #[test]
    fn a_come_cotas_before_or_on_the_day_of_the_loss_gets_no_note() {
        // a perda é de 01/06/2025: antes e no mesmo dia não há crédito (só datas estritamente anteriores); no dia seguinte há
        let mut none = [with_come_cotas(
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0),
            &["2025-05-01", "2025-06-01"],
        )];
        assert!(run_with_notes(&mut none).1.is_empty());

        let mut next_day = [with_come_cotas(loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0), &["2025-06-02"])];
        let notes = run_with_notes(&mut next_day).1;
        assert!(notes.len() == 1 && notes[0].date == d("2025-06-02"), "{notes:?}");
    }

    #[test]
    fn a_come_cotas_after_a_redemption_spent_the_whole_credit_gets_no_note() {
        // X perde 200 (01/06/2025) e Y gasta os 200 num ganho de 01/09/2025. O come-cotas de X em 01/07/2025 ainda via 200 de
        // saldo (nota); o de Y em 30/11/2025 já não vê nada (sem nota)
        let mut set = [
            with_come_cotas(loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0), &["2025-07-01"]),
            with_come_cotas(winner(2, "YPTO11", FundTerm::LongTerm, Some("XP"), "2025-09-01"), &["2025-11-30"]),
        ];
        let (credits, notes) = run_with_notes(&mut set);
        assert!(credits.is_empty(), "{credits:?}");
        assert!(notes.len() == 1 && notes[0].asset_id == 1 && close(notes[0].balance, 200.0), "{notes:?}");
    }

    #[test]
    fn the_note_shows_the_balance_left_after_a_partial_use() {
        // X perde 200; Y ganha só 100 (11,00 contra 10,00 em 243 dias: IR 20, usa 100 de crédito); sobram 100 para o
        // come-cotas de 30/11/2025
        let small_winner = fixture(
            2,
            "YPTO11",
            FundTerm::LongTerm,
            Some("XP"),
            &[buy(1, "2025-01-01", 100.0, 1_000.0), redeem(2, "2025-09-01", 100.0, 11.0)],
        );
        let mut set = [
            with_come_cotas(loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0), &["2025-11-30"]),
            small_winner,
        ];
        let (credits, notes) = run_with_notes(&mut set);
        assert!(close(set[1].redemptions[0].loss_credit_used, 100.0), "{:?}", set[1].redemptions[0]);
        assert!(close(credits[0].amount, 100.0), "{credits:?}");
        assert!(notes.len() == 1 && close(notes[0].balance, 100.0), "{notes:?}");
    }

    #[test]
    fn other_groups_and_funds_without_an_administrator_get_no_note_from_a_loss_that_is_not_theirs() {
        // X (XP, longo prazo) perde 200; os come-cotas de outra administradora, de outro tipo e de um fundo sem
        // administradora não são do grupo dele
        let mut set = [
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0),
            with_come_cotas(winner(2, "BBB11", FundTerm::LongTerm, Some("BTG"), "2025-09-01"), &["2025-11-30"]),
            with_come_cotas(winner(3, "CCC11", FundTerm::ShortTerm, Some("XP"), "2025-09-01"), &["2025-11-30"]),
            with_come_cotas(winner(4, "DDD11", FundTerm::LongTerm, None, "2025-09-01"), &["2025-11-30"]),
        ];
        assert!(run_with_notes(&mut set).1.is_empty());

        // sem administradora o grupo é o próprio fundo: o come-cotas dele depois da sua perda avisa
        let mut alone = [with_come_cotas(loser(5, "EEE11", FundTerm::LongTerm, None, 8.0), &["2025-11-30"])];
        let notes = run_with_notes(&mut alone).1;
        assert!(notes.len() == 1 && notes[0].asset_id == 5, "{notes:?}");
    }

    #[test]
    fn each_recorded_come_cotas_gets_its_own_note_in_date_order() {
        let mut set = [with_come_cotas(
            loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0),
            &["2025-12-01", "2025-11-30"],
        )];
        let notes = run_with_notes(&mut set).1;
        let dates: Vec<NaiveDate> = notes.iter().map(|n| n.date).collect();
        assert_eq!(dates, vec![d("2025-11-30"), d("2025-12-01")]);
    }

    #[test]
    fn a_credit_informed_on_a_come_cotas_comes_off_the_group_balance_and_the_later_redemption_gets_less() {
        // X perde 200 (01/06/2025). No come-cotas de 01/07/2025 o usuário informou 150 de rendimento coberto: o saldo cai
        // para 50, sem aviso. O ganho de Y de 01/09/2025 (200 a 20% = IR 40) só recebe os 50 restantes: (200 − 50) × 20% = 30
        let mut set = [
            with_credit_used(loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0), &[("2025-07-01", 150.0)]),
            winner(2, "YPTO11", FundTerm::LongTerm, Some("XP"), "2025-09-01"),
        ];
        let (credits, notes) = run_with_notes(&mut set);
        assert!(notes.is_empty(), "{notes:?}");
        let y = &set[1].redemptions[0];
        assert!(close(y.loss_credit_used, 50.0) && close(y.ir, 30.0), "{y:?}");
        assert!(credits.is_empty(), "{credits:?}");
    }

    #[test]
    fn a_zero_informed_on_a_come_cotas_means_no_credit_was_used_and_gives_no_note() {
        // 0 é uma resposta ("nenhum crédito usado"): sem aviso e o saldo de 200 fica inteiro para o ganho de Y (IR zero)
        let mut set = [
            with_credit_used(loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0), &[("2025-07-01", 0.0)]),
            winner(2, "YPTO11", FundTerm::LongTerm, Some("XP"), "2025-09-01"),
        ];
        let (credits, notes) = run_with_notes(&mut set);
        assert!(notes.is_empty(), "{notes:?}");
        assert!(close(set[1].redemptions[0].ir, 0.0) && close(set[1].redemptions[0].loss_credit_used, 200.0));
        assert!(credits.is_empty());
    }

    #[test]
    fn a_credit_informed_above_the_balance_takes_the_balance_and_gets_a_note() {
        // saldo de 200 e o usuário informou 300: o saldo zera (não fica negativo), Y não recebe crédito (IR 40) e há aviso
        // com o saldo de 200 e o valor informado
        let mut set = [
            with_credit_used(loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0), &[("2025-07-01", 300.0)]),
            winner(2, "YPTO11", FundTerm::LongTerm, Some("XP"), "2025-09-01"),
        ];
        let (credits, notes) = run_with_notes(&mut set);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(close(notes[0].balance, 200.0) && notes[0].informed == Some(300.0), "{notes:?}");
        assert!(close(set[1].redemptions[0].ir, 40.0) && close(set[1].redemptions[0].loss_credit_used, 0.0));
        assert!(credits.is_empty(), "{credits:?}");

        // sem nenhum saldo (o come-cotas é de antes da perda), qualquer valor informado acima de zero também avisa
        let mut early =
            [with_credit_used(loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0), &[("2025-05-01", 100.0)])];
        let notes = run_with_notes(&mut early).1;
        assert!(notes.len() == 1 && close(notes[0].balance, 0.0) && notes[0].informed == Some(100.0), "{notes:?}");
    }

    #[test]
    fn a_come_cotas_without_a_credit_informed_still_gets_the_original_note() {
        let mut set = [with_come_cotas(loser(1, "XPTO11", FundTerm::LongTerm, Some("XP"), 8.0), &["2025-11-30"])];
        let notes = run_with_notes(&mut set).1;
        assert!(notes.len() == 1 && notes[0].informed.is_none() && close(notes[0].balance, 200.0), "{notes:?}");
    }
}
