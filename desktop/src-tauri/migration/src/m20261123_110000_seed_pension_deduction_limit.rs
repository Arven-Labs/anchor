use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED: &str = "2026-11-22";
const EFFECTIVE_FROM: &str = "2005-01-01";

// Limite de deducao das contribuicoes ao PGBL na declaracao: 12% do total dos rendimentos computados na base de calculo
// (Lei 9.532/1997, art. 11, redacao da Lei 10.887/2004). Valor do texto compilado do Planalto, lido em 2026-11-22
// (Sessao 404). Mesmo conteudo da tabela embarcada (`finance_core/tax_rules.rs`; o teste
// `embedded_rules_match_the_migration_seed` pega a divergencia); `INSERT OR IGNORE` por causa do indice unico (chave,
// vigencia), pra nao duplicar o que `sync_tax_rules` ja tenha trazido nem sobrescrever edicao do usuario. So dados:
// nenhuma mudanca de schema.
// ATENCAO: descricao e fonte vao dentro de aspas simples no SQL, entao NAO podem ter apostrofo.
// (key, value, description, source)
const SOURCE: &str = "Lei 9.532/1997, art. 11, wording of Lei 10.887/2004 (Planalto compiled text read on 2026-11-22; verify the current wording)";
const SEEDS: &[(&str, f64, &str, &str)] = &[(
    "pension_pgbl_deduction_limit",
    0.15,
    "Limit on the deduction of PGBL contributions, as a fraction of the total income in the tax base of the annual return (the deduction also needs contributions to the general or the own social security regime, and only the complete model deducts)",
    SOURCE,
)];

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
