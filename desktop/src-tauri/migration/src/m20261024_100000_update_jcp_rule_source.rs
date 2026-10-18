use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// A fonte do JCP a 17,5% dizia "secondary sources; verify". Foi conferida no texto publicado da LC 224/2025:
/// o art. 8º dá ao § 2º do art. 9º da Lei 9.249/1995 a alíquota de 17,5%, e o art. 14, III, a faz valer desde
/// 1º/1/2026. Só o texto da fonte muda, nunca o valor, que o usuário pode ter editado.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE tax_rules SET source = 'LC 224/2025, art. 8º (amends Lei 9.249/1995, art. 9º, § 2º), in force from 2026-01-01 (art. 14, III)' \
                 WHERE rule_key = 'jcp_irrf_rate' AND effective_from = '2026-01-01' AND source LIKE '%secondary sources%'",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
