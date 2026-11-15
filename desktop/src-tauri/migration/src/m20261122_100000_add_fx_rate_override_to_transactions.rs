use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Câmbio digitado pelo usuário (ADR 0003): reais por 1 unidade da moeda do ativo, para esta transação, no lugar da PTAX e
// do câmbio de mercado do IR de exterior. Só vale em compra, venda e provento de ativo do exterior. Nulo = o câmbio
// automático (todas as linhas que já existem).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE transactions ADD COLUMN fx_rate_override REAL")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE transactions DROP COLUMN fx_rate_override")
            .await?;
        Ok(())
    }
}
