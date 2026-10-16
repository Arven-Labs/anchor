use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Borrowers::Table)
                    .if_not_exists()
                    .col(pk_auto(Borrowers::Id))
                    .col(integer(Borrowers::WorkspaceId))
                    .col(string(Borrowers::Nome))
                    .col(string_null(Borrowers::Notes))
                    .col(string(Borrowers::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_borrowers_workspace")
                            .from(Borrowers::Table, Borrowers::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Loans::Table)
                    .if_not_exists()
                    .col(pk_auto(Loans::Id))
                    .col(integer(Loans::WorkspaceId))
                    .col(integer(Loans::BorrowerId))
                    .col(integer_null(Loans::BankAccountId))
                    .col(integer_null(Loans::GeneralTransactionId))
                    .col(double(Loans::Principal))
                    .col(double_null(Loans::TaxaMensalPercentual))
                    .col(string(Loans::DataInicio))
                    .col(boolean(Loans::IncludeInNetWorth).default(true))
                    .col(string_null(Loans::Notes))
                    .col(string(Loans::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_loans_workspace")
                            .from(Loans::Table, Loans::WorkspaceId)
                            .to(Workspace::Table, Workspace::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_loans_borrower")
                            .from(Loans::Table, Loans::BorrowerId)
                            .to(Borrowers::Table, Borrowers::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_loans_bank_account")
                            .from(Loans::Table, Loans::BankAccountId)
                            .to(BankAccounts::Table, BankAccounts::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_loans_general_transaction")
                            .from(Loans::Table, Loans::GeneralTransactionId)
                            .to(GeneralTransactions::Table, GeneralTransactions::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(LoanRepayments::Table)
                    .if_not_exists()
                    .col(pk_auto(LoanRepayments::Id))
                    .col(integer(LoanRepayments::LoanId))
                    .col(integer_null(LoanRepayments::BankAccountId))
                    .col(integer_null(LoanRepayments::GeneralTransactionId))
                    .col(string(LoanRepayments::Data))
                    .col(double(LoanRepayments::ValorRecebido))
                    .col(double(LoanRepayments::ValorBaixado))
                    .col(string_null(LoanRepayments::Notes))
                    .col(string(LoanRepayments::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_loan_repayments_loan")
                            .from(LoanRepayments::Table, LoanRepayments::LoanId)
                            .to(Loans::Table, Loans::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_loan_repayments_bank_account")
                            .from(LoanRepayments::Table, LoanRepayments::BankAccountId)
                            .to(BankAccounts::Table, BankAccounts::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_loan_repayments_general_transaction")
                            .from(LoanRepayments::Table, LoanRepayments::GeneralTransactionId)
                            .to(GeneralTransactions::Table, GeneralTransactions::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(LoanRepayments::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Loans::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Borrowers::Table).to_owned())
            .await
    }
}

/// Fase 29 — devedores cadastrados (texto livre cairia em nomes duplicados
/// com grafias diferentes; entidade própria deixa o relatório por pessoa
/// possível).
#[derive(DeriveIden)]
enum Borrowers {
    Table,
    Id,
    WorkspaceId,
    Nome,
    Notes,
    CreatedAt,
}

/// Fase 29 — empréstimo concedido (lado oposto de `Liabilities`).
/// `TaxaMensalPercentual` nulo = sem juros (caso real: empréstimo informal
/// em família). `IncludeInNetWorth` decide por empréstimo se o saldo a
/// receber entra no patrimônio. `BankAccountId`/`GeneralTransactionId` só
/// existem quando o usuário escolheu uma conta — sem conta o empréstimo é
/// puro registro e não mexe no caixa.
#[derive(DeriveIden)]
enum Loans {
    Table,
    Id,
    WorkspaceId,
    BorrowerId,
    BankAccountId,
    GeneralTransactionId,
    Principal,
    TaxaMensalPercentual,
    DataInicio,
    IncludeInNetWorth,
    Notes,
    CreatedAt,
}

/// Baixa manual: `ValorRecebido` (dinheiro que entrou) e `ValorBaixado`
/// (quanto abate do saldo) são independentes de propósito — a diferença é
/// "extra recebido", não vira receita sozinha.
#[derive(DeriveIden)]
enum LoanRepayments {
    Table,
    Id,
    LoanId,
    BankAccountId,
    GeneralTransactionId,
    Data,
    ValorRecebido,
    ValorBaixado,
    Notes,
    CreatedAt,
}

/// Só `Table`/`Id` — apontam FK pras tabelas donas, criadas em migrations
/// anteriores.
#[derive(DeriveIden)]
enum Workspace {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum BankAccounts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum GeneralTransactions {
    Table,
    Id,
}
