use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

// PTAX do Banco Central (fechamento, compra e venda, em reais por unidade da moeda), um valor por dia útil e moeda.
// Tabela própria, e não `stock_price_history`: o IR de exterior (Lei 14.754) lê a PTAX por regra legal, e os demais
// leitores de câmbio (proventos, rentabilidade, cripto) seguem no câmbio de mercado.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PtaxDaily::Table)
                    .if_not_exists()
                    .col(pk_auto(PtaxDaily::Id))
                    .col(string(PtaxDaily::Currency))
                    .col(string(PtaxDaily::ReferenceDate))
                    .col(double(PtaxDaily::Buy))
                    .col(double(PtaxDaily::Sell))
                    .col(string(PtaxDaily::Source))
                    .col(string(PtaxDaily::FetchedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_ptax_daily_unique")
                    .table(PtaxDaily::Table)
                    .col(PtaxDaily::Currency)
                    .col(PtaxDaily::ReferenceDate)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(PtaxDaily::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum PtaxDaily {
    Table,
    Id,
    Currency,
    ReferenceDate,
    Buy,
    Sell,
    Source,
    FetchedAt,
}
