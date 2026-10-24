use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED: &str = "2026-10-24";

// Regras da Lei 14.754/2023 para fundos fechados e exclusivos: IR das entidades de investimento (FIP, FIDC, ETF que nao e de
// renda fixa), 15% fixo na distribuicao, amortizacao ou resgate e sem come-cotas (art. 24), e as aliquotas do estoque de
// rendimento acumulado ate 31/12/2023 (art. 27: 15%; art. 28: 8% por opcao). Fonte secundaria: confira no gov.br. Mesmo
// conteudo da tabela embarcada (`finance_core/tax_rules.rs`); `INSERT OR IGNORE` por causa do indice unico (chave, vigencia),
// pra nao duplicar o que `sync_tax_rules` ja tenha trazido nem sobrescrever edicao do usuario.
// ATENCAO: descricao e fonte vao dentro de aspas simples no SQL, entao NAO podem ter apostrofo.
// (key, effective_from, value, description, source)
const SEEDS: &[(&str, &str, f64, &str, &str)] = &[
    (
        "fund_ir_investment_entity",
        "2024-01-01",
        0.15,
        "IR withheld on the distribution, amortization or redemption of an investment entity fund (FIP, FIDC, non fixed income ETF), flat on the gain and with no come-cotas",
        "Lei 14.754/2023, arts. 18 and 24 (secondary sources; verify on gov.br)",
    ),
    (
        "fund_estoque_rate_general",
        "2023-12-12",
        0.15,
        "IR on the income of a closed or exclusive fund accrued up to 31/12/2023 (quota value at 31/12/2023 over the cost), the general rule",
        "Lei 14.754/2023, art. 27 (secondary sources; verify on gov.br; its constitutionality is disputed in court)",
    ),
    (
        "fund_estoque_rate_option",
        "2023-12-12",
        0.08,
        "IR on the same accrued income when the investor opts to anticipate the payment, valid only with the full payment",
        "Lei 14.754/2023, art. 28 (secondary sources; verify on gov.br)",
    ),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (key, effective_from, value, description, source) in SEEDS {
            db.execute_unprepared(&format!(
                "INSERT OR IGNORE INTO tax_rules \
                 (rule_key, effective_from, value, default_value, description, source, verified_at, user_edited, updated_at) \
                 VALUES ('{key}', '{effective_from}', {value}, {value}, '{description}', '{source}', '{VERIFIED}', 0, '{VERIFIED}')"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (key, ..) in SEEDS {
            db.execute_unprepared(&format!("DELETE FROM tax_rules WHERE rule_key = '{key}' AND user_edited = 0")).await?;
        }
        Ok(())
    }
}
