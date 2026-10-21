use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Imposto retido no exterior sobre um provento (ex.: 30% nos EUA), na moeda do ativo. A apuração da Lei 14.754 o usa como
// crédito, limitado ao imposto devido no Brasil. Nulo = sem retenção informada (todas as linhas que já existem).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE transactions ADD COLUMN foreign_tax_withheld REAL")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE transactions DROP COLUMN foreign_tax_withheld")
            .await?;
        Ok(())
    }
}
