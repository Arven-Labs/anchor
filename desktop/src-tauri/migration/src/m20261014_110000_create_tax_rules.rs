use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED: &str = "2026-10-14";
const RECEITA_BOLSA: &str = "https://www.gov.br/receitafederal/pt-br/assuntos/meu-imposto-de-renda/pagamento/renda-variavel/bolsa-de-valores-1";
const RECEITA_CALCULO: &str = "https://www.gov.br/receitafederal/pt-br/assuntos/meu-imposto-de-renda/pagamento/renda-variavel/bolsa-de-valores-1/calculo-e-pagamento-do-imposto";
const RECEITA_ISENCOES: &str = "https://www.gov.br/receitafederal/pt-br/assuntos/meu-imposto-de-renda/pagamento/renda-variavel/bolsa-de-valores-1/isencoes";
const RECEITA_REVAR: &str = "https://www.gov.br/receitafederal/pt-br/assuntos/meu-imposto-de-renda/pagamento/renda-variavel/manual";

// Mesmo conteúdo de `api/app/services/tax_rules_service.py` do Lume
// (`/v1/tax-rules/br`) — este seed é o fallback offline, `sync_tax_rules`
// atualiza a partir do Lume depois. (key, value, effective_from, description, source)
const SEEDS: &[(&str, f64, &str, &str, &str)] = &[
    ("common_rate", 0.15, "2005-01-01", "Rate on monthly net gains from regular (swing) trades of stocks, units, ETFs and BDRs on B3", RECEITA_BOLSA),
    ("day_trade_rate", 0.20, "2005-01-01", "Rate on monthly net gains from day trades on B3", RECEITA_BOLSA),
    ("fii_rate", 0.20, "2005-01-01", "Rate on net gains from selling real estate fund (FII/Fiagro) quotas, regular or day trade", RECEITA_REVAR),
    ("stock_exemption_monthly_sales", 20000.0, "2005-01-01", "Monthly stock sales up to this amount make stock gains exempt (stocks only; not units — SC Cosit 145/2021 —, ETFs, BDRs, FIIs or day trade)", RECEITA_ISENCOES),
    ("irrf_common_rate", 0.00005, "2005-01-01", "Withholding on the sale value of regular trades (0.005%), deductible from the tax due", RECEITA_CALCULO),
    ("irrf_day_trade_rate", 0.01, "2005-01-01", "Withholding on day trade gains (1%), deductible from the tax due", "Lei 9.430/1996, art. 8º (verify on planalto.gov.br)"),
    ("darf_minimum", 10.0, "2005-01-01", "Tax below this amount is not paid that month; it accumulates into the following months", RECEITA_REVAR),
    ("darf_code_b3", 6015.0, "2005-01-01", "DARF revenue code for net gains on the stock exchange (individuals)", RECEITA_CALCULO),
    ("jcp_irrf_rate", 0.15, "1996-01-01", "Withholding on interest on equity (JCP)", "Lei 9.249/1995, art. 9º, § 2º"),
    ("jcp_irrf_rate", 0.175, "2026-01-01", "Withholding on interest on equity (JCP), raised by LC 224/2025", "LC 224/2025 (secondary sources; verify on planalto.gov.br)"),
    ("crypto_exemption_monthly_sales", 35000.0, "2019-01-01", "Monthly crypto sales up to this amount make crypto gains exempt (domestic exchanges)", "IN RFB 1.888/2019 (secondary sources; verify on gov.br)"),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaxRules::Table)
                    .if_not_exists()
                    .col(pk_auto(TaxRules::Id))
                    .col(string(TaxRules::RuleKey))
                    .col(string(TaxRules::EffectiveFrom))
                    .col(double(TaxRules::Value))
                    .col(double(TaxRules::DefaultValue))
                    .col(string(TaxRules::Description))
                    .col(string(TaxRules::Source))
                    .col(string(TaxRules::VerifiedAt))
                    .col(boolean(TaxRules::UserEdited).default(false))
                    .col(string(TaxRules::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        // Uma regra por (chave, vigência): mudança de lei = linha nova com a
        // data nova, nunca sobrescreve a antiga (meses passados são apurados
        // com a regra que valia na época).
        manager
            .create_index(
                Index::create()
                    .name("idx_tax_rules_key_effective_from")
                    .table(TaxRules::Table)
                    .col(TaxRules::RuleKey)
                    .col(TaxRules::EffectiveFrom)
                    .unique()
                    .to_owned(),
            )
            .await?;

        for (key, value, effective_from, description, source) in SEEDS {
            manager
                .exec_stmt(
                    Query::insert()
                        .into_table(TaxRules::Table)
                        .columns([
                            TaxRules::RuleKey,
                            TaxRules::EffectiveFrom,
                            TaxRules::Value,
                            TaxRules::DefaultValue,
                            TaxRules::Description,
                            TaxRules::Source,
                            TaxRules::VerifiedAt,
                            TaxRules::UserEdited,
                            TaxRules::UpdatedAt,
                        ])
                        .values_panic([
                            (*key).into(),
                            (*effective_from).into(),
                            (*value).into(),
                            (*value).into(),
                            (*description).into(),
                            (*source).into(),
                            VERIFIED.into(),
                            false.into(),
                            VERIFIED.into(),
                        ])
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TaxRules::Table).to_owned())
            .await
    }
}

/// Fase 24 — parâmetros editáveis das regras de IR (alíquotas, limites,
/// IRRF, mínimo de DARF, códigos). `value` é o que o motor usa;
/// `default_value` é o último valor oficial conhecido (seed ou Lume), pra
/// "reset" e pra comparar com o que o usuário editou (`user_edited`).
#[derive(DeriveIden)]
enum TaxRules {
    Table,
    Id,
    RuleKey,
    EffectiveFrom,
    Value,
    DefaultValue,
    Description,
    Source,
    VerifiedAt,
    UserEdited,
    UpdatedAt,
}
