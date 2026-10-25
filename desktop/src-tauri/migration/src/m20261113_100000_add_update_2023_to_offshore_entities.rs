use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Atualização opcional de 2023 das participações em controladas no exterior (Lei 14.754, art. 14 §1º IV): o patrimônio líquido
// proporcional à participação em 31/12/2023 (§2º III) e, dentro dele, a parcela de lucros (acumulados e reservas de lucros),
// os dois na moeda da entidade. O app converte pela PTAX de venda do último dia útil de dezembro de 2023. Nulo = o usuário não
// optou pela atualização (todas as linhas que já existem).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE offshore_entities ADD COLUMN update_2023_equity REAL").await?;
        db.execute_unprepared("ALTER TABLE offshore_entities ADD COLUMN update_2023_profits REAL").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE offshore_entities DROP COLUMN update_2023_profits").await?;
        db.execute_unprepared("ALTER TABLE offshore_entities DROP COLUMN update_2023_equity").await?;
        Ok(())
    }
}
