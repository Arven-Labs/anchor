// Previdência privada: estimativa do IR no resgate, regime com tabela por prazo (Lei 11.053/2004, art. 1º). É
// **informativa**: quem retém é a seguradora ou a entidade, o imposto sai do valor do resgate, e nada aqui entra na
// apuração de IR, no patrimônio nem na Rentabilidade.
//
// Regras confirmadas em texto literal (Sessão 399): 35% até 2 anos, 30% de 2 a 4, 25% de 4 a 6, 20% de 6 a 8, 15% de 8 a
// 10 e 10% acima de 10 anos, contados da data do aporte; IR retido na fonte e definitivo; base do PGBL = o valor total do
// resgate, base do VGBL = só o rendimento. As alíquotas vêm de `tax_rules` (editáveis); os limites das faixas são código.
//
// NÃO confirmado: como separar principal de rendimento num resgate parcial e qual aporte um resgate consome primeiro. Por
// isso o rendimento do VGBL e o prazo são informados por quem usa (o informe da seguradora traz os dois).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::rules::{self, RuleSet};
use crate::error::AppError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PensionKind {
    Pgbl,
    Vgbl,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RegressiveEstimate {
    /// Chave da regra da faixa usada (ex.: `pension_ir_regressive_over_10y`), para a tela mostrar a fonte.
    pub rule_key: &'static str,
    pub rate: f64,
    /// Sobre o que a alíquota incide: o valor total (PGBL) ou só o rendimento (VGBL).
    pub taxable_base: f64,
    pub estimated_tax: f64,
    pub net_amount: f64,
    /// 15% retidos como antecipação por quem não optou pela tabela (art. 3º); só informativo, o ajuste anual não é calculado.
    pub advance_rate: f64,
}

/// A chave da faixa do prazo em anos desde o aporte. Limite superior inclusivo (a lei diz "inferior ou igual a").
pub fn bracket_key(years: f64) -> &'static str {
    if years < 2.0 {
        rules::PENSION_IR_REGRESSIVE_UP_TO_2Y
    } else if years <= 4.0 {
        rules::PENSION_IR_REGRESSIVE_2_TO_4Y
    } else if years <= 6.0 {
        rules::PENSION_IR_REGRESSIVE_4_TO_6Y
    } else if years <= 8.0 {
        rules::PENSION_IR_REGRESSIVE_6_TO_8Y
    } else if years <= 10.0 {
        rules::PENSION_IR_REGRESSIVE_8_TO_10Y
    } else {
        rules::PENSION_IR_REGRESSIVE_OVER_10Y
    }
}

/// `redeemed` é o valor bruto do resgate; `income` é a parcela de rendimento (obrigatória no VGBL, ignorada no PGBL);
/// `years` é o prazo desde o aporte. Alíquotas vigentes em `on`.
pub fn estimate_regressive(
    rules: &RuleSet,
    on: NaiveDate,
    kind: PensionKind,
    redeemed: f64,
    income: Option<f64>,
    years: f64,
) -> Result<RegressiveEstimate, AppError> {
    if !redeemed.is_finite() || redeemed <= 0.0 {
        return Err(AppError::InvalidInput(format!("invalid redeemed amount {redeemed}")));
    }
    if !years.is_finite() || years < 0.0 {
        return Err(AppError::InvalidInput(format!("invalid years since contribution {years}")));
    }
    let taxable_base = match kind {
        PensionKind::Pgbl => redeemed,
        PensionKind::Vgbl => match income {
            None => return Err(AppError::InvalidInput("a VGBL redemption needs the income part".to_string())),
            Some(i) if !i.is_finite() || i < 0.0 => {
                return Err(AppError::InvalidInput(format!("invalid income part {i}")));
            }
            Some(i) if i > redeemed => {
                return Err(AppError::InvalidInput(format!(
                    "income part {i} is greater than the redeemed amount {redeemed}"
                )));
            }
            Some(_) => redeemed,
        },
    };
    let rule_key = bracket_key(years);
    let rate = rules.get(rule_key, on)?;
    let advance_rate = rules.get(rules::PENSION_IR_ADVANCE_RATE, on)?;
    let estimated_tax = taxable_base * rate;
    Ok(RegressiveEstimate { rule_key, rate, taxable_base, estimated_tax, net_amount: redeemed - estimated_tax, advance_rate })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax::rules::{test_rule_set, Rule};

    const EPS: f64 = 1e-9;

    fn on() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 22).unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn the_bracket_follows_the_years_with_an_inclusive_upper_limit() {
        let cases = [
            (0.0, rules::PENSION_IR_REGRESSIVE_UP_TO_2Y),
            (1.5, rules::PENSION_IR_REGRESSIVE_UP_TO_2Y),
            (2.0, rules::PENSION_IR_REGRESSIVE_UP_TO_2Y),
            (2.01, rules::PENSION_IR_REGRESSIVE_2_TO_4Y),
            (4.0, rules::PENSION_IR_REGRESSIVE_2_TO_4Y),
            (4.01, rules::PENSION_IR_REGRESSIVE_4_TO_6Y),
            (6.0, rules::PENSION_IR_REGRESSIVE_4_TO_6Y),
            (6.01, rules::PENSION_IR_REGRESSIVE_6_TO_8Y),
            (8.0, rules::PENSION_IR_REGRESSIVE_6_TO_8Y),
            (8.01, rules::PENSION_IR_REGRESSIVE_8_TO_10Y),
            (10.0, rules::PENSION_IR_REGRESSIVE_8_TO_10Y),
            (10.01, rules::PENSION_IR_REGRESSIVE_OVER_10Y),
            (35.0, rules::PENSION_IR_REGRESSIVE_OVER_10Y),
        ];
        for (years, key) in cases {
            assert_eq!(bracket_key(years), key, "{years} anos");
        }
    }

    #[test]
    fn pgbl_is_taxed_on_the_whole_redemption() {
        let e = estimate_regressive(&test_rule_set(), on(), PensionKind::Pgbl, 1000.0, None, 11.0).unwrap();

        assert_eq!(e.rule_key, rules::PENSION_IR_REGRESSIVE_OVER_10Y);
        assert!(close(e.rate, 0.10));
        assert!(close(e.taxable_base, 1000.0));
        assert!(close(e.estimated_tax, 100.0));
        assert!(close(e.net_amount, 900.0));
        assert!(close(e.advance_rate, 0.15));
    }

    #[test]
    fn vgbl_is_taxed_only_on_the_income() {
        let e = estimate_regressive(&test_rule_set(), on(), PensionKind::Vgbl, 1000.0, Some(200.0), 11.0).unwrap();

        assert!(close(e.taxable_base, 200.0));
        assert!(close(e.estimated_tax, 20.0));
        assert!(close(e.net_amount, 980.0));
    }

    #[test]
    fn a_young_contribution_pays_the_highest_rate() {
        let e = estimate_regressive(&test_rule_set(), on(), PensionKind::Vgbl, 500.0, Some(100.0), 1.0).unwrap();

        assert!(close(e.rate, 0.35));
        assert!(close(e.estimated_tax, 35.0));
        assert!(close(e.net_amount, 465.0));
    }

    #[test]
    fn pgbl_ignores_an_income_figure() {
        let e = estimate_regressive(&test_rule_set(), on(), PensionKind::Pgbl, 1000.0, Some(200.0), 11.0).unwrap();

        assert!(close(e.taxable_base, 1000.0));
        assert!(close(e.estimated_tax, 100.0));
    }

    #[test]
    fn vgbl_income_cannot_exceed_the_redemption() {
        let err = estimate_regressive(&test_rule_set(), on(), PensionKind::Vgbl, 1000.0, Some(1200.0), 11.0).unwrap_err();

        assert!(matches!(&err, AppError::InvalidInput(msg) if msg.contains("income")), "{err:?}");
    }

    #[test]
    fn vgbl_needs_the_income_figure() {
        let err = estimate_regressive(&test_rule_set(), on(), PensionKind::Vgbl, 1000.0, None, 11.0).unwrap_err();

        assert!(matches!(&err, AppError::InvalidInput(msg) if msg.contains("income")), "{err:?}");
    }

    #[test]
    fn invalid_numbers_are_refused_with_the_field_name() {
        let rules = test_rule_set();
        let redeemed = |v: f64| estimate_regressive(&rules, on(), PensionKind::Pgbl, v, None, 11.0).unwrap_err();
        for bad in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            let err = redeemed(bad);
            assert!(matches!(&err, AppError::InvalidInput(msg) if msg.contains("redeemed")), "{bad}: {err:?}");
        }
        for bad in [-1.0, f64::NAN] {
            let err = estimate_regressive(&rules, on(), PensionKind::Pgbl, 1000.0, None, bad).unwrap_err();
            assert!(matches!(&err, AppError::InvalidInput(msg) if msg.contains("years")), "{bad}: {err:?}");
        }
        let err = estimate_regressive(&rules, on(), PensionKind::Vgbl, 1000.0, Some(-1.0), 11.0).unwrap_err();
        assert!(matches!(&err, AppError::InvalidInput(msg) if msg.contains("income")), "{err:?}");
    }

    #[test]
    fn it_uses_the_rate_the_rule_set_holds() {
        // o dono editou a faixa de mais de 10 anos para 12%
        let rules = RuleSet::new(vec![
            Rule {
                key: rules::PENSION_IR_REGRESSIVE_OVER_10Y.to_string(),
                effective_from: NaiveDate::from_ymd_opt(2005, 1, 1).unwrap(),
                value: 0.12,
            },
            Rule {
                key: rules::PENSION_IR_ADVANCE_RATE.to_string(),
                effective_from: NaiveDate::from_ymd_opt(2005, 1, 1).unwrap(),
                value: 0.15,
            },
        ]);

        let e = estimate_regressive(&rules, on(), PensionKind::Pgbl, 1000.0, None, 11.0).unwrap();

        assert!(close(e.rate, 0.12));
        assert!(close(e.estimated_tax, 120.0));
    }

    #[test]
    fn a_rule_not_yet_in_force_on_the_date_is_not_found() {
        let rules = RuleSet::new(vec![
            Rule {
                key: rules::PENSION_IR_REGRESSIVE_OVER_10Y.to_string(),
                effective_from: NaiveDate::from_ymd_opt(2030, 1, 1).unwrap(),
                value: 0.10,
            },
            Rule {
                key: rules::PENSION_IR_ADVANCE_RATE.to_string(),
                effective_from: NaiveDate::from_ymd_opt(2005, 1, 1).unwrap(),
                value: 0.15,
            },
        ]);

        let err = estimate_regressive(&rules, on(), PensionKind::Pgbl, 1000.0, None, 11.0).unwrap_err();

        assert!(matches!(err, AppError::NotFound(_)), "{err:?}");
    }
}
