use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 35, entrega 11 — conta Porquinho com estimativa automática do rendimento: o percentual do CDI com que o
// app estima sozinho cada resgate depois que o extrato é revisto (nulo = desligado, como toda conta existente).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE bank_accounts ADD COLUMN auto_yield_pct_of_cdi REAL")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE bank_accounts DROP COLUMN auto_yield_pct_of_cdi")
            .await?;
        Ok(())
    }
}
