use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Atualização opcional de 2023 dos bens no exterior (Lei 14.754, art. 14): o saldo de 31/12/2023 na moeda do ativo, como vem
// do documento da custodiante. O app converte pela PTAX de venda do último dia útil de dezembro de 2023 e usa o resultado como
// custo da posição daquela data. Nulo = o usuário não optou pela atualização (todas as linhas que já existem).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("ALTER TABLE assets ADD COLUMN update_2023_value REAL").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("ALTER TABLE assets DROP COLUMN update_2023_value").await?;
        Ok(())
    }
}
