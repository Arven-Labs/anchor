use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Fundos de investimento (classe `fundo`): o tipo de cada um, que decide o come-cotas (longo prazo, curto prazo ou ações),
// e o valor da cota lançado à mão por data. Não existe preço por unidade para ativo sem cotação de mercado, e nada lê o
// informe diário da CVM, então o valor da cota é do usuário.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FundDetails::Table)
                    .if_not_exists()
                    .col(pk_auto(FundDetails::Id))
                    .col(integer(FundDetails::AssetId).unique_key())
                    .col(string(FundDetails::Term))
                    .col(string(FundDetails::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_fund_details_asset")
                            .from(FundDetails::Table, FundDetails::AssetId)
                            .to(Assets::Table, Assets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(FundQuotaValues::Table)
                    .if_not_exists()
                    .col(pk_auto(FundQuotaValues::Id))
                    .col(integer(FundQuotaValues::AssetId))
                    .col(string(FundQuotaValues::QuotaDate))
                    .col(double(FundQuotaValues::Value))
                    .col(string(FundQuotaValues::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_fund_quota_values_asset")
                            .from(FundQuotaValues::Table, FundQuotaValues::AssetId)
                            .to(Assets::Table, Assets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_fund_quota_values_unique")
                    .table(FundQuotaValues::Table)
                    .col(FundQuotaValues::AssetId)
                    .col(FundQuotaValues::QuotaDate)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(FundQuotaValues::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(FundDetails::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum FundDetails {
    Table,
    Id,
    AssetId,
    Term,
    CreatedAt,
}

#[derive(DeriveIden)]
enum FundQuotaValues {
    Table,
    Id,
    AssetId,
    QuotaDate,
    Value,
    CreatedAt,
}

/// Só `Table`/`Id`: esta migration não é dona de `assets`, só aponta uma foreign key pra ele.
#[derive(DeriveIden)]
enum Assets {
    Table,
    Id,
}
