// Regras de IR sobre investimentos (pessoa física, Brasil) — porta de
// `lume/api/app/services/tax_rules_service.py`. Não existe fonte oficial
// legível por máquina: esta tabela É a fonte, curada à mão e versionada no
// repo, com a fonte e a data de verificação de cada valor.
//
// Cada regra vale a partir de `effective_from`; mudança de lei = entrada nova
// com a data nova, nunca editar a antiga (meses passados são apurados com a
// regra vigente na época). `sync_tax_rules` aplica esta tabela no `tax_rules`
// do banco; o seed da migration `m20261014_110000_create_tax_rules` guarda o
// mesmo conteúdo (mudar um exige mudar o outro; o teste
// `embedded_rules_match_the_migration_seed` pega a divergência).

use chrono::NaiveDate;

pub struct TaxRule {
    pub key: &'static str,
    pub value: f64,
    pub effective_from: NaiveDate,
    pub description: &'static str,
    pub source: &'static str,
    pub verified_at: NaiveDate,
}

const RECEITA_BOLSA: &str = "https://www.gov.br/receitafederal/pt-br/assuntos/meu-imposto-de-renda/pagamento/renda-variavel/bolsa-de-valores-1";
const RECEITA_CALCULO: &str = "https://www.gov.br/receitafederal/pt-br/assuntos/meu-imposto-de-renda/pagamento/renda-variavel/bolsa-de-valores-1/calculo-e-pagamento-do-imposto";
const RECEITA_ISENCOES: &str = "https://www.gov.br/receitafederal/pt-br/assuntos/meu-imposto-de-renda/pagamento/renda-variavel/bolsa-de-valores-1/isencoes";
const RECEITA_REVAR: &str = "https://www.gov.br/receitafederal/pt-br/assuntos/meu-imposto-de-renda/pagamento/renda-variavel/manual";

/// (key, value, effective_from, description, source)
const RULES: &[(&str, f64, (i32, u32, u32), &str, &str)] = &[
    ("common_rate", 0.15, (2005, 1, 1), "Rate on monthly net gains from regular (swing) trades of stocks, units, ETFs and BDRs on B3", RECEITA_BOLSA),
    ("day_trade_rate", 0.20, (2005, 1, 1), "Rate on monthly net gains from day trades on B3", RECEITA_BOLSA),
    ("fii_rate", 0.20, (2005, 1, 1), "Rate on net gains from selling real estate fund (FII/Fiagro) quotas, regular or day trade", RECEITA_REVAR),
    ("stock_exemption_monthly_sales", 20000.0, (2005, 1, 1), "Monthly stock sales up to this amount make stock gains exempt (stocks only; not units — SC Cosit 145/2021 —, ETFs, BDRs, FIIs or day trade)", RECEITA_ISENCOES),
    ("irrf_common_rate", 0.00005, (2005, 1, 1), "Withholding on the sale value of regular trades (0.005%), deductible from the tax due", RECEITA_CALCULO),
    ("irrf_day_trade_rate", 0.01, (2005, 1, 1), "Withholding on day trade gains (1%), deductible from the tax due", "Lei 9.430/1996, art. 8º (verify on planalto.gov.br)"),
    ("darf_minimum", 10.0, (2005, 1, 1), "Tax below this amount is not paid that month; it accumulates into the following months", RECEITA_REVAR),
    ("darf_code_b3", 6015.0, (2005, 1, 1), "DARF revenue code for net gains on the stock exchange (individuals)", RECEITA_CALCULO),
    ("jcp_irrf_rate", 0.15, (1996, 1, 1), "Withholding on interest on equity (JCP)", "Lei 9.249/1995, art. 9º, § 2º"),
    ("jcp_irrf_rate", 0.175, (2026, 1, 1), "Withholding on interest on equity (JCP), raised by LC 224/2025", "LC 224/2025 (secondary sources; verify on planalto.gov.br)"),
    ("fixed_income_ir_cdb", 0.225, (2005, 1, 1), "IR withheld on CDB redemptions, as a flat rate (22.5% is the first bracket, up to 180 days). The regressive table and IOF are not modeled", "Lei 11.033/2004, art. 1º (secondary sources; verify on planalto.gov.br)"),
    ("fixed_income_ir_lci", 0.0, (2005, 1, 1), "IR on LCI redemptions (exempt for individuals)", "Lei 11.033/2004, art. 3º (secondary sources; verify on planalto.gov.br)"),
    ("fixed_income_ir_lca", 0.0, (2005, 1, 1), "IR on LCA redemptions (exempt for individuals)", "Lei 11.033/2004, art. 3º (secondary sources; verify on planalto.gov.br)"),
    ("fixed_income_ir_cri", 0.0, (2005, 1, 1), "IR on CRI redemptions (exempt for individuals)", "Lei 11.033/2004, art. 3º (secondary sources; verify on planalto.gov.br)"),
    ("fixed_income_ir_cra", 0.0, (2005, 1, 1), "IR on CRA redemptions (exempt for individuals)", "Lei 11.033/2004, art. 3º (secondary sources; verify on planalto.gov.br)"),
    ("crypto_exemption_monthly_sales", 35000.0, (2019, 1, 1), "Monthly crypto sales up to this amount make crypto gains exempt (domestic exchanges)", "IN RFB 1.888/2019 (secondary sources; verify on gov.br)"),
];

fn date(ymd: (i32, u32, u32)) -> NaiveDate {
    NaiveDate::from_ymd_opt(ymd.0, ymd.1, ymd.2).expect("valid date in the rules table")
}

/// Data da última conferência de toda a tabela contra as fontes.
fn verified_at() -> NaiveDate {
    date((2026, 10, 14))
}

pub fn rules_br() -> Vec<TaxRule> {
    RULES
        .iter()
        .map(|&(key, value, effective_from, description, source)| TaxRule {
            key,
            value,
            effective_from: date(effective_from),
            description,
            source,
            verified_at: verified_at(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn there_is_one_rule_per_key_and_effective_date() {
        let rules = rules_br();
        let unique: HashSet<_> = rules.iter().map(|r| (r.key, r.effective_from)).collect();
        assert_eq!(unique.len(), rules.len());
        assert_eq!(rules.len(), 16);
    }

    #[test]
    fn jcp_withholding_has_both_the_old_and_the_2026_rate() {
        let rules = rules_br();
        let jcp: Vec<_> = rules.iter().filter(|r| r.key == "jcp_irrf_rate").collect();
        assert_eq!(jcp.len(), 2);
        assert!(jcp.iter().any(|r| r.value == 0.15 && r.effective_from.to_string() == "1996-01-01"));
        assert!(jcp.iter().any(|r| r.value == 0.175 && r.effective_from.to_string() == "2026-01-01"));
    }
}
