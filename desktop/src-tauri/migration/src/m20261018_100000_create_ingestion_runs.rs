use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IngestionRuns::Table)
                    .if_not_exists()
                    .col(pk_auto(IngestionRuns::Id))
                    .col(string(IngestionRuns::TriggerKind))
                    .col(string(IngestionRuns::StartedAt))
                    .col(string_null(IngestionRuns::FinishedAt))
                    .col(string(IngestionRuns::Status))
                    .col(integer(IngestionRuns::ItemsOk).default(0))
                    .col(integer(IngestionRuns::ItemsFailed).default(0))
                    .col(string_null(IngestionRuns::Error))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IngestionRuns::Table).to_owned())
            .await
    }
}

/// Registro de cada rodada da ingestão diária. `TriggerKind` é `startup`,
/// `scheduled` ou `manual` (não se chama `trigger` porque é palavra reservada
/// do SQLite); `Status` é `running`, `ok`, `partial` ou `error`. `FinishedAt`
/// nulo com status `running` é uma rodada que o app não chegou a fechar.
#[derive(DeriveIden)]
enum IngestionRuns {
    Table,
    Id,
    TriggerKind,
    StartedAt,
    FinishedAt,
    Status,
    ItemsOk,
    ItemsFailed,
    Error,
}
