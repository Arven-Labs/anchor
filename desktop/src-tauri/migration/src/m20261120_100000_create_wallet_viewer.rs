use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Visualizador de wallets (ADR 0002). Tabelas só do módulo e **sem nenhuma chave estrangeira**,
// nem para `workspace`: o módulo é global e não toca as tabelas de investimento. Um teste
// (`wallet_viewer::tests`) falha se aparecer uma FK aqui.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // No máximo uma linha; sem linha, o módulo está desligado.
        manager
            .create_table(
                Table::create()
                    .table(WalletViewerSettings::Table)
                    .if_not_exists()
                    .col(pk_auto(WalletViewerSettings::Id))
                    .col(boolean(WalletViewerSettings::Enabled))
                    .col(string(WalletViewerSettings::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(WalletViewerAddress::Table)
                    .if_not_exists()
                    .col(pk_auto(WalletViewerAddress::Id))
                    .col(string(WalletViewerAddress::Address))
                    .col(string_null(WalletViewerAddress::Label))
                    .col(string(WalletViewerAddress::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_wallet_viewer_address_unique")
                    .table(WalletViewerAddress::Table)
                    .col(WalletViewerAddress::Address)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(WalletViewerAddress::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(WalletViewerSettings::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum WalletViewerSettings {
    Table,
    Id,
    Enabled,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WalletViewerAddress {
    Table,
    Id,
    Address,
    Label,
    CreatedAt,
}
