use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED: &str = "2026-10-22";
const EFFECTIVE_FROM: &str = "2005-01-01";

// Come-cotas dos fundos abertos: IR cobrado em cotas na ultima data util de maio e de novembro, 15% nos fundos de longo prazo
// e 20% nos de curto prazo. Fonte secundaria: confira no gov.br. Mesmo conteudo da tabela embarcada
// (`finance_core/tax_rules.rs`); `INSERT OR IGNORE` por causa do indice unico (chave, vigencia), pra nao duplicar o que
// `sync_tax_rules` ja tenha trazido nem sobrescrever edicao do usuario.
// ATENCAO: descricao e fonte vao dentro de aspas simples no SQL, entao NAO podem ter apostrofo.
// (key, value, description, source)
const SEEDS: &[(&str, f64, &str, &str)] = &[
    ("come_cotas_rate_long", 0.15, "Come-cotas rate of open funds with a long-term portfolio (average maturity above 365 days), charged in quotas on the last business day of May and November", "Lei 11.033/2004, art. 1 (secondary sources; verify on gov.br)"),
    ("come_cotas_rate_short", 0.20, "Come-cotas rate of open funds with a short-term portfolio (average maturity up to 365 days), charged in quotas on the last business day of May and November", "Lei 11.033/2004, art. 1 (secondary sources; verify on gov.br)"),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (key, value, description, source) in SEEDS {
            db.execute_unprepared(&format!(
                "INSERT OR IGNORE INTO tax_rules \
                 (rule_key, effective_from, value, default_value, description, source, verified_at, user_edited, updated_at) \
                 VALUES ('{key}', '{EFFECTIVE_FROM}', {value}, {value}, '{description}', '{source}', '{VERIFIED}', 0, '{VERIFIED}')"
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
