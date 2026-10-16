use sea_orm_migration::{prelude::*, schema::*};

/// Fase 26 — o Lume foi aposentado e a escolha Local/Remote da Finance API
/// (`finance_api_settings`, Fase 14.3) sempre foi gravada e nunca lida pelo app.
/// Apaga a tabela. As credenciais das fontes de dados agora ficam no keyring do
/// SO (Settings → Data sources), não no banco.
///
/// O `down` recria a tabela vazia (schema igual ao da migration original); a
/// linha de configuração que existia não volta, mas nada a usava.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FinanceApiSettings::Table).if_exists().to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FinanceApiSettings::Table)
                    .if_not_exists()
                    .col(pk_auto(FinanceApiSettings::Id))
                    .col(string(FinanceApiSettings::Mode))
                    .col(string_null(FinanceApiSettings::RemoteUrl))
                    .col(string(FinanceApiSettings::UpdatedAt))
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum FinanceApiSettings {
    Table,
    Id,
    Mode,
    RemoteUrl,
    UpdatedAt,
}
