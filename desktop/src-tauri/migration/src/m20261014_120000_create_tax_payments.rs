use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaxPayments::Table)
                    .if_not_exists()
                    .col(pk_auto(TaxPayments::Id))
                    .col(integer(TaxPayments::WorkspaceId))
                    .col(string(TaxPayments::Period))
                    .col(double(TaxPayments::Amount))
                    .col(string(TaxPayments::PaidDate))
                    .col(integer_null(TaxPayments::GeneralTransactionId))
                    .col(string(TaxPayments::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tax_payments_workspace")
                            .from(TaxPayments::Table, TaxPayments::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    // Apagar a despesa em Finanças Gerais não apaga o registro
                    // do pagamento — só desfaz o vínculo.
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tax_payments_general_transaction")
                            .from(TaxPayments::Table, TaxPayments::GeneralTransactionId)
                            .to(GeneralTransactions::Table, GeneralTransactions::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Um pagamento por mês de apuração por Workspace.
        manager
            .create_index(
                Index::create()
                    .name("idx_tax_payments_workspace_period")
                    .table(TaxPayments::Table)
                    .col(TaxPayments::WorkspaceId)
                    .col(TaxPayments::Period)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TaxPayments::Table).to_owned())
            .await
    }
}

/// Fase 24 — DARF de renda variável marcado como pago. `period` é o mês de
/// apuração (`YYYY-MM`); a apuração em si é recalculada sob demanda a partir
/// de `transactions`, nunca persistida.
#[derive(DeriveIden)]
enum TaxPayments {
    Table,
    Id,
    WorkspaceId,
    Period,
    Amount,
    PaidDate,
    GeneralTransactionId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Workspace {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum GeneralTransactions {
    Table,
    Id,
}
