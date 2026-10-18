use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fase 22, fatia 1 — fechamento ajustado a dividendos e desdobramentos (`adjclose` do Yahoo), a base do retorno
// total do backtest. Fica numa tabela própria, ao lado de `stock_price_history` (que guarda só o `close`), para não
// mexer nos coletores que já gravam essa tabela. Mesma chave: um valor por `(ticker, price_date)`.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS stock_adjusted_close (\
                    ticker TEXT NOT NULL, \
                    price_date TEXT NOT NULL, \
                    adj_close_price REAL NOT NULL, \
                    fetched_at TEXT NOT NULL, \
                    PRIMARY KEY (ticker, price_date))",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("DROP TABLE IF EXISTS stock_adjusted_close").await?;
        Ok(())
    }
}
