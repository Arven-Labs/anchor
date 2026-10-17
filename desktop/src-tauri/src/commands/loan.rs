// Fase 29 — empréstimos concedidos a terceiros (lado oposto de `liability`).
// Saldo a receber, juros e "extra recebido" são sempre derivados na leitura
// (`domain::loan`), nunca gravados. Conta bancária é opcional: com conta, o
// empréstimo e cada baixa geram lançamento em Finanças Gerais; sem conta,
// só registro.

use std::collections::HashSet;

use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::commands::currency::{rates_for, validate_currency, DEFAULT_BASE_CURRENCY};
use crate::domain::loan::{
    accrued_interest, extra_received, outstanding_balance, validate_positive,
};
use crate::entity::{bank_account, borrower, general_transaction, loan, loan_repayment};
use crate::error::AppError;

pub const EMPRESTIMO_CONCEDIDO: &str = "emprestimo_concedido";
pub const RECEBIMENTO_EMPRESTIMO: &str = "recebimento_emprestimo";

fn parse_date(field: &str, value: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|e| AppError::InvalidInput(format!("{field} inválida: {e}")))
}

fn clean_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) async fn ensure_account_in_workspace<C: sea_orm::ConnectionTrait>(
    db: &C,
    bank_account_id: i32,
    workspace_id: i32,
) -> Result<bank_account::Model, AppError> {
    let account = bank_account::Entity::find_by_id(bank_account_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("bank account {bank_account_id}")))?;
    if account.workspace_id != workspace_id {
        return Err(AppError::InvalidInput(
            "bank account belongs to another workspace".to_string(),
        ));
    }
    Ok(account)
}

// ---------------------------------------------------------------------------
// Devedores
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateBorrowerRequest {
    pub workspace_id: i32,
    pub nome: String,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn create_borrower(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateBorrowerRequest,
) -> Result<borrower::Model, AppError> {
    create_borrower_in(db.inner(), request).await
}

// As funções `*_in` recebem `&DatabaseConnection` (os comandos acima só
// repassam o `State`): é o que os testes de fluxo chamam, sem subir o Tauri.
pub async fn create_borrower_in(
    db: &DatabaseConnection,
    request: CreateBorrowerRequest,
) -> Result<borrower::Model, AppError> {
    let nome = request.nome.trim().to_string();
    if nome.is_empty() {
        return Err(AppError::InvalidInput("borrower name is required".to_string()));
    }
    let model = borrower::ActiveModel {
        workspace_id: Set(request.workspace_id),
        nome: Set(nome),
        notes: Set(request.notes.and_then(clean_text)),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(model)
}

#[tauri::command]
pub async fn list_borrowers(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<borrower::Model>, AppError> {
    Ok(borrower::Entity::find()
        .filter(borrower::Column::WorkspaceId.eq(workspace_id))
        .order_by_asc(borrower::Column::Nome)
        .all(db.inner())
        .await?)
}

#[tauri::command]
pub async fn delete_borrower(
    db: tauri::State<'_, DatabaseConnection>,
    borrower_id: i32,
) -> Result<(), AppError> {
    let db = db.inner();
    let has_loans = loan::Entity::find()
        .filter(loan::Column::BorrowerId.eq(borrower_id))
        .one(db)
        .await?
        .is_some();
    if has_loans {
        return Err(AppError::InvalidInput(
            "this borrower still has loans — delete them first".to_string(),
        ));
    }
    borrower::Entity::delete_by_id(borrower_id).exec(db).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Empréstimos
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateLoanRequest {
    pub workspace_id: i32,
    pub borrower_id: i32,
    /// Opcional: com conta, a saída do dinheiro vira lançamento.
    pub bank_account_id: Option<i32>,
    pub principal: f64,
    /// Percentual mensal como digitado (ex. `1.5`). Sem taxa = sem juros.
    pub taxa_mensal_percentual: Option<f64>,
    pub data_inicio: String,
    pub include_in_net_worth: bool,
    pub notes: Option<String>,
    /// Fase 19 — moeda do empréstimo sem conta (BRL se omitida). Com conta
    /// vale sempre a da conta, e o valor pedido aqui é ignorado.
    #[serde(default)]
    pub currency: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateLoanRequest {
    pub loan_id: i32,
    pub taxa_mensal_percentual: Option<f64>,
    pub include_in_net_worth: bool,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterLoanRepaymentRequest {
    pub loan_id: i32,
    pub data: String,
    /// Dinheiro que entrou de fato (0 = perdão/baixa sem recebimento).
    pub valor_recebido: f64,
    /// Quanto abate do saldo, independente do recebido.
    pub valor_baixado: f64,
    /// Opcional: com conta e `valor_recebido > 0`, a entrada vira lançamento.
    pub bank_account_id: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Serialize)]
pub struct LoanView {
    pub id: i32,
    pub workspace_id: i32,
    pub borrower_id: i32,
    pub borrower_nome: String,
    pub bank_account_id: Option<i32>,
    pub principal: f64,
    pub taxa_mensal_percentual: Option<f64>,
    pub data_inicio: String,
    pub include_in_net_worth: bool,
    pub notes: Option<String>,
    pub created_at: String,
    pub currency: String,
    pub accrued_interest: f64,
    pub total_written_off: f64,
    pub extra_received: f64,
    pub saldo_a_receber: f64,
    /// `saldo_a_receber` na moeda base do workspace; `None` sem cotação do par.
    pub saldo_a_receber_in_base: Option<f64>,
    pub repayments: Vec<loan_repayment::Model>,
}

#[tauri::command]
pub async fn create_loan(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateLoanRequest,
) -> Result<loan::Model, AppError> {
    create_loan_in(db.inner(), request).await
}

pub async fn create_loan_in(
    db: &DatabaseConnection,
    request: CreateLoanRequest,
) -> Result<loan::Model, AppError> {
    validate_positive("principal", request.principal)?;
    if let Some(taxa) = request.taxa_mensal_percentual {
        if !taxa.is_finite() || taxa < 0.0 {
            return Err(AppError::InvalidGuard("monthly rate cannot be negative".to_string()));
        }
    }
    parse_date("data_inicio", &request.data_inicio)?;

    let txn = db.begin().await?;

    let borrower = borrower::Entity::find_by_id(request.borrower_id)
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("borrower {}", request.borrower_id)))?;
    if borrower.workspace_id != request.workspace_id {
        return Err(AppError::InvalidInput(
            "borrower belongs to another workspace".to_string(),
        ));
    }

    let mut currency = match request.currency.as_deref() {
        Some(code) => validate_currency(code)?,
        None => DEFAULT_BASE_CURRENCY.to_string(),
    };
    let mut general_transaction_id = None;
    if let Some(account_id) = request.bank_account_id {
        currency = ensure_account_in_workspace(&txn, account_id, request.workspace_id).await?.currency;
        let tx = general_transaction::ActiveModel {
            workspace_id: Set(request.workspace_id),
            bank_account_id: Set(account_id),
            transaction_type: Set(EMPRESTIMO_CONCEDIDO.to_string()),
            category_id: Set(None),
            valor: Set(request.principal),
            transaction_date: Set(request.data_inicio.clone()),
            notes: Set(Some(format!("Loan to {}", borrower.nome))),
            created_at: Set(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        general_transaction_id = Some(tx.id);
    }

    let model = loan::ActiveModel {
        workspace_id: Set(request.workspace_id),
        borrower_id: Set(request.borrower_id),
        bank_account_id: Set(request.bank_account_id),
        general_transaction_id: Set(general_transaction_id),
        principal: Set(request.principal),
        taxa_mensal_percentual: Set(request.taxa_mensal_percentual.filter(|t| *t > 0.0)),
        data_inicio: Set(request.data_inicio),
        include_in_net_worth: Set(request.include_in_net_worth),
        currency: Set(currency),
        notes: Set(request.notes.and_then(clean_text)),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;
    Ok(model)
}

#[tauri::command]
pub async fn update_loan(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateLoanRequest,
) -> Result<loan::Model, AppError> {
    update_loan_in(db.inner(), request).await
}

pub async fn update_loan_in(
    db: &DatabaseConnection,
    request: UpdateLoanRequest,
) -> Result<loan::Model, AppError> {
    if let Some(taxa) = request.taxa_mensal_percentual {
        if !taxa.is_finite() || taxa < 0.0 {
            return Err(AppError::InvalidGuard("monthly rate cannot be negative".to_string()));
        }
    }
    let existing = loan::Entity::find_by_id(request.loan_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("loan {}", request.loan_id)))?;

    let mut active: loan::ActiveModel = existing.into();
    active.taxa_mensal_percentual = Set(request.taxa_mensal_percentual.filter(|t| *t > 0.0));
    active.include_in_net_worth = Set(request.include_in_net_worth);
    active.notes = Set(request.notes.and_then(clean_text));
    Ok(active.update(db).await?)
}

#[tauri::command]
pub async fn list_loans(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<LoanView>, AppError> {
    list_loans_view(db.inner(), workspace_id).await
}

// Extraída pra ser reaproveitada por `commands::net_worth` (mesmo
// precedente de `liability::list_liabilities_view`).
pub async fn list_loans_view(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Vec<LoanView>, AppError> {
    let loans = loan::Entity::find()
        .filter(loan::Column::WorkspaceId.eq(workspace_id))
        .order_by_desc(loan::Column::DataInicio)
        .all(db)
        .await?;
    if loans.is_empty() {
        return Ok(vec![]);
    }

    let borrowers = borrower::Entity::find()
        .filter(borrower::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;
    let loan_ids: Vec<i32> = loans.iter().map(|l| l.id).collect();
    let repayments = loan_repayment::Entity::find()
        .filter(loan_repayment::Column::LoanId.is_in(loan_ids))
        .order_by_asc(loan_repayment::Column::Data)
        .all(db)
        .await?;

    let currencies: HashSet<String> = loans.iter().map(|l| l.currency.clone()).collect();
    let rates = rates_for(db, workspace_id, &currencies).await?;

    let hoje = chrono::Utc::now().date_naive();
    let mut views = Vec::with_capacity(loans.len());
    for l in loans {
        let own: Vec<loan_repayment::Model> =
            repayments.iter().filter(|r| r.loan_id == l.id).cloned().collect();
        let baixados: Vec<f64> = own.iter().map(|r| r.valor_baixado).collect();
        let pairs: Vec<(f64, f64)> =
            own.iter().map(|r| (r.valor_recebido, r.valor_baixado)).collect();
        // `data_inicio` foi validada na criação; se estiver corrompida, os
        // juros viram 0 em vez de derrubar a lista inteira.
        let interest = NaiveDate::parse_from_str(&l.data_inicio, "%Y-%m-%d")
            .map(|inicio| accrued_interest(l.principal, l.taxa_mensal_percentual, inicio, hoje))
            .unwrap_or(0.0);

        let saldo_a_receber = outstanding_balance(l.principal, interest, &baixados);
        views.push(LoanView {
            saldo_a_receber_in_base: rates.convert(saldo_a_receber, &l.currency),
            id: l.id,
            workspace_id: l.workspace_id,
            borrower_id: l.borrower_id,
            borrower_nome: borrowers
                .iter()
                .find(|b| b.id == l.borrower_id)
                .map(|b| b.nome.clone())
                .unwrap_or_default(),
            bank_account_id: l.bank_account_id,
            principal: l.principal,
            taxa_mensal_percentual: l.taxa_mensal_percentual,
            data_inicio: l.data_inicio,
            include_in_net_worth: l.include_in_net_worth,
            notes: l.notes,
            created_at: l.created_at,
            currency: l.currency,
            accrued_interest: interest,
            total_written_off: baixados.iter().sum(),
            extra_received: extra_received(&pairs),
            saldo_a_receber,
            repayments: own,
        });
    }
    Ok(views)
}

#[tauri::command]
pub async fn delete_loan(
    db: tauri::State<'_, DatabaseConnection>,
    loan_id: i32,
) -> Result<(), AppError> {
    delete_loan_in(db.inner(), loan_id).await
}

pub async fn delete_loan_in(db: &DatabaseConnection, loan_id: i32) -> Result<(), AppError> {
    let txn = db.begin().await?;

    let existing = loan::Entity::find_by_id(loan_id)
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("loan {loan_id}")))?;
    let repayments = loan_repayment::Entity::find()
        .filter(loan_repayment::Column::LoanId.eq(loan_id))
        .all(&txn)
        .await?;

    // Mesmo cuidado do `delete_liability`: a FK aponta do empréstimo/baixa pro
    // lançamento, então apagar o empréstimo não apaga o lançamento de caixa; ele
    // é removido explicitamente (senão fica "fantasma" no saldo da conta).
    let mut transaction_ids: Vec<i32> =
        repayments.iter().filter_map(|r| r.general_transaction_id).collect();
    transaction_ids.extend(existing.general_transaction_id);

    loan_repayment::Entity::delete_many()
        .filter(loan_repayment::Column::LoanId.eq(loan_id))
        .exec(&txn)
        .await?;
    loan::Entity::delete_by_id(loan_id).exec(&txn).await?;
    if !transaction_ids.is_empty() {
        general_transaction::Entity::delete_many()
            .filter(general_transaction::Column::Id.is_in(transaction_ids))
            .exec(&txn)
            .await?;
    }

    txn.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Baixas
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn register_loan_repayment(
    db: tauri::State<'_, DatabaseConnection>,
    request: RegisterLoanRepaymentRequest,
) -> Result<loan_repayment::Model, AppError> {
    register_loan_repayment_in(db.inner(), request).await
}

pub async fn register_loan_repayment_in(
    db: &DatabaseConnection,
    request: RegisterLoanRepaymentRequest,
) -> Result<loan_repayment::Model, AppError> {
    validate_positive("valor_baixado", request.valor_baixado)?;
    if !request.valor_recebido.is_finite() || request.valor_recebido < 0.0 {
        return Err(AppError::InvalidGuard(
            "valor_recebido não pode ser negativo".to_string(),
        ));
    }
    parse_date("data", &request.data)?;

    let txn = db.begin().await?;

    let l = loan::Entity::find_by_id(request.loan_id)
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("loan {}", request.loan_id)))?;
    let borrower = borrower::Entity::find_by_id(l.borrower_id).one(&txn).await?;

    let mut general_transaction_id = None;
    if let Some(account_id) = request.bank_account_id {
        let account = ensure_account_in_workspace(&txn, account_id, l.workspace_id).await?;
        if account.currency != l.currency {
            return Err(AppError::InvalidInput(format!(
                "the loan is in {} but the account is in {}",
                l.currency, account.currency
            )));
        }
        if request.valor_recebido > 0.0 {
            let tx = general_transaction::ActiveModel {
                workspace_id: Set(l.workspace_id),
                bank_account_id: Set(account_id),
                transaction_type: Set(RECEBIMENTO_EMPRESTIMO.to_string()),
                category_id: Set(None),
                valor: Set(request.valor_recebido),
                transaction_date: Set(request.data.clone()),
                notes: Set(Some(format!(
                    "Repayment from {}",
                    borrower.map(|b| b.nome).unwrap_or_default()
                ))),
                created_at: Set(chrono::Utc::now().to_rfc3339()),
                ..Default::default()
            }
            .insert(&txn)
            .await?;
            general_transaction_id = Some(tx.id);
        }
    }

    let model = loan_repayment::ActiveModel {
        loan_id: Set(request.loan_id),
        bank_account_id: Set(request.bank_account_id),
        general_transaction_id: Set(general_transaction_id),
        data: Set(request.data),
        valor_recebido: Set(request.valor_recebido),
        valor_baixado: Set(request.valor_baixado),
        notes: Set(request.notes.and_then(clean_text)),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;
    Ok(model)
}

#[tauri::command]
pub async fn delete_loan_repayment(
    db: tauri::State<'_, DatabaseConnection>,
    repayment_id: i32,
) -> Result<(), AppError> {
    delete_loan_repayment_in(db.inner(), repayment_id).await
}

pub async fn delete_loan_repayment_in(
    db: &DatabaseConnection,
    repayment_id: i32,
) -> Result<(), AppError> {
    let txn = db.begin().await?;

    let existing = loan_repayment::Entity::find_by_id(repayment_id)
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("repayment {repayment_id}")))?;

    loan_repayment::Entity::delete_by_id(repayment_id).exec(&txn).await?;
    if let Some(tx_id) = existing.general_transaction_id {
        general_transaction::Entity::delete_by_id(tx_id).exec(&txn).await?;
    }

    txn.commit().await?;
    Ok(())
}
