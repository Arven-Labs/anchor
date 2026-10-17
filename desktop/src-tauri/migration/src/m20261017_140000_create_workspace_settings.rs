use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WorkspaceSettings::Table)
                    .if_not_exists()
                    .col(pk_auto(WorkspaceSettings::Id))
                    .col(integer(WorkspaceSettings::WorkspaceId).unique_key())
                    .col(string(WorkspaceSettings::BaseCurrency).default("BRL"))
                    .col(string(WorkspaceSettings::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workspace_settings_workspace")
                            .from(WorkspaceSettings::Table, WorkspaceSettings::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WorkspaceSettings::Table).to_owned())
            .await
    }
}

/// Fase 19 — configurações por Workspace. Hoje só a moeda base em que a
/// carteira é exibida (`BaseCurrency`, código ISO do catálogo de
/// `finance_core::fx`). Uma linha por workspace; sem linha vale BRL, então
/// nenhum workspace existente precisa de backfill.
#[derive(DeriveIden)]
enum WorkspaceSettings {
    Table,
    Id,
    WorkspaceId,
    BaseCurrency,
    UpdatedAt,
}

/// Só `Table`/`Id` — aponta FK pra tabela dona, criada em migration anterior.
#[derive(DeriveIden)]
enum Workspace {
    Table,
    Id,
}
