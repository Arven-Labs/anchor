use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED: &str = "2026-10-19";
const EFFECTIVE_FROM: &str = "2019-01-01";

// IR de cripto: alíquota de 15% fixa, teto da 1ª faixa (o app só avisa acima dele) e o código do DARF. Fontes
// secundárias: confira no gov.br. Mesmo conteúdo da tabela embarcada (`finance_core/tax_rules.rs`);
// `INSERT OR IGNORE` por causa do índice único (chave, vigência), pra não duplicar o que `sync_tax_rules` já tenha
// trazido nem sobrescrever edição do usuário. (key, value, description, source)
const SEEDS: &[(&str, f64, &str, &str)] = &[
    ("crypto_rate", 0.15, "IR on crypto gains above the monthly exemption, as a flat 15% (the real table rises to 17.5%, 20% and 22.5% above R$ 5 million of gain in the year; Anchor only warns)", "Lei 8.981/1995, art. 21, with Lei 13.259/2016 (secondary sources; verify on gov.br)"),
    ("crypto_first_tier_gain_limit", 5000000.0, "Yearly crypto gain up to which the 15% rate holds; above it Anchor warns that a higher bracket applies", "Lei 8.981/1995, art. 21, with Lei 13.259/2016 (secondary sources; verify on gov.br)"),
    ("darf_code_crypto", 4600.0, "Revenue code of the DARF for capital gains on crypto (the stock exchange DARF is 6015)", "Receita Federal, GCAP (secondary sources; verify on gov.br)"),
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
