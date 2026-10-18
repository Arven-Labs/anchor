// Fase 35, entrega 10 — estimativa do rendimento de cada resgate do porquinho (CDB de liquidez diária
// ≈ 100% do CDI) a partir só do que o extrato mostra: as aplicações e o que o banco pagou em cada resgate.
//
// O CDI diário não está no app (só a Selic meta diária), então vale `meta − 0,10 p.p.`, que é a relação
// histórica entre as duas. O título rende nos dias úteis, 252 ao ano: o fator de um dia é
// `1 + ((1 + cdi/100)^(1/252) − 1) × pct`. Nos 23 resgates de 2025 que a Movimentação da B3 explica, a 100% do CDI
// o erro mediano foi de 0,4% por resgate (98% e 102% pioram para 2,2% e 2,4%).
//
// Cada resgate consome as aplicações mais antigas primeiro (PEPS). O valor pago `X` é líquido de IOF e IR, e
// cada lote rende `k = 1 + g × (1 − iof) × (1 − ir)` por real de principal (g = fator − 1), então o principal
// de cada lote sai de `X ÷ k` e o resto da conta é direta. IOF pela tabela do decreto (`domain::tax::iof`) e IR pela
// regra do papel, sobre o que sobra do IOF, como em `commands::fixed_income_io`.
use std::collections::VecDeque;

use chrono::{Duration, NaiveDate};

use crate::domain::calendar::is_business_day;
use crate::domain::tax::iof::iof_rate_for_days;
use crate::error::AppError;

/// Diferença histórica entre a Selic meta e o CDI, em pontos percentuais ao ano.
pub const CDI_BELOW_SELIC_META_PP: f64 = 0.10;
/// Dias úteis do ano-base do CDI.
const BUSINESS_DAYS_PER_YEAR: f64 = 252.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LegKind {
    /// Dinheiro que entrou no porquinho: vira um lote.
    Application,
    /// Dinheiro que saiu do porquinho: o que o banco pagou, líquido de IOF e IR.
    Redemption,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    pub date: NaiveDate,
    pub kind: LegKind,
    pub amount: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RedemptionEstimate {
    /// Posição do resgate na lista de fluxos recebida.
    pub leg_index: usize,
    pub date: NaiveDate,
    /// O que o banco pagou (`principal + net_yield`).
    pub paid: f64,
    pub principal: f64,
    pub gross_yield: f64,
    pub iof: f64,
    pub ir: f64,
    pub net_yield: f64,
    /// Dias aplicados, ponderados pelo principal de cada lote (só da parte com aplicação conhecida).
    pub holding_days: Option<u32>,
    /// Toda a quantia paga tem aplicação nos fluxos recebidos; falso quando o resgate passa do que se conhece.
    pub fully_covered: bool,
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Meta Selic em vigor no dia (a última ≤ dia); `None` antes do primeiro ponto. `selic` vem ordenada por data.
pub fn meta_on(selic: &[(NaiveDate, f64)], day: NaiveDate) -> Option<f64> {
    let after = selic.partition_point(|(date, _)| *date <= day);
    after.checked_sub(1).map(|i| selic[i].1)
}

fn daily_cdi(meta_pct: f64) -> f64 {
    (1.0 + (meta_pct - CDI_BELOW_SELIC_META_PP) / 100.0).powf(1.0 / BUSINESS_DAYS_PER_YEAR) - 1.0
}

/// Fator de rendimento de `from` (exclusive) até `to` (inclusive), nos dias úteis, a `pct_of_cdi` do CDI
/// (1,0 = 100%). `None` quando falta a meta Selic em algum dia do trecho.
pub fn accrual_factor(selic: &[(NaiveDate, f64)], from: NaiveDate, to: NaiveDate, pct_of_cdi: f64) -> Option<f64> {
    let mut factor = 1.0;
    let mut day = from + Duration::days(1);
    while day <= to {
        if is_business_day(day) {
            factor *= 1.0 + daily_cdi(meta_on(selic, day)?) * pct_of_cdi;
        }
        day += Duration::days(1);
    }
    Some(factor)
}

/// Estima o rendimento de cada resgate. `legs` vem em ordem cronológica (no mesmo dia, as aplicações antes dos resgates
/// ficam a cargo de quem monta a lista). `ir_rate_at` dá a alíquota do IR na data do resgate.
pub fn estimate_redemptions(
    legs: &[Leg],
    selic: &[(NaiveDate, f64)],
    pct_of_cdi: f64,
    ir_rate_at: impl Fn(NaiveDate) -> Result<f64, AppError>,
) -> Result<Vec<RedemptionEstimate>, AppError> {
    let mut lots: VecDeque<(NaiveDate, f64)> = VecDeque::new();
    let mut out = Vec::new();
    for (leg_index, leg) in legs.iter().enumerate() {
        match leg.kind {
            LegKind::Application => lots.push_back((leg.date, leg.amount)),
            LegKind::Redemption => {
                let ir_rate = ir_rate_at(leg.date)?;
                let mut remaining_paid = leg.amount;
                let (mut gross, mut iof, mut ir) = (0.0, 0.0, 0.0);
                let mut weighted_days = 0.0;
                let mut known_principal = 0.0;
                for lot in lots.iter_mut() {
                    if remaining_paid <= 1e-9 {
                        break;
                    }
                    if lot.1 <= 1e-9 || lot.0 > leg.date {
                        continue;
                    }
                    let factor = accrual_factor(selic, lot.0, leg.date, pct_of_cdi).ok_or_else(|| {
                        AppError::InvalidInput(format!(
                            "the Selic history does not cover {} to {}: fetch the rates and try again",
                            lot.0, leg.date
                        ))
                    })?;
                    let days = (leg.date - lot.0).num_days();
                    let iof_rate = iof_rate_for_days(days);
                    let gain = factor - 1.0;
                    let net_multiplier = 1.0 + gain * (1.0 - iof_rate) * (1.0 - ir_rate);
                    let take = lot.1.min(remaining_paid / net_multiplier);
                    lot.1 -= take;
                    remaining_paid -= take * net_multiplier;
                    known_principal += take;
                    gross += take * gain;
                    iof += take * gain * iof_rate;
                    ir += take * gain * (1.0 - iof_rate) * ir_rate;
                    weighted_days += days as f64 * take;
                }
                let fully_covered = remaining_paid <= 0.005;
                let (gross, iof, ir) = (round2(gross), round2(iof), round2(ir));
                let net_yield = round2(gross - iof - ir);
                // O que passou dos lotes conhecidos é principal sem aplicação nos fluxos: sem rendimento estimado,
                // e `principal` é sempre o pago menos o rendimento líquido, para a soma fechar no centavo.
                out.push(RedemptionEstimate {
                    leg_index,
                    date: leg.date,
                    paid: leg.amount,
                    principal: round2(leg.amount - net_yield),
                    gross_yield: gross,
                    iof,
                    ir,
                    net_yield,
                    holding_days: (known_principal > 1e-9).then(|| (weighted_days / known_principal).round().max(0.0) as u32),
                    fully_covered,
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// Meta de 10,10% (CDI de 10,00% ao ano) desde antes de tudo.
    fn flat_selic() -> Vec<(NaiveDate, f64)> {
        vec![(d(2024, 1, 1), 10.10)]
    }

    fn legs(items: &[(NaiveDate, LegKind, f64)]) -> Vec<Leg> {
        items.iter().map(|(date, kind, amount)| Leg { date: *date, kind: *kind, amount: *amount }).collect()
    }

    fn ir_22_5(_: NaiveDate) -> Result<f64, AppError> {
        Ok(0.225)
    }

    #[test]
    fn the_meta_in_force_is_the_last_one_on_or_before_the_day() {
        let selic = vec![(d(2025, 1, 1), 12.25), (d(2025, 3, 20), 14.25)];
        assert_eq!(meta_on(&selic, d(2024, 12, 31)), None);
        assert_eq!(meta_on(&selic, d(2025, 3, 19)), Some(12.25));
        assert_eq!(meta_on(&selic, d(2025, 3, 20)), Some(14.25));
    }

    #[test]
    fn one_business_day_accrues_the_daily_cdi_and_pct_scales_it() {
        // Quarta 11/6/2025 → quinta 12/6: um dia útil. CDI de 10% ao ano.
        let one = accrual_factor(&flat_selic(), d(2025, 6, 11), d(2025, 6, 12), 1.0).unwrap();
        let daily = 1.10_f64.powf(1.0 / 252.0) - 1.0;
        assert!((one - (1.0 + daily)).abs() < 1e-12);
        let half = accrual_factor(&flat_selic(), d(2025, 6, 11), d(2025, 6, 12), 0.5).unwrap();
        assert!((half - (1.0 + daily * 0.5)).abs() < 1e-12);
    }

    #[test]
    fn weekends_and_holidays_do_not_accrue() {
        // Sexta 13/6 → segunda 16/6: só o dia 16 rende.
        let weekend = accrual_factor(&flat_selic(), d(2025, 6, 13), d(2025, 6, 16), 1.0).unwrap();
        let one_day = accrual_factor(&flat_selic(), d(2025, 6, 15), d(2025, 6, 16), 1.0).unwrap();
        assert!((weekend - one_day).abs() < 1e-12);
        // Corpus Christi (quinta 19/6/2025) não rende: quarta 18 → sexta 20 é um dia só.
        let holiday = accrual_factor(&flat_selic(), d(2025, 6, 18), d(2025, 6, 20), 1.0).unwrap();
        assert!((holiday - one_day).abs() < 1e-12);
    }

    #[test]
    fn a_meta_change_in_the_middle_is_picked_up_day_by_day() {
        let selic = vec![(d(2024, 1, 1), 10.10), (d(2025, 6, 17), 20.10)];
        let f = accrual_factor(&selic, d(2025, 6, 13), d(2025, 6, 18), 1.0).unwrap();
        // 16/6 a 10% ao ano, 17/6 e 18/6 a 20%.
        let low = 1.10_f64.powf(1.0 / 252.0);
        let high = 1.20_f64.powf(1.0 / 252.0);
        assert!((f - low * high * high).abs() < 1e-12);
    }

    #[test]
    fn missing_meta_before_the_series_is_none() {
        assert_eq!(accrual_factor(&[(d(2025, 6, 17), 10.10)], d(2025, 6, 11), d(2025, 6, 12), 1.0), None);
    }

    #[test]
    fn what_was_paid_equals_principal_plus_net_yield_to_the_cent() {
        let flows = legs(&[
            (d(2025, 3, 3), LegKind::Application, 1000.0),
            (d(2025, 6, 3), LegKind::Redemption, 250.37),
        ]);
        let est = estimate_redemptions(&flows, &flat_selic(), 1.0, ir_22_5).unwrap();
        assert_eq!(est.len(), 1);
        let e = &est[0];
        assert!(e.fully_covered);
        assert!((e.principal + e.net_yield - 250.37).abs() < 0.0051, "principal {} + líquido {}", e.principal, e.net_yield);
        assert!(e.gross_yield > 0.0 && e.net_yield > 0.0 && e.net_yield < e.gross_yield);
        // Acima de 30 dias não há IOF; o IR é 22,5% do rendimento bruto.
        assert_eq!(e.iof, 0.0);
        assert!((e.ir - round2(e.gross_yield * 0.225)).abs() <= 0.011);
    }

    #[test]
    fn a_redemption_within_thirty_days_pays_iof_before_the_ir() {
        let flows = legs(&[
            (d(2025, 6, 2), LegKind::Application, 10_000.0),
            (d(2025, 6, 9), LegKind::Redemption, 10_000.0),
        ]);
        let e = &estimate_redemptions(&flows, &flat_selic(), 1.0, ir_22_5).unwrap()[0];
        // 7 dias aplicados: IOF de 76% do rendimento; o IR incide sobre os 24% que sobram.
        assert_eq!(e.holding_days, Some(7));
        assert!((e.iof - round2(e.gross_yield * 0.76)).abs() <= 0.011, "iof {} de {}", e.iof, e.gross_yield);
        assert!((e.ir - round2((e.gross_yield - e.iof) * 0.225)).abs() <= 0.011);
    }

    #[test]
    fn redemptions_consume_the_oldest_application_first() {
        // Dois lotes; o resgate pequeno sai todo do mais antigo (maior tempo aplicado), então rende mais por real.
        let flows = legs(&[
            (d(2025, 1, 6), LegKind::Application, 500.0),
            (d(2025, 5, 5), LegKind::Application, 500.0),
            (d(2025, 6, 2), LegKind::Redemption, 100.0),
            (d(2025, 6, 2), LegKind::Redemption, 100.0),
        ]);
        let est = estimate_redemptions(&flows, &flat_selic(), 1.0, ir_22_5).unwrap();
        // Os dois saem do lote de janeiro (500 cobre 200): mesmo rendimento nos dois, e o dia aplicado é o do lote antigo.
        assert_eq!(est[0].holding_days, est[1].holding_days);
        assert_eq!(est[0].holding_days, Some((d(2025, 6, 2) - d(2025, 1, 6)).num_days() as u32));
        assert_eq!(est[0].gross_yield, est[1].gross_yield);
    }

    #[test]
    fn a_redemption_larger_than_the_known_applications_is_flagged_and_estimates_only_the_known_part() {
        let flows = legs(&[
            (d(2025, 3, 3), LegKind::Application, 100.0),
            (d(2025, 6, 3), LegKind::Redemption, 400.0),
        ]);
        let e = &estimate_redemptions(&flows, &flat_selic(), 1.0, ir_22_5).unwrap()[0];
        assert!(!e.fully_covered);
        // O rendimento é só o dos 100 aplicados (cerca de 2,4% em três meses a 10% ao ano, menos IR).
        assert!(e.gross_yield > 1.5 && e.gross_yield < 3.5, "bruto {}", e.gross_yield);
        assert!((e.principal + e.net_yield - 400.0).abs() < 0.0051);
    }

    #[test]
    fn a_redemption_the_same_day_as_the_application_has_no_yield() {
        let flows = legs(&[(d(2025, 6, 2), LegKind::Application, 50.0), (d(2025, 6, 2), LegKind::Redemption, 50.0)]);
        let e = &estimate_redemptions(&flows, &flat_selic(), 1.0, ir_22_5).unwrap()[0];
        assert_eq!((e.gross_yield, e.net_yield), (0.0, 0.0));
        assert!(e.fully_covered);
    }

    #[test]
    fn a_selic_history_that_starts_after_the_application_is_an_error() {
        let flows = legs(&[(d(2025, 3, 3), LegKind::Application, 100.0), (d(2025, 6, 3), LegKind::Redemption, 101.0)]);
        let late = vec![(d(2025, 5, 1), 10.10)];
        assert!(estimate_redemptions(&flows, &late, 1.0, ir_22_5).is_err());
    }

    #[test]
    fn a_missing_ir_rule_is_an_error_not_a_zero_rate() {
        let flows = legs(&[(d(2025, 3, 3), LegKind::Application, 100.0), (d(2025, 6, 3), LegKind::Redemption, 101.0)]);
        let missing = |_: NaiveDate| Err(AppError::NotFound("tax rule".to_string()));
        assert!(estimate_redemptions(&flows, &flat_selic(), 1.0, missing).is_err());
    }
}
