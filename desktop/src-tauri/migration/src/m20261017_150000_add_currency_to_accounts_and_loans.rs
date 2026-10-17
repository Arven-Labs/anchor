use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 19 — moeda por conta bancária e por empréstimo. Tudo que já existia era
// BRL implícito, então o default `'BRL'` dispensa backfill. Dívidas e
// lançamentos não ganham coluna: herdam a moeda da conta. SQLite só adiciona
// uma coluna por `ALTER TABLE`.
const TABLES: [&str; 2] = ["bank_accounts", "loans"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in TABLES {
            db.execute_unprepared(&format!(
                "ALTER TABLE {table} ADD COLUMN currency VARCHAR NOT NULL DEFAULT 'BRL'"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in TABLES {
            db.execute_unprepared(&format!("ALTER TABLE {table} DROP COLUMN currency")).await?;
        }
        Ok(())
    }
}
