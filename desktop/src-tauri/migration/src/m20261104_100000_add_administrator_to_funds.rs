use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Compensacao de prejuizo entre fundos: a lei so deixa compensar entre fundos da mesma administradora (ou do mesmo
// intermediario por conta e ordem). `fund_details.administrator` e o texto livre que o usuario informa por fundo; fundos
// com o mesmo texto (sem diferenca de caixa ou de espacos) e do mesmo tipo formam um grupo. Nulo = fundo isolado.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("ALTER TABLE fund_details ADD COLUMN administrator TEXT").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("ALTER TABLE fund_details DROP COLUMN administrator").await?;
        Ok(())
    }
}
