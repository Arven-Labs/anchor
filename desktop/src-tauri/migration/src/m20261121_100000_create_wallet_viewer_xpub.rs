use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Visualizador de wallets, Bitcoin (Fase 36.4a, ADR 0002). Chave pública estendida (xpub/zpub),
// só do módulo e **sem nenhuma chave estrangeira**. Nunca guarda chave privada. O tipo da chave
// (legado ou SegWit nativo) não é uma coluna: sai da própria chave ao listar.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WalletViewerXpub::Table)
                    .if_not_exists()
                    .col(pk_auto(WalletViewerXpub::Id))
                    .col(string(WalletViewerXpub::Xpub))
                    .col(string_null(WalletViewerXpub::Label))
                    .col(string(WalletViewerXpub::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_wallet_viewer_xpub_unique")
                    .table(WalletViewerXpub::Table)
                    .col(WalletViewerXpub::Xpub)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(WalletViewerXpub::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum WalletViewerXpub {
    Table,
    Id,
    Xpub,
    Label,
    CreatedAt,
}
