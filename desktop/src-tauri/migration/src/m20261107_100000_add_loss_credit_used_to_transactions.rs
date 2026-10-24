use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Crédito de prejuízo que a administradora usou num come-cotas registrado, em BRL de rendimento abatido (o imposto poupado é
// esse valor × a alíquota). Só vale para `come_cotas`. Nulo = não informado (todas as linhas que já existem); 0 = nenhum
// crédito foi usado.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE transactions ADD COLUMN loss_credit_used REAL")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE transactions DROP COLUMN loss_credit_used")
            .await?;
        Ok(())
    }
}
