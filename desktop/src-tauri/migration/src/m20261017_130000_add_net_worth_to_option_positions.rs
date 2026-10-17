use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 18 × Fase 12 — posição em opção no patrimônio. `include_in_net_worth`
// nasce ligada (como nos empréstimos). `mark_price`/`mark_date`/`marked_at`
// guardam o último preço negociado trazido por "Refresh market prices", porque
// o patrimônio não usa rede (a marcação baixa ~80 MB da B3): `mark_date` é a
// data do último negócio e `marked_at` é quando o preço foi atualizado.
// SQLite só adiciona uma coluna por `ALTER TABLE`.
const ADD_COLUMNS: [&str; 4] = [
    "ALTER TABLE option_positions ADD COLUMN include_in_net_worth BOOLEAN NOT NULL DEFAULT 1",
    "ALTER TABLE option_positions ADD COLUMN mark_price DOUBLE",
    "ALTER TABLE option_positions ADD COLUMN mark_date VARCHAR",
    "ALTER TABLE option_positions ADD COLUMN marked_at VARCHAR",
];

const COLUMNS: [&str; 4] = ["include_in_net_worth", "mark_price", "mark_date", "marked_at"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for statement in ADD_COLUMNS {
            db.execute_unprepared(statement).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for column in COLUMNS {
            db.execute_unprepared(&format!("ALTER TABLE option_positions DROP COLUMN {column}")).await?;
        }
        Ok(())
    }
}
