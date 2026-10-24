// Come-cotas dos fundos abertos: IR cobrado do cotista em cotas, na última data útil de maio e de novembro, sobre o ganho
// desde a compra ou desde o último come-cotas. Aqui só a ESTIMATIVA do próximo desconto; o desconto real e o IR no
// resgate (tabela regressiva, com o crédito do come-cotas) ficam para a fatia seguinte.
//
// Regras (as alíquotas vêm de `tax_rules`, o resto é código):
// - O usuário diz o tipo de cada fundo: longo prazo (15%), curto prazo (20%) ou ações (sem come-cotas).
// - Base por cota = o maior entre o custo médio da cota e o valor da cota na última data de come-cotas que já passou (se
//   a posição já existia e há valor lançado perto dessa data); ganho por cota = max(0, valor atual − base).
// - Imposto = ganho × cotas × alíquota; cotas descontadas = imposto ÷ valor da cota.
// - A estimativa usa o último valor de cota lançado (não projeta até a data) e a posição de hoje.
//
// ATENÇÃO: as regras vêm de fontes secundárias; conferir na Receita antes de confiar.

use chrono::{Datelike, Duration, NaiveDate};
use serde::Serialize;

use super::calendar::last_business_day;
use super::rules::{self, RuleSet};
use crate::domain::cost_basis::EPS;
use crate::error::AppError;

/// Valor de cota com mais dias que isto avisa que a estimativa pode estar velha.
pub const STALE_QUOTA_DAYS: i64 = 35;
/// O valor da cota da última data de come-cotas só serve de base se foi lançado até tantos dias antes dela.
const BASIS_QUOTA_WINDOW_DAYS: i64 = 45;
/// Um desconto registrado conta como o da última data de come-cotas se caiu entre tantos dias antes e depois dela.
const DEDUCTION_DAYS_BEFORE: i64 = 5;
const DEDUCTION_DAYS_AFTER: i64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FundTerm {
    LongTerm,
    ShortTerm,
    Equity,
    /// Entidade de investimento (FIP, FIDC, ETF que não é de renda fixa; Lei 14.754, art. 18): 15% fixo na distribuição,
    /// amortização ou resgate e sem come-cotas (art. 24).
    InvestmentEntity,
}

impl FundTerm {
    /// Sem come-cotas: a estimativa fica isenta e o resgate paga uma alíquota fixa, sem IOF.
    pub fn is_flat_rate(self) -> bool {
        matches!(self, Self::Equity | Self::InvestmentEntity)
    }
}

impl FundTerm {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "longo_prazo" => Some(Self::LongTerm),
            "curto_prazo" => Some(Self::ShortTerm),
            "acoes" => Some(Self::Equity),
            "entidade_investimento" => Some(Self::InvestmentEntity),
            _ => None,
        }
    }
}

/// Valor de uma cota numa data, em BRL.
#[derive(Clone, Copy, Debug)]
pub struct QuotaValue {
    pub date: NaiveDate,
    pub value: f64,
}

/// A posição de um fundo, no formato da estimativa.
#[derive(Clone, Debug)]
pub struct FundHolding {
    pub asset_id: i32,
    pub ticker: String,
    pub name: String,
    pub term: Option<FundTerm>,
    pub quantity: f64,
    /// Custo médio por cota, em BRL.
    pub average_cost: f64,
    /// Data da primeira compra (para saber se a posição já existia no último come-cotas).
    pub first_buy: Option<NaiveDate>,
    /// Valores de cota lançados, em qualquer ordem.
    pub quotas: Vec<QuotaValue>,
    /// Descontos de come-cotas já registrados (transação `come_cotas`): data e valor da cota de cada um.
    pub deductions: Vec<QuotaValue>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EstimateStatus {
    Estimated,
    /// Fundo de ações: sem come-cotas.
    Exempt,
    /// O usuário ainda não disse o tipo do fundo.
    NoTerm,
    /// Nenhum valor de cota lançado (até hoje).
    NoQuota,
    NoPosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BasisSource {
    /// Custo médio da cota.
    Cost,
    /// Valor da cota no último come-cotas (maior que o custo).
    LastComeCotas,
}

#[derive(Clone, Debug, Serialize)]
pub struct ComeCotasEstimate {
    pub asset_id: i32,
    pub ticker: String,
    pub name: String,
    pub status: EstimateStatus,
    /// Próxima data de come-cotas (última data útil de maio ou de novembro).
    pub reference_date: NaiveDate,
    pub quotas: f64,
    pub quota_value: Option<f64>,
    pub quota_date: Option<NaiveDate>,
    pub basis_per_quota: Option<f64>,
    pub basis_source: Option<BasisSource>,
    /// Ganho tributável total, em BRL.
    pub gain: f64,
    pub rate: f64,
    pub tax: f64,
    pub quotas_deducted: f64,
    /// O último valor de cota lançado é antigo (`STALE_QUOTA_DAYS`).
    pub stale_quota: bool,
    /// A última data de come-cotas passou com a posição já existente e sem nenhum desconto registrado em volta dela:
    /// o usuário provavelmente ainda não lançou a transação `come_cotas`.
    pub last_reference_unrecorded: Option<NaiveDate>,
    /// Crédito de prejuízo de resgates anteriores que abateu o ganho desta estimativa (`fund_compensation`); zero até a
    /// compensação rodar. `gain` e `tax` já vêm com o abatimento.
    pub loss_credit_used: f64,
    /// Imposto antes do crédito de prejuízo (igual a `tax` até a compensação rodar).
    pub tax_before_compensation: f64,
}

/// Datas de come-cotas de um ano: última data útil de maio e de novembro.
fn reference_dates(year: i32) -> [NaiveDate; 2] {
    [last_business_day(year, 5), last_business_day(year, 11)]
}

/// Primeira data de come-cotas a partir de `today` (inclusive).
pub fn next_reference_date(today: NaiveDate) -> NaiveDate {
    let year = today.year();
    (year..=year + 1)
        .flat_map(reference_dates)
        .find(|date| *date >= today)
        .expect("há uma data de come-cotas no ano seguinte")
}

/// Última data de come-cotas antes de `today` (estritamente).
pub fn last_reference_date(today: NaiveDate) -> NaiveDate {
    let year = today.year();
    (year - 1..=year)
        .flat_map(reference_dates)
        .filter(|date| *date < today)
        .max()
        .expect("há uma data de come-cotas no ano anterior")
}

/// Estima o próximo come-cotas de cada fundo. Fundo sem tipo, sem valor de cota ou sem posição vem sem imposto e com o
/// motivo em `status`.
pub fn estimate(
    holdings: &[FundHolding],
    rules: &RuleSet,
    today: NaiveDate,
) -> Result<Vec<ComeCotasEstimate>, AppError> {
    let reference_date = next_reference_date(today);
    let last_reference = last_reference_date(today);
    let mut estimates = Vec::with_capacity(holdings.len());

    for holding in holdings {
        let mut estimate = ComeCotasEstimate {
            asset_id: holding.asset_id,
            ticker: holding.ticker.clone(),
            name: holding.name.clone(),
            status: EstimateStatus::NoPosition,
            reference_date,
            quotas: holding.quantity.max(0.0),
            quota_value: None,
            quota_date: None,
            basis_per_quota: None,
            basis_source: None,
            gain: 0.0,
            rate: 0.0,
            tax: 0.0,
            quotas_deducted: 0.0,
            stale_quota: false,
            last_reference_unrecorded: None,
            loss_credit_used: 0.0,
            tax_before_compensation: 0.0,
        };
        if holding.quantity <= EPS {
            estimates.push(estimate);
            continue;
        }
        let Some(term) = holding.term else {
            estimate.status = EstimateStatus::NoTerm;
            estimates.push(estimate);
            continue;
        };
        // O último valor lançado até hoje (valor com data futura não conta).
        let Some(latest) = holding.quotas.iter().filter(|q| q.date <= today).max_by_key(|q| q.date) else {
            estimate.status = EstimateStatus::NoQuota;
            estimates.push(estimate);
            continue;
        };
        estimate.quota_value = Some(latest.value);
        estimate.quota_date = Some(latest.date);
        estimate.stale_quota = (today - latest.date).num_days() > STALE_QUOTA_DAYS;

        let rate_key = match term {
            FundTerm::Equity | FundTerm::InvestmentEntity => {
                estimate.status = EstimateStatus::Exempt;
                estimates.push(estimate);
                continue;
            }
            FundTerm::LongTerm => rules::COME_COTAS_RATE_LONG,
            FundTerm::ShortTerm => rules::COME_COTAS_RATE_SHORT,
        };
        let rate = rules.get(rate_key, reference_date)?;

        // Se a posição já existia no último come-cotas, o ganho antes dele já foi tributado: a base sobe para o valor
        // da cota daquela data (quando é maior que o custo). O desconto registrado dá esse valor com exatidão e vale
        // mais que a busca na tabela de cotas (valor lançado perto da data).
        let held_then = holding.first_buy.is_some_and(|date| date <= last_reference);
        let recorded = holding
            .deductions
            .iter()
            .filter(|q| {
                q.date >= last_reference - Duration::days(DEDUCTION_DAYS_BEFORE)
                    && q.date <= last_reference + Duration::days(DEDUCTION_DAYS_AFTER)
            })
            .max_by_key(|q| q.date);
        let table_quota = holding
            .quotas
            .iter()
            .filter(|q| q.date <= last_reference && (last_reference - q.date).num_days() <= BASIS_QUOTA_WINDOW_DAYS)
            .max_by_key(|q| q.date);
        let last_value = recorded.or(table_quota).map(|q| q.value);
        let (basis, basis_source) = match last_value {
            Some(value) if (recorded.is_some() || held_then) && value > holding.average_cost => {
                (value, BasisSource::LastComeCotas)
            }
            _ => (holding.average_cost, BasisSource::Cost),
        };
        if recorded.is_none() && held_then {
            estimate.last_reference_unrecorded = Some(last_reference);
        }

        let gain = (latest.value - basis).max(0.0) * holding.quantity;
        let tax = gain * rate;
        estimate.status = EstimateStatus::Estimated;
        estimate.basis_per_quota = Some(basis);
        estimate.basis_source = Some(basis_source);
        estimate.gain = gain;
        estimate.rate = rate;
        estimate.tax = tax;
        estimate.tax_before_compensation = tax;
        estimate.quotas_deducted = if latest.value > 0.0 { tax / latest.value } else { 0.0 };
        estimates.push(estimate);
    }
    Ok(estimates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax::rules::test_rule_set;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn quota(date: &str, value: f64) -> QuotaValue {
        QuotaValue { date: d(date), value }
    }

    fn fund(term: Option<FundTerm>, quantity: f64, average_cost: f64, quotas: Vec<QuotaValue>) -> FundHolding {
        FundHolding {
            asset_id: 1,
            ticker: "FUNDO".to_string(),
            name: "Fundo Teste".to_string(),
            term,
            quantity,
            average_cost,
            first_buy: Some(d("2025-01-10")),
            quotas,
            deductions: Vec::new(),
        }
    }

    fn with_deductions(mut holding: FundHolding, deductions: Vec<QuotaValue>) -> FundHolding {
        holding.deductions = deductions;
        holding
    }

    fn run(holding: FundHolding, today: &str) -> ComeCotasEstimate {
        estimate(&[holding], &test_rule_set(), d(today)).unwrap().remove(0)
    }

    #[test]
    fn the_reference_dates_are_the_last_business_day_of_may_and_november() {
        assert_eq!(last_business_day(2026, 5), d("2026-05-29"), "31/05/2026 é domingo");
        assert_eq!(last_business_day(2026, 11), d("2026-11-30"));
        assert_eq!(last_business_day(2025, 5), d("2025-05-30"), "31/05/2025 é sábado");
        assert_eq!(last_business_day(2025, 11), d("2025-11-28"), "30/11/2025 é domingo");
    }

    #[test]
    fn the_next_reference_date_includes_today_and_rolls_into_next_year() {
        assert_eq!(next_reference_date(d("2026-01-10")), d("2026-05-29"));
        assert_eq!(next_reference_date(d("2026-05-29")), d("2026-05-29"));
        assert_eq!(next_reference_date(d("2026-05-30")), d("2026-11-30"));
        assert_eq!(next_reference_date(d("2026-12-01")), d("2027-05-31"));
    }

    #[test]
    fn the_last_reference_date_is_strictly_before_today() {
        assert_eq!(last_reference_date(d("2026-10-22")), d("2026-05-29"));
        assert_eq!(last_reference_date(d("2026-01-10")), d("2025-11-28"));
        assert_eq!(last_reference_date(d("2026-05-29")), d("2025-11-28"));
        assert_eq!(last_reference_date(d("2026-05-30")), d("2026-05-29"));
    }

    #[test]
    fn a_long_term_fund_pays_15_percent_of_the_gain_over_the_average_cost() {
        // 100 cotas, custo 10,00, cota a 12,00 (sem come-cotas anterior): ganho 200, imposto 30, 2,5 cotas
        let e = run(fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-01-05", 12.0)]), "2026-01-10");
        assert_eq!(e.status, EstimateStatus::Estimated);
        assert!(close(e.gain, 200.0) && close(e.rate, 0.15) && close(e.tax, 30.0), "{e:?}");
        assert!(close(e.quotas_deducted, 2.5), "{e:?}");
        assert_eq!(e.basis_source, Some(BasisSource::Cost));
        assert_eq!(e.reference_date, d("2026-05-29"));
    }

    #[test]
    fn a_short_term_fund_pays_20_percent() {
        let e = run(fund(Some(FundTerm::ShortTerm), 100.0, 10.0, vec![quota("2026-01-05", 12.0)]), "2026-01-10");
        assert!(close(e.rate, 0.20) && close(e.tax, 40.0), "{e:?}");
    }

    #[test]
    fn the_basis_rises_to_the_quota_value_of_the_last_come_cotas() {
        // última data: 29/05/2026; cota de 28/05 a 11,00 (> custo 10,00); hoje 12,00 → ganho só de 1,00 por cota
        let e = run(
            fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-05-28", 11.0), quota("2026-10-20", 12.0)]),
            "2026-10-22",
        );
        assert_eq!(e.basis_source, Some(BasisSource::LastComeCotas));
        assert!(close(e.basis_per_quota.unwrap(), 11.0), "{e:?}");
        assert!(close(e.gain, 100.0) && close(e.tax, 15.0), "{e:?}");
        assert_eq!(e.reference_date, d("2026-11-30"));
    }

    #[test]
    fn a_position_bought_after_the_last_come_cotas_keeps_the_cost_as_the_basis() {
        let mut holding = fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-05-28", 11.0), quota("2026-10-20", 12.0)]);
        holding.first_buy = Some(d("2026-07-01"));
        let e = run(holding, "2026-10-22");
        assert_eq!(e.basis_source, Some(BasisSource::Cost));
        assert!(close(e.gain, 200.0), "{e:?}");
    }

    #[test]
    fn an_old_quota_value_is_not_used_as_the_basis() {
        // o único valor antes do último come-cotas é de mais de 45 dias antes dele
        let e = run(
            fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-03-01", 11.0), quota("2026-10-20", 12.0)]),
            "2026-10-22",
        );
        assert_eq!(e.basis_source, Some(BasisSource::Cost));
    }

    #[test]
    fn a_quota_below_the_basis_pays_nothing() {
        let e = run(fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-01-05", 9.0)]), "2026-01-10");
        assert_eq!(e.status, EstimateStatus::Estimated);
        assert!(close(e.gain, 0.0) && close(e.tax, 0.0) && close(e.quotas_deducted, 0.0), "{e:?}");
        assert!(e.gain.is_sign_positive() && e.tax.is_sign_positive());
    }

    #[test]
    fn an_equity_fund_is_exempt() {
        let e = run(fund(Some(FundTerm::Equity), 100.0, 10.0, vec![quota("2026-01-05", 12.0)]), "2026-01-10");
        assert_eq!(e.status, EstimateStatus::Exempt);
        assert!(close(e.tax, 0.0));
    }

    #[test]
    fn a_fund_without_a_term_a_quota_or_a_position_has_no_estimate() {
        assert_eq!(run(fund(None, 100.0, 10.0, vec![quota("2026-01-05", 12.0)]), "2026-01-10").status, EstimateStatus::NoTerm);
        assert_eq!(run(fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![]), "2026-01-10").status, EstimateStatus::NoQuota);
        // valor de cota com data futura não conta
        assert_eq!(
            run(fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-02-01", 12.0)]), "2026-01-10").status,
            EstimateStatus::NoQuota
        );
        assert_eq!(
            run(fund(Some(FundTerm::LongTerm), 0.0, 10.0, vec![quota("2026-01-05", 12.0)]), "2026-01-10").status,
            EstimateStatus::NoPosition
        );
    }

    #[test]
    fn a_quota_value_older_than_35_days_is_flagged() {
        let old = run(fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2025-12-01", 12.0)]), "2026-01-10");
        assert!(old.stale_quota);
        let recent = run(fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-01-05", 12.0)]), "2026-01-10");
        assert!(!recent.stale_quota);
    }

    #[test]
    fn a_recorded_deduction_gives_the_exact_quota_value_of_the_last_come_cotas() {
        // sem valor lançado perto de 29/05, mas com o desconto registrado a 11,00 (> custo 10,00)
        let holding = with_deductions(
            fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-10-20", 12.0)]),
            vec![quota("2026-05-29", 11.0)],
        );
        let e = run(holding, "2026-10-22");
        assert_eq!(e.basis_source, Some(BasisSource::LastComeCotas));
        assert!(close(e.basis_per_quota.unwrap(), 11.0) && close(e.gain, 100.0), "{e:?}");
        assert_eq!(e.last_reference_unrecorded, None, "o desconto está registrado");
    }

    #[test]
    fn a_recorded_deduction_wins_over_the_quota_table() {
        // a tabela diz 10,50 perto da data, o desconto registrado diz 11,00
        let holding = with_deductions(
            fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-05-28", 10.5), quota("2026-10-20", 12.0)]),
            vec![quota("2026-05-29", 11.0)],
        );
        let e = run(holding, "2026-10-22");
        assert!(close(e.basis_per_quota.unwrap(), 11.0), "{e:?}");
    }

    #[test]
    fn a_come_cotas_date_that_passed_without_a_record_is_flagged() {
        let holding = fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-10-20", 12.0)]);
        let e = run(holding, "2026-10-22");
        assert_eq!(e.last_reference_unrecorded, Some(d("2026-05-29")));
        // um desconto registrado 30 dias depois da data não é o dela
        let late = with_deductions(
            fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-10-20", 12.0)]),
            vec![quota("2026-06-28", 11.0)],
        );
        assert_eq!(run(late, "2026-10-22").last_reference_unrecorded, Some(d("2026-05-29")));
    }

    #[test]
    fn no_flag_for_an_equity_fund_a_later_purchase_or_no_position() {
        let equity = fund(Some(FundTerm::Equity), 100.0, 10.0, vec![quota("2026-10-20", 12.0)]);
        assert_eq!(run(equity, "2026-10-22").last_reference_unrecorded, None);

        let mut later = fund(Some(FundTerm::LongTerm), 100.0, 10.0, vec![quota("2026-10-20", 12.0)]);
        later.first_buy = Some(d("2026-07-01"));
        assert_eq!(run(later, "2026-10-22").last_reference_unrecorded, None);

        let empty = fund(Some(FundTerm::LongTerm), 0.0, 10.0, vec![quota("2026-10-20", 12.0)]);
        assert_eq!(run(empty, "2026-10-22").last_reference_unrecorded, None);
    }

    #[test]
    fn the_term_is_parsed_from_the_stored_text() {
        assert_eq!(FundTerm::parse("longo_prazo"), Some(FundTerm::LongTerm));
        assert_eq!(FundTerm::parse("curto_prazo"), Some(FundTerm::ShortTerm));
        assert_eq!(FundTerm::parse("acoes"), Some(FundTerm::Equity));
        assert_eq!(FundTerm::parse("entidade_investimento"), Some(FundTerm::InvestmentEntity));
        assert_eq!(FundTerm::parse("outro"), None);
    }

    #[test]
    fn an_investment_entity_has_no_come_cotas() {
        // FIP, FIDC e ETF que não é de renda fixa: 15% fixo na distribuição, sem come-cotas (Lei 14.754, art. 24)
        let e = run(
            fund(Some(FundTerm::InvestmentEntity), 100.0, 10.0, vec![quota("2026-01-05", 12.0)]),
            "2026-01-10",
        );
        assert_eq!(e.status, EstimateStatus::Exempt);
        assert!(close(e.tax, 0.0));
        assert!(FundTerm::InvestmentEntity.is_flat_rate() && FundTerm::Equity.is_flat_rate());
        assert!(!FundTerm::LongTerm.is_flat_rate() && !FundTerm::ShortTerm.is_flat_rate());
    }
}
