use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Eventos do capital aplicado numa entidade controlada no exterior (Lei 14.754, art. 7º): o custo da participação nasce dos
// aportes e cai nas vendas e devoluções de capital, que geram o ganho de capital. Os valores são na moeda da entidade;
// `event_date` é texto `AAAA-MM-DD`. `kind` é `aporte`, `venda` ou `devolucao`. `brl_cost` (só aporte) é o custo em reais da
// DAA, para aportes antigos; nulo = o app converte pela PTAX do dia. `fraction` (só venda) é a fração da participação vendida.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE offshore_capital_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                entity_id INTEGER NOT NULL REFERENCES offshore_entities(id) ON DELETE CASCADE,
                kind TEXT NOT NULL,
                event_date TEXT NOT NULL,
                amount REAL NOT NULL,
                brl_cost REAL,
                fraction REAL,
                created_at TEXT NOT NULL
            )",
        )
        .await?;
        db.execute_unprepared("CREATE INDEX idx_offshore_capital_events_entity ON offshore_capital_events (entity_id)").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("DROP TABLE offshore_capital_events").await?;
        Ok(())
    }
}
