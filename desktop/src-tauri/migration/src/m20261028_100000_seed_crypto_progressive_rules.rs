use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED: &str = "2026-10-19";
const EFFECTIVE_FROM: &str = "2019-01-01";
const SOURCE: &str = "Lei 8.981/1995, art. 21, with Lei 13.259/2016 (secondary sources; verify on gov.br)";

// Tabela progressiva do IR de cripto sobre o ganho tributável acumulado no ano: 15% até R$ 5 mi, 17,5% até R$ 10 mi,
// 20% até R$ 30 mi e 22,5% acima. Mesmo conteúdo da tabela embarcada (`finance_core/tax_rules.rs`); `INSERT OR IGNORE`
// pelo índice único (chave, vigência), pra não duplicar o que `sync_tax_rules` já tenha trazido nem sobrescrever
// edição do usuário. (key, value, description)
const SEEDS: &[(&str, f64, &str)] = &[
    ("crypto_second_tier_gain_limit", 10000000.0, "Yearly taxable crypto gain up to which the second bracket rate (17.5%) holds"),
    ("crypto_third_tier_gain_limit", 30000000.0, "Yearly taxable crypto gain up to which the third bracket rate (20%) holds"),
    ("crypto_rate_second_tier", 0.175, "Rate on the part of the yearly taxable crypto gain between the first and second tier limits"),
    ("crypto_rate_third_tier", 0.20, "Rate on the part of the yearly taxable crypto gain between the second and third tier limits"),
    ("crypto_rate_fourth_tier", 0.225, "Rate on the part of the yearly taxable crypto gain above the third tier limit"),
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
        // As duas regras antigas diziam que o app só avisava acima da 1ª faixa; só o texto muda, nunca o valor editado.
        db.execute_unprepared(
            "UPDATE tax_rules SET description = 'IR on crypto gains above the monthly exemption, first bracket (up to the first tier limit of taxable gain in the year; higher brackets follow the tier rules)' \
             WHERE rule_key = 'crypto_rate' AND description LIKE '%Anchor only warns%'",
        )
        .await?;
        db.execute_unprepared(
            "UPDATE tax_rules SET description = 'Yearly taxable crypto gain up to which the first bracket rate holds (15%)' \
             WHERE rule_key = 'crypto_first_tier_gain_limit' AND description LIKE '%Anchor warns%'",
        )
        .await?;
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
