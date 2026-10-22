use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Entidades controladas no exterior (Lei 14.754/2023) e o resultado do exercício de cada uma. O usuário informa o
// lucro (ou prejuízo) e o imposto pago lá fora na moeda da entidade; a apuração converte pela PTAX de 31/12.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OffshoreEntities::Table)
                    .if_not_exists()
                    .col(pk_auto(OffshoreEntities::Id))
                    .col(integer(OffshoreEntities::WorkspaceId))
                    .col(string(OffshoreEntities::Name))
                    .col(string(OffshoreEntities::Country))
                    .col(string(OffshoreEntities::Currency))
                    .col(double(OffshoreEntities::OwnershipPercent))
                    .col(string_null(OffshoreEntities::Notes))
                    .col(string(OffshoreEntities::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_offshore_entities_workspace")
                            .from(OffshoreEntities::Table, OffshoreEntities::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(OffshoreResults::Table)
                    .if_not_exists()
                    .col(pk_auto(OffshoreResults::Id))
                    .col(integer(OffshoreResults::EntityId))
                    .col(integer(OffshoreResults::Year))
                    .col(double(OffshoreResults::Profit))
                    .col(double(OffshoreResults::TaxPaidAbroad))
                    .col(string(OffshoreResults::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_offshore_results_entity")
                            .from(OffshoreResults::Table, OffshoreResults::EntityId)
                            .to(OffshoreEntities::Table, OffshoreEntities::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_offshore_results_unique")
                    .table(OffshoreResults::Table)
                    .col(OffshoreResults::EntityId)
                    .col(OffshoreResults::Year)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(OffshoreResults::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(OffshoreEntities::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum OffshoreEntities {
    Table,
    Id,
    WorkspaceId,
    Name,
    Country,
    Currency,
    OwnershipPercent,
    Notes,
    CreatedAt,
}

#[derive(DeriveIden)]
enum OffshoreResults {
    Table,
    Id,
    EntityId,
    Year,
    Profit,
    TaxPaidAbroad,
    CreatedAt,
}

/// Só `Table`/`Id`: esta migration não é dona do `workspace`, só aponta uma foreign key pra ele.
#[derive(DeriveIden)]
enum Workspace {
    Table,
    Id,
}
