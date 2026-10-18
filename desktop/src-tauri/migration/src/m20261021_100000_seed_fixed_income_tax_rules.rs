use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED: &str = "2026-10-19";

// Fase 35, entrega 4 — IR retido no resgate de renda fixa, uma taxa fixa por papel.
// O CDB usa 22,5% (primeira faixa da tabela regressiva, resgate em até 180 dias);
// LCI, LCA, CRI e CRA são isentos para pessoa física. Fontes secundárias: confira
// no Planalto. Mesmo conteúdo da tabela embarcada (`finance_core/tax_rules.rs`);
// `INSERT OR IGNORE` por causa do índice único (chave, vigência), pra não
// duplicar o que `sync_tax_rules` já tenha trazido. (key, value, description, source)
const SEEDS: &[(&str, f64, &str, &str)] = &[
    ("fixed_income_ir_cdb", 0.225, "IR withheld on CDB redemptions, as a flat rate (22.5% is the first bracket, up to 180 days). The regressive table and IOF are not modeled", "Lei 11.033/2004, art. 1º (secondary sources; verify on planalto.gov.br)"),
    ("fixed_income_ir_lci", 0.0, "IR on LCI redemptions (exempt for individuals)", "Lei 11.033/2004, art. 3º (secondary sources; verify on planalto.gov.br)"),
    ("fixed_income_ir_lca", 0.0, "IR on LCA redemptions (exempt for individuals)", "Lei 11.033/2004, art. 3º (secondary sources; verify on planalto.gov.br)"),
    ("fixed_income_ir_cri", 0.0, "IR on CRI redemptions (exempt for individuals)", "Lei 11.033/2004, art. 3º (secondary sources; verify on planalto.gov.br)"),
    ("fixed_income_ir_cra", 0.0, "IR on CRA redemptions (exempt for individuals)", "Lei 11.033/2004, art. 3º (secondary sources; verify on planalto.gov.br)"),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (key, value, description, source) in SEEDS {
            db.execute_unprepared(&format!(
                "INSERT OR IGNORE INTO tax_rules \
                 (rule_key, effective_from, value, default_value, description, source, verified_at, user_edited, updated_at) \
                 VALUES ('{key}', '2005-01-01', {value}, {value}, '{description}', '{source}', '{VERIFIED}', 0, '{VERIFIED}')"
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
