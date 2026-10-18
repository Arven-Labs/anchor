use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// IR de cripto tem DARF próprio (código 4600), então um mês de apuração pode ter dois pagamentos: o da B3 e o da
// cripto. `kind` ('b3' | 'crypto') entra na chave única; os pagamentos que já existem são todos da B3.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE tax_payments ADD COLUMN kind TEXT NOT NULL DEFAULT 'b3'").await?;
        db.execute_unprepared("DROP INDEX IF EXISTS idx_tax_payments_workspace_period").await?;
        db.execute_unprepared(
            "CREATE UNIQUE INDEX idx_tax_payments_workspace_period_kind ON tax_payments (workspace_id, period, kind)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP INDEX IF EXISTS idx_tax_payments_workspace_period_kind").await?;
        db.execute_unprepared("DELETE FROM tax_payments WHERE kind <> 'b3'").await?;
        db.execute_unprepared("ALTER TABLE tax_payments DROP COLUMN kind").await?;
        db.execute_unprepared(
            "CREATE UNIQUE INDEX idx_tax_payments_workspace_period ON tax_payments (workspace_id, period)",
        )
        .await?;
        Ok(())
    }
}
