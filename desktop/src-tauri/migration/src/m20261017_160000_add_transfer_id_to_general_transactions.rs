use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 19 — transferência entre contas. Uma transferência são duas linhas
// (`transferencia_saida` e `transferencia_entrada`) ligadas pelo mesmo
// `transfer_id`, que é o `id` da linha de saída. Sem FK: apagar uma das linhas
// apaga a outra pelo comando, e linha comum fica com `NULL`.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE general_transactions ADD COLUMN transfer_id INTEGER")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE general_transactions DROP COLUMN transfer_id")
            .await?;
        Ok(())
    }
}
