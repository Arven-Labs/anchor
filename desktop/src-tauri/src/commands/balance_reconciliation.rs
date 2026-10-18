// Fase 35, entrega 9 — conciliação do saldo de uma conta pelo saldo real do banco. O saldo da conta
// Porquinho sai só dos fluxos do extrato (aplicação − resgate) e não tem o rendimento; a B3 explica o de
// alguns resgates (entrega 4), mas cobre só parte. O dono do projeto digita o saldo que o app do banco
// mostra numa data, e a diferença contra o saldo do ledger naquela data é o rendimento que faltava
// (inclusive o ainda não resgatado), gravado como uma receita. É exato no total e não depende de modelo
// nem de CDI; não diz o rendimento de cada resgate. O que já foi aprovado pela B3 está no ledger, então
// não conta de novo, e conciliar duas vezes na mesma data não duplica: na segunda a diferença já é zero.
use chrono::{NaiveDate, Utc};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};

use crate::commands::bank_account::balance_on;
use crate::commands::general_transaction::{create_general_transaction_in, CreateGeneralTransactionRequest};
use crate::entity::{bank_account, general_transaction};
use crate::error::AppError;

#[derive(Deserialize)]
pub struct ReconcileRequest {
    pub bank_account_id: i32,
    /// Saldo que o banco mostra, na moeda da conta.
    pub bank_balance: f64,
    /// `YYYY-MM-DD`: o dia a que o saldo do banco se refere.
    pub date: String,
}

#[derive(Serialize)]
pub struct ReconcilePreview {
    pub date: String,
    pub bank_balance: f64,
    /// Saldo do ledger no fim do dia, sem o rendimento que falta.
    pub ledger_balance: f64,
    /// `bank_balance − ledger_balance`, em centavos; só vira receita se for positiva.
    pub difference: f64,
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

async fn preview_in(db: &DatabaseConnection, request: &ReconcileRequest) -> Result<(bank_account::Model, ReconcilePreview), AppError> {
    let account = bank_account::Entity::find_by_id(request.bank_account_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("bank account {}", request.bank_account_id)))?;
    if !request.bank_balance.is_finite() || request.bank_balance < 0.0 {
        return Err(AppError::InvalidInput("the bank balance must be a number of zero or more".to_string()));
    }
    let date = NaiveDate::parse_from_str(&request.date, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidInput(format!("invalid date: '{}'", request.date)))?;
    if date > Utc::now().date_naive() {
        return Err(AppError::InvalidInput("the balance date cannot be in the future".to_string()));
    }
    let ledger_balance = round2(balance_on(db, account.id, &request.date).await?);
    let bank_balance = round2(request.bank_balance);
    let preview = ReconcilePreview {
        date: request.date.clone(),
        bank_balance,
        ledger_balance,
        difference: round2(bank_balance - ledger_balance),
    };
    Ok((account, preview))
}

pub async fn preview_balance_reconciliation_in(
    db: &DatabaseConnection,
    request: ReconcileRequest,
) -> Result<ReconcilePreview, AppError> {
    Ok(preview_in(db, &request).await?.1)
}

/// Grava a receita pela diferença. Recusa quando o banco mostra menos que o ledger (falta um fluxo
/// no extrato, ou um rendimento aprovado a mais) ou quando não há diferença: nada a conciliar.
pub async fn confirm_balance_reconciliation_in(
    db: &DatabaseConnection,
    request: ReconcileRequest,
) -> Result<general_transaction::Model, AppError> {
    let (account, preview) = preview_in(db, &request).await?;
    if preview.difference <= 0.0 {
        return Err(AppError::InvalidInput(if preview.difference == 0.0 {
            "the ledger already matches the bank balance: nothing to reconcile".to_string()
        } else {
            "the bank balance is lower than the ledger balance: a withdrawal may be missing from the statement, or a yield was approved for too much".to_string()
        }));
    }
    create_general_transaction_in(
        db,
        CreateGeneralTransactionRequest {
            workspace_id: account.workspace_id,
            bank_account_id: account.id,
            transaction_type: "receita".to_string(),
            category_id: None,
            valor: preview.difference,
            transaction_date: preview.date.clone(),
            notes: Some(format!(
                "Yield by balance reconciliation: bank balance {:.2} on {}, ledger {:.2}",
                preview.bank_balance, preview.date, preview.ledger_balance
            )),
        },
    )
    .await
}

#[tauri::command]
pub async fn preview_balance_reconciliation(
    db: tauri::State<'_, DatabaseConnection>,
    request: ReconcileRequest,
) -> Result<ReconcilePreview, AppError> {
    preview_balance_reconciliation_in(db.inner(), request).await
}

#[tauri::command]
pub async fn confirm_balance_reconciliation(
    db: tauri::State<'_, DatabaseConnection>,
    request: ReconcileRequest,
) -> Result<general_transaction::Model, AppError> {
    confirm_balance_reconciliation_in(db.inner(), request).await
}
