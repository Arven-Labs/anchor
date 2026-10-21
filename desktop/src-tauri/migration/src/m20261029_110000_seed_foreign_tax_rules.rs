use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED: &str = "2026-10-21";
const EFFECTIVE_FROM: &str = "2024-01-01";

// IR de aplicações financeiras no exterior (Lei 14.754/2023): alíquota anual de 15% sobre ganho de capital e proventos. Fonte
// secundária: confira no gov.br. Mesmo conteúdo da tabela embarcada (`finance_core/tax_rules.rs`); `INSERT OR IGNORE` por causa
// do índice único (chave, vigência), pra não duplicar o que `sync_tax_rules` já tenha trazido nem sobrescrever edição do usuário.
// (key, value, description, source)
const SEEDS: &[(&str, f64, &str, &str)] = &[
    ("foreign_rate", 0.15, "Yearly IR on gains and dividends of financial investments held abroad (stocks, ETFs, REITs), as a flat 15% on the net result of the year, with no monthly exemption and no monthly DARF", "Lei 14.754/2023, arts. 2 to 4 (secondary sources; verify on gov.br)"),
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
