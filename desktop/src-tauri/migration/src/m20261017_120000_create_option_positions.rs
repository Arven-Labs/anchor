use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OptionPositions::Table)
                    .if_not_exists()
                    .col(pk_auto(OptionPositions::Id))
                    .col(integer(OptionPositions::PortfolioId))
                    .col(string(OptionPositions::SeriesTicker))
                    .col(string(OptionPositions::UnderlyingTicker))
                    .col(string(OptionPositions::OptionType))
                    .col(double(OptionPositions::Strike))
                    .col(string(OptionPositions::Expiration))
                    .col(string(OptionPositions::Side))
                    .col(double(OptionPositions::Quantity))
                    .col(double(OptionPositions::Premium))
                    .col(string(OptionPositions::OpenDate))
                    .col(double_null(OptionPositions::OpenFee))
                    .col(string_null(OptionPositions::Notes))
                    .col(string_null(OptionPositions::CloseKind))
                    .col(double_null(OptionPositions::ClosePrice))
                    .col(string_null(OptionPositions::CloseDate))
                    .col(double_null(OptionPositions::CloseFee))
                    .col(string(OptionPositions::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_option_positions_portfolio")
                            .from(OptionPositions::Table, OptionPositions::PortfolioId)
                            .to(Portfolio::Table, Portfolio::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OptionPositions::Table).to_owned())
            .await
    }
}

/// Fase 18, 3ª fatia — posição em opção da B3, comprada ou lançada. Tabela
/// própria (não classe de ativo): o livro de compra/venda só tem posição
/// comprada e não comporta opção lançada. Aberta = `CloseDate` nulo; o
/// encerramento é manual (`CloseKind`: closed, expired ou exercised).
#[derive(DeriveIden)]
enum OptionPositions {
    Table,
    Id,
    PortfolioId,
    SeriesTicker,
    UnderlyingTicker,
    OptionType,
    Strike,
    Expiration,
    Side,
    Quantity,
    Premium,
    OpenDate,
    OpenFee,
    Notes,
    CloseKind,
    ClosePrice,
    CloseDate,
    CloseFee,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Portfolio {
    Table,
    Id,
}
