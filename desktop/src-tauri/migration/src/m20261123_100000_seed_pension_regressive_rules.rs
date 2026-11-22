use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED: &str = "2026-11-22";
const EFFECTIVE_FROM: &str = "2005-01-01";

// IR retido no resgate de previdencia privada, regime com tabela por prazo desde o aporte (Lei 11.053/2004, art. 1) e a
// antecipacao de quem nao optou (art. 3). Valores do texto literal da lei no Planalto, lido em 2026-11-22 (Sessao 399).
// Mesmo conteudo da tabela embarcada (`finance_core/tax_rules.rs`; o teste `embedded_rules_match_the_migration_seed`
// pega a divergencia); `INSERT OR IGNORE` por causa do indice unico (chave, vigencia), pra nao duplicar o que
// `sync_tax_rules` ja tenha trazido nem sobrescrever edicao do usuario. Os limites das faixas (2, 4, 6, 8 e 10 anos) ficam
// no codigo (`domain/tax/pension.rs`). So dados: nenhuma mudanca de schema.
// ATENCAO: descricao e fonte vao dentro de aspas simples no SQL, entao NAO podem ter apostrofo.
// (key, value, description, source)
const TABLE: &str = "Lei 11.053/2004, art. 1º (Planalto text read on 2026-11-22; verify the current wording)";
const ADVANCE: &str = "Lei 11.053/2004, art. 3º (Planalto text read on 2026-11-22; verify the current wording)";
const SEEDS: &[(&str, f64, &str, &str)] = &[
    ("pension_ir_regressive_up_to_2y", 0.35, "IR withheld on a private pension redemption (table regime), contribution held up to 2 years", TABLE),
    ("pension_ir_regressive_2_to_4y", 0.30, "IR withheld on a private pension redemption (table regime), contribution held over 2 and up to 4 years", TABLE),
    ("pension_ir_regressive_4_to_6y", 0.25, "IR withheld on a private pension redemption (table regime), contribution held over 4 and up to 6 years", TABLE),
    ("pension_ir_regressive_6_to_8y", 0.22, "IR withheld on a private pension redemption (table regime), contribution held over 6 and up to 8 years", TABLE),
    ("pension_ir_regressive_8_to_10y", 0.15, "IR withheld on a private pension redemption (table regime), contribution held over 8 and up to 10 years", TABLE),
    ("pension_ir_regressive_over_10y", 0.10, "IR withheld on a private pension redemption (table regime), contribution held over 10 years", TABLE),
    ("pension_ir_advance_rate", 0.15, "IR withheld as an advance on a private pension redemption when the participant did not opt for the table regime (adjusted in the annual return; not computed here)", ADVANCE),
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
