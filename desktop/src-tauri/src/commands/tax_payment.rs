// Pagamento do DARF (Fase 24). O cálculo do imposto fica em `tax`; marcar como pago
// é lançar uma despesa numa conta bancária, então mora nas Finanças (ADR 0007).

use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set, TransactionTrait};
use serde::Deserialize;

use crate::commands::general_transaction::DESPESA;
use crate::commands::general_transaction_category::ensure_taxes_category;
use crate::commands::tax::{parse_date, PAYMENT_KIND_B3, PAYMENT_KIND_CRYPTO};
use crate::entity::{bank_account, general_transaction, tax_payments};
use crate::error::AppError;

#[derive(Deserialize)]
pub struct MarkTaxPaidRequest {
    pub workspace_id: i32,
    pub period: String,
    pub amount: f64,
    pub paid_date: String,
    pub bank_account_id: i32,
    /// `b3` (padrão) ou `crypto`.
    #[serde(default = "default_payment_kind")]
    pub kind: String,
}

fn default_payment_kind() -> String {
    PAYMENT_KIND_B3.to_string()
}

#[tauri::command]
pub async fn mark_tax_paid(
    db: tauri::State<'_, DatabaseConnection>,
    request: MarkTaxPaidRequest,
) -> Result<tax_payments::Model, AppError> {
    mark_tax_paid_in(db.inner(), request).await
}

pub async fn mark_tax_paid_in(
    db: &DatabaseConnection,
    request: MarkTaxPaidRequest,
) -> Result<tax_payments::Model, AppError> {
    if request.amount <= 0.0 || !request.amount.is_finite() {
        return Err(AppError::InvalidInput("amount must be positive".to_string()));
    }
    parse_date(&request.paid_date)?;
    parse_date(&format!("{}-01", request.period))?;
    let note_kind = match request.kind.as_str() {
        PAYMENT_KIND_B3 => "stock exchange",
        PAYMENT_KIND_CRYPTO => "crypto",
        other => return Err(AppError::InvalidInput(format!("payment kind must be b3 or crypto, got '{other}'"))),
    };

    let account = bank_account::Entity::find_by_id(request.bank_account_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("bank account {}", request.bank_account_id)))?;
    if account.workspace_id != request.workspace_id {
        return Err(AppError::InvalidGuard("bank account belongs to another workspace".to_string()));
    }
    // O DARF é sempre em reais; numa conta em outra moeda o débito sairia no
    // valor errado.
    if account.currency == "BRL" {
        return Err(AppError::InvalidInput(format!(
            "the tax is paid in BRL, but the account is in {}",
            account.currency
        )));
    }

    let now = Utc::now().to_rfc3339();
    let txn = db.begin().await?;

    let category_id = ensure_taxes_category(&txn, request.workspace_id).await?;

    let expense = general_transaction::ActiveModel {
        workspace_id: Set(request.workspace_id),
        bank_account_id: Set(request.bank_account_id),
        transaction_type: Set(DESPESA.to_string()),
        category_id: Set(Some(category_id)),
        valor: Set(request.amount),
        transaction_date: Set(request.paid_date.clone()),
        notes: Set(Some(format!("DARF — capital gains tax ({note_kind}), period {}", request.period))),
        created_at: Set(now.clone()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    let payment = tax_payments::ActiveModel {
        workspace_id: Set(request.workspace_id),
        period: Set(request.period),
        amount: Set(request.amount),
        paid_date: Set(request.paid_date),
        kind: Set(request.kind),
        general_transaction_id: Set(Some(expense.id)),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;
    Ok(payment)
}

/// Desfaz o "pago": apaga o registro e a despesa criada junto em Finanças Gerais.
#[tauri::command]
pub async fn unmark_tax_paid(
    db: tauri::State<'_, DatabaseConnection>,
    payment_id: i32,
) -> Result<(), AppError> {
    let db = db.inner();
    let payment = tax_payments::Entity::find_by_id(payment_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("tax payment {payment_id}")))?;

    let txn = db.begin().await?;
    tax_payments::Entity::delete_by_id(payment.id).exec(&txn).await?;
    if let Some(gt_id) = payment.general_transaction_id {
        general_transaction::Entity::delete_by_id(gt_id).exec(&txn).await?;
    }
    txn.commit().await?;
    Ok(())
}
