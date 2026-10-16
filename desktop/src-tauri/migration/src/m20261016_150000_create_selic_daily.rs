use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SelicDaily::Table)
                    .if_not_exists()
                    .col(pk_auto(SelicDaily::Id))
                    .col(string(SelicDaily::SeriesCode))
                    .col(string(SelicDaily::ReferenceDate))
                    .col(double(SelicDaily::ValuePct))
                    .col(string(SelicDaily::Source))
                    .col(string(SelicDaily::FetchedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_selic_daily_unique")
                    .table(SelicDaily::Table)
                    .col(SelicDaily::SeriesCode)
                    .col(SelicDaily::ReferenceDate)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SelicDaily::Table).to_owned())
            .await
    }
}

/// Fase 26 — Meta Selic diária (BCB SGS 432, % a.a.) guardada no `anchor.db`.
/// O BCB leva de 0,4 s a mais de 30 s por janela de 9 anos; sem isto a série
/// inteira (4 janelas) seria baixada a cada abertura do app. Com a tabela só o
/// trecho novo é buscado. `series_code` hoje é só `meta`. Mesmo papel do
/// `selic_daily` do Lume, que cacheava no `finance_api.db`.
#[derive(DeriveIden)]
enum SelicDaily {
    Table,
    Id,
    SeriesCode,
    ReferenceDate,
    ValuePct,
    Source,
    FetchedAt,
}
