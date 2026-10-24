use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Data em que o imposto retido no exterior foi de fato pago (Lei 14.754, art. 15: o crédito converte pela PTAX de compra do
// dia do pagamento). Só vale em `provento` com `foreign_tax_withheld`. Texto `AAAA-MM-DD`. Nulo = a data do provento (todas
// as linhas que já existem).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE transactions ADD COLUMN foreign_tax_paid_on TEXT")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE transactions DROP COLUMN foreign_tax_paid_on")
            .await?;
        Ok(())
    }
}
