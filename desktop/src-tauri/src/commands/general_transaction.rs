use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    Unchanged,
};
use serde::Deserialize;

use crate::entity::{bank_account, general_transaction};
use crate::error::AppError;

const RECEITA: &str = "receita";
const DESPESA: &str = "despesa";
const GENERAL_TRANSACTION_TYPES: [&str; 2] = [RECEITA, DESPESA];

fn validate_type(transaction_type: &str) -> Result<(), AppError> {
    if !GENERAL_TRANSACTION_TYPES.contains(&transaction_type) {
        return Err(AppError::InvalidGuard(format!(
            "transaction_type '{transaction_type}' inválido (esperado um de {GENERAL_TRANSACTION_TYPES:?})"
        )));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct CreateGeneralTransactionRequest {
    pub workspace_id: i32,
    pub bank_account_id: i32,
    pub transaction_type: String,
    pub category_id: Option<i32>,
    pub valor: f64,
    pub transaction_date: String,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn create_general_transaction(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateGeneralTransactionRequest,
) -> Result<general_transaction::Model, AppError> {
    validate_type(&request.transaction_type)?;

    let tx = general_transaction::ActiveModel {
        workspace_id: Set(request.workspace_id),
        bank_account_id: Set(request.bank_account_id),
        transaction_type: Set(request.transaction_type),
        category_id: Set(request.category_id),
        valor: Set(request.valor),
        transaction_date: Set(request.transaction_date),
        notes: Set(request.notes),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?;

    Ok(tx)
}

#[derive(Deserialize)]
pub struct UpdateGeneralTransactionRequest {
    pub general_transaction_id: i32,
    pub bank_account_id: i32,
    pub transaction_type: String,
    pub category_id: Option<i32>,
    pub valor: f64,
    pub transaction_date: String,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn update_general_transaction(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateGeneralTransactionRequest,
) -> Result<general_transaction::Model, AppError> {
    update_general_transaction_in(db.inner(), request).await
}

pub async fn update_general_transaction_in(
    db: &DatabaseConnection,
    request: UpdateGeneralTransactionRequest,
) -> Result<general_transaction::Model, AppError> {
    validate_type(&request.transaction_type)?;

    let existing = general_transaction::Entity::find_by_id(request.general_transaction_id)
        .one(db)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "general transaction {}",
                request.general_transaction_id
            ))
        })?;

    // Fase 19 — o lançamento herda a moeda da conta; mudar de conta só é
    // possível entre contas da mesma moeda (não há conversão do valor).
    if request.bank_account_id != existing.bank_account_id {
        let currency_of = |id: i32| async move {
            bank_account::Entity::find_by_id(id)
                .one(db)
                .await?
                .map(|a| a.currency)
                .ok_or_else(|| AppError::NotFound(format!("bank account {id}")))
        };
        let (from, to) = (
            currency_of(existing.bank_account_id).await?,
            currency_of(request.bank_account_id).await?,
        );
        if from != to {
            return Err(AppError::InvalidInput(format!(
                "cannot move a transaction from a {from} account to a {to} account"
            )));
        }
    }

    let updated = general_transaction::ActiveModel {
        id: Unchanged(existing.id),
        workspace_id: Unchanged(existing.workspace_id),
        bank_account_id: Set(request.bank_account_id),
        transaction_type: Set(request.transaction_type),
        category_id: Set(request.category_id),
        valor: Set(request.valor),
        transaction_date: Set(request.transaction_date),
        notes: Set(request.notes),
        ..Default::default()
    }
    .update(db)
    .await?;

    Ok(updated)
}

#[tauri::command]
pub async fn delete_general_transaction(
    db: tauri::State<'_, DatabaseConnection>,
    general_transaction_id: i32,
) -> Result<(), AppError> {
    general_transaction::Entity::delete_by_id(general_transaction_id)
        .exec(db.inner())
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn list_general_transactions(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<general_transaction::Model>, AppError> {
    Ok(general_transaction::Entity::find()
        .filter(general_transaction::Column::WorkspaceId.eq(workspace_id))
        .order_by_desc(general_transaction::Column::TransactionDate)
        .all(db.inner())
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_type_accepts_receita_and_despesa() {
        assert!(validate_type("receita").is_ok());
        assert!(validate_type("despesa").is_ok());
    }

    #[test]
    fn validate_type_rejects_unknown_type() {
        assert!(validate_type("parcela_divida").is_err());
    }
}
