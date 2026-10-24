use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Distribuições de lucros que o usuário recebeu de uma entidade controlada no exterior (Lei 14.754, art. 5º §11): reduzem o
// "crédito de dividendo a receber" do ano de origem na ficha Bens e Direitos, sem nova tributação. O valor é o que foi
// recebido, na moeda da entidade; `received_on` é texto `AAAA-MM-DD`.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE offshore_distributions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                entity_id INTEGER NOT NULL REFERENCES offshore_entities(id) ON DELETE CASCADE,
                origin_year INTEGER NOT NULL,
                received_on TEXT NOT NULL,
                amount REAL NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .await?;
        db.execute_unprepared("CREATE INDEX idx_offshore_distributions_entity ON offshore_distributions (entity_id)").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("DROP TABLE offshore_distributions").await?;
        Ok(())
    }
}
