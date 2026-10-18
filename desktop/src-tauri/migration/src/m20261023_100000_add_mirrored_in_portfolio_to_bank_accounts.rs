use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 35, entrega 8 — marca por conta: o saldo dela já está na carteira (posições de renda fixa
// importadas da Movimentação), então o patrimônio não o conta de novo no caixa. O padrão é falso:
// toda conta existente segue contando como antes.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE bank_accounts ADD COLUMN mirrored_in_portfolio BOOLEAN NOT NULL DEFAULT 0")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE bank_accounts DROP COLUMN mirrored_in_portfolio")
            .await?;
        Ok(())
    }
}
