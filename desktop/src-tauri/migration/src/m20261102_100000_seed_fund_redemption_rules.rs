use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED: &str = "2026-10-22";
const EFFECTIVE_FROM: &str = "2005-01-01";

// IR retido no resgate de cotas de fundos abertos (tabela regressiva por prazo de aplicacao). Fonte secundaria: confira no
// gov.br. Mesmo conteudo da tabela embarcada (`finance_core/tax_rules.rs`); `INSERT OR IGNORE` por causa do indice unico
// (chave, vigencia), pra nao duplicar o que `sync_tax_rules` ja tenha trazido nem sobrescrever edicao do usuario. Os limites
// de dias (180, 360, 720) ficam no codigo (`domain/tax/fund_redemption.rs`).
// ATENCAO: descricao e fonte vao dentro de aspas simples no SQL, entao NAO podem ter apostrofo.
// (key, value, description, source)
const SOURCE: &str = "Lei 11.033/2004, art. 1 (secondary sources; verify on gov.br)";
const SEEDS: &[(&str, f64, &str)] = &[
    ("fund_ir_long_up_to_180", 0.225, "IR withheld on the redemption of a long-term open fund, quotas held up to 180 days"),
    ("fund_ir_long_181_360", 0.20, "IR withheld on the redemption of a long-term open fund, quotas held from 181 to 360 days"),
    ("fund_ir_long_361_720", 0.175, "IR withheld on the redemption of a long-term open fund, quotas held from 361 to 720 days"),
    ("fund_ir_long_over_720", 0.15, "IR withheld on the redemption of a long-term open fund, quotas held over 720 days"),
    ("fund_ir_short_up_to_180", 0.225, "IR withheld on the redemption of a short-term open fund, quotas held up to 180 days"),
    ("fund_ir_short_over_180", 0.20, "IR withheld on the redemption of a short-term open fund, quotas held over 180 days"),
    ("fund_ir_equity", 0.15, "IR withheld on the redemption of an equity fund, flat on the gain and with no come-cotas"),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (key, value, description) in SEEDS {
            db.execute_unprepared(&format!(
                "INSERT OR IGNORE INTO tax_rules \
                 (rule_key, effective_from, value, default_value, description, source, verified_at, user_edited, updated_at) \
                 VALUES ('{key}', '{EFFECTIVE_FROM}', {value}, {value}, '{description}', '{SOURCE}', '{VERIFIED}', 0, '{VERIFIED}')"
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
