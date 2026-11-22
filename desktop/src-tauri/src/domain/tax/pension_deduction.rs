// Previdência privada: limite de dedução das contribuições ao PGBL na declaração (Lei 9.532/1997, art. 11, redação da Lei
// 10.887/2004). É **informativo**: o dono digita a base e os aportes ao PGBL do ano, e nada aqui entra na apuração de IR, no
// patrimônio nem na Rentabilidade.
//
// Fonte (texto compilado do Planalto, lido em 2026-11-22, Sessão 404): limite de 12% "do total dos rendimentos computados
// na determinação da base de cálculo do imposto devido na declaração"; a dedução depende de o contribuinte também recolher
// ao regime geral (ou próprio). O percentual vem de `tax_rules` (editável).
//
// NÃO confirmado: o que exatamente compõe essa base (a Receita fala em "rendimentos tributáveis"). Por isso a base é um
// número informado por quem usa, e o app não afirma o que a compõe.

use chrono::NaiveDate;
use serde::Serialize;

use super::rules::RuleSet;
use crate::error::AppError;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DeductionLimitEstimate {
    pub rate: f64,
    /// Teto da dedução: `rate × base`.
    pub limit: f64,
    /// Parte dos aportes que cabe no teto.
    pub deductible: f64,
    /// Quanto do teto ainda não foi usado.
    pub remaining: f64,
    /// Aportes acima do teto (não dedutíveis).
    pub excess: f64,
}

/// `base` é o total dos rendimentos da base de cálculo informado por quem usa; `contributions` são os aportes ao PGBL no ano.
pub fn estimate_deduction_limit(
    _rules: &RuleSet,
    _on: NaiveDate,
    _base: f64,
    _contributions: f64,
) -> Result<DeductionLimitEstimate, AppError> {
    Ok(DeductionLimitEstimate { rate: 0.0, limit: 0.0, deductible: 0.0, remaining: 0.0, excess: 0.0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax::rules::{test_rule_set, Rule, PENSION_PGBL_DEDUCTION_LIMIT};

    const EPS: f64 = 1e-9;

    fn on() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 22).unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn contributions_below_the_limit_leave_room() {
        let e = estimate_deduction_limit(&test_rule_set(), on(), 100_000.0, 8_000.0).unwrap();

        assert!(close(e.rate, 0.12));
        assert!(close(e.limit, 12_000.0));
        assert!(close(e.deductible, 8_000.0));
        assert!(close(e.remaining, 4_000.0));
        assert!(close(e.excess, 0.0));
    }

    #[test]
    fn contributions_above_the_limit_have_an_excess() {
        let e = estimate_deduction_limit(&test_rule_set(), on(), 100_000.0, 15_000.0).unwrap();

        assert!(close(e.limit, 12_000.0));
        assert!(close(e.deductible, 12_000.0));
        assert!(close(e.remaining, 0.0));
        assert!(close(e.excess, 3_000.0));
    }

    #[test]
    fn contributions_exactly_at_the_limit_fit() {
        let e = estimate_deduction_limit(&test_rule_set(), on(), 100_000.0, 12_000.0).unwrap();

        assert!(close(e.deductible, 12_000.0));
        assert!(close(e.remaining, 0.0));
        assert!(close(e.excess, 0.0));
    }

    #[test]
    fn no_contributions_leave_the_whole_limit() {
        let e = estimate_deduction_limit(&test_rule_set(), on(), 100_000.0, 0.0).unwrap();

        assert!(close(e.deductible, 0.0));
        assert!(close(e.remaining, 12_000.0));
    }

    #[test]
    fn the_rate_comes_from_the_rule_set_not_from_the_code() {
        let d = NaiveDate::from_ymd_opt(2005, 1, 1).unwrap();
        let rules = RuleSet::new(vec![Rule { key: PENSION_PGBL_DEDUCTION_LIMIT.to_string(), effective_from: d, value: 0.10 }]);
        let e = estimate_deduction_limit(&rules, on(), 100_000.0, 5_000.0).unwrap();

        assert!(close(e.rate, 0.10));
        assert!(close(e.limit, 10_000.0));
    }

    #[test]
    fn a_missing_rule_is_an_error_not_zero() {
        let rules = RuleSet::new(vec![]);

        assert!(matches!(estimate_deduction_limit(&rules, on(), 100_000.0, 1_000.0), Err(AppError::NotFound(_))));
    }

    #[test]
    fn invalid_inputs_are_refused() {
        let rules = test_rule_set();
        for (base, contributions) in [
            (0.0, 1_000.0),
            (-1.0, 1_000.0),
            (100_000.0, -1.0),
            (f64::NAN, 1_000.0),
            (100_000.0, f64::INFINITY),
        ] {
            let err = estimate_deduction_limit(&rules, on(), base, contributions).unwrap_err();
            assert!(matches!(err, AppError::InvalidInput(_)), "{base} / {contributions}: {err:?}");
        }
    }
}
