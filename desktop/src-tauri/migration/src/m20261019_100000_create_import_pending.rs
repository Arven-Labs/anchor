use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ImportPending::Table)
                    .if_not_exists()
                    .col(pk_auto(ImportPending::Id))
                    .col(integer(ImportPending::PortfolioId))
                    .col(string(ImportPending::Kind))
                    .col(string(ImportPending::Source))
                    .col(text(ImportPending::Payload))
                    .col(string(ImportPending::Status))
                    .col(string(ImportPending::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_import_pending_portfolio")
                            .from(ImportPending::Table, ImportPending::PortfolioId)
                            .to(Portfolio::Table, Portfolio::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(ImportPending::Table).to_owned()).await
    }
}

/// Fase 35 — fila de itens propostos por uma importação (ou, no futuro, pela IA)
/// que o dono do projeto ainda não aprovou nem recusou. `Kind` é `transaction`
/// (depois `dividend`, `bank_statement`); `Source` diz de onde veio
/// (`b3_negociacao`, `anchor_csv`); `Payload` é o JSON do item proposto mais a
/// classe escolhida, sem ids de outras tabelas. `Status` é `pending` ou
/// `discarded`: o descartado fica só para a reimportação não recriar o item, e o
/// aprovado sai da fila (o lançamento gravado é o registro).
#[derive(DeriveIden)]
enum ImportPending {
    Table,
    Id,
    PortfolioId,
    Kind,
    Source,
    Payload,
    Status,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Portfolio {
    Table,
    Id,
}
