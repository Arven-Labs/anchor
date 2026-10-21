use chrono::NaiveDate;

use crate::error::AppError;

// Chaves usadas pelo motor — precisam existir em `tax_rules` (seed da
// migration `m20261014_110000_create_tax_rules` / Lume `/v1/tax-rules/br`).
pub const COMMON_RATE: &str = "common_rate";
pub const DAY_TRADE_RATE: &str = "day_trade_rate";
pub const FII_RATE: &str = "fii_rate";
pub const STOCK_EXEMPTION_MONTHLY_SALES: &str = "stock_exemption_monthly_sales";
pub const IRRF_COMMON_RATE: &str = "irrf_common_rate";
pub const IRRF_DAY_TRADE_RATE: &str = "irrf_day_trade_rate";
pub const DARF_MINIMUM: &str = "darf_minimum";
pub const DARF_CODE_B3: &str = "darf_code_b3";
pub const JCP_IRRF_RATE: &str = "jcp_irrf_rate";
pub const CRYPTO_EXEMPTION_MONTHLY_SALES: &str = "crypto_exemption_monthly_sales";
pub const CRYPTO_RATE: &str = "crypto_rate";
pub const CRYPTO_FIRST_TIER_GAIN_LIMIT: &str = "crypto_first_tier_gain_limit";
pub const CRYPTO_SECOND_TIER_GAIN_LIMIT: &str = "crypto_second_tier_gain_limit";
pub const CRYPTO_THIRD_TIER_GAIN_LIMIT: &str = "crypto_third_tier_gain_limit";
pub const CRYPTO_RATE_SECOND_TIER: &str = "crypto_rate_second_tier";
pub const CRYPTO_RATE_THIRD_TIER: &str = "crypto_rate_third_tier";
pub const CRYPTO_RATE_FOURTH_TIER: &str = "crypto_rate_fourth_tier";
pub const DARF_CODE_CRYPTO: &str = "darf_code_crypto";
pub const FOREIGN_RATE: &str = "foreign_rate";

/// IR retido no resgate de renda fixa, uma taxa fixa por papel (a tabela
/// regressiva do IR não é modelada; o IOF mora em `domain::tax::iof`). `None`
/// para papel desconhecido.
pub fn fixed_income_ir_key(instrument: &str) -> Option<&'static str> {
    match instrument {
        "cdb" => Some("fixed_income_ir_cdb"),
        "lci" => Some("fixed_income_ir_lci"),
        "lca" => Some("fixed_income_ir_lca"),
        "cri" => Some("fixed_income_ir_cri"),
        "cra" => Some("fixed_income_ir_cra"),
        _ => None,
    }
}

pub struct Rule {
    pub key: String,
    pub effective_from: NaiveDate,
    pub value: f64,
}

/// Conjunto de regras com histórico: cada chave pode ter várias linhas,
/// uma por vigência. `get` devolve a vigente na data pedida.
pub struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    pub fn get(&self, key: &str, on: NaiveDate) -> Result<f64, AppError> {
        self.rules
            .iter()
            .filter(|r| r.key == key && r.effective_from <= on)
            .max_by_key(|r| r.effective_from)
            .map(|r| r.value)
            .ok_or_else(|| AppError::NotFound(format!("tax rule '{key}' in effect on {on}")))
    }
}

#[cfg(test)]
pub(crate) fn test_rule_set() -> RuleSet {
    let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
    let rule = |key: &str, from: &str, value: f64| Rule {
        key: key.to_string(),
        effective_from: d(from),
        value,
    };
    RuleSet::new(vec![
        rule(COMMON_RATE, "2005-01-01", 0.15),
        rule(DAY_TRADE_RATE, "2005-01-01", 0.20),
        rule(FII_RATE, "2005-01-01", 0.20),
        rule(STOCK_EXEMPTION_MONTHLY_SALES, "2005-01-01", 20000.0),
        rule(IRRF_COMMON_RATE, "2005-01-01", 0.00005),
        rule(IRRF_DAY_TRADE_RATE, "2005-01-01", 0.01),
        rule(DARF_MINIMUM, "2005-01-01", 10.0),
        rule(DARF_CODE_B3, "2005-01-01", 6015.0),
        rule(JCP_IRRF_RATE, "1996-01-01", 0.15),
        rule(JCP_IRRF_RATE, "2026-01-01", 0.175),
        rule(CRYPTO_EXEMPTION_MONTHLY_SALES, "2019-01-01", 35000.0),
        rule(CRYPTO_RATE, "2019-01-01", 0.15),
        rule(CRYPTO_FIRST_TIER_GAIN_LIMIT, "2019-01-01", 5_000_000.0),
        rule(CRYPTO_SECOND_TIER_GAIN_LIMIT, "2019-01-01", 10_000_000.0),
        rule(CRYPTO_THIRD_TIER_GAIN_LIMIT, "2019-01-01", 30_000_000.0),
        rule(CRYPTO_RATE_SECOND_TIER, "2019-01-01", 0.175),
        rule(CRYPTO_RATE_THIRD_TIER, "2019-01-01", 0.20),
        rule(CRYPTO_RATE_FOURTH_TIER, "2019-01-01", 0.225),
        rule(DARF_CODE_CRYPTO, "2019-01-01", 4600.0),
        rule(FOREIGN_RATE, "2024-01-01", 0.15),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_rule_in_effect_on_date() {
        let rules = test_rule_set();
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();

        assert_eq!(rules.get(JCP_IRRF_RATE, d("2025-12-31")).unwrap(), 0.15);
        assert_eq!(rules.get(JCP_IRRF_RATE, d("2026-01-01")).unwrap(), 0.175);
    }

    #[test]
    fn missing_rule_is_an_error_not_zero() {
        let rules = test_rule_set();
        let d = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();

        assert!(matches!(rules.get("nope", d), Err(AppError::NotFound(_))));
        // Antes da primeira vigência também não existe regra.
        let early = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
        assert!(rules.get(COMMON_RATE, early).is_err());
    }
}
