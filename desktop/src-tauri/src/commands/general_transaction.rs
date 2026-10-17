use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait, Unchanged,
};
use serde::{Deserialize, Serialize};

use crate::commands::loan::ensure_account_in_workspace;
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
    if existing.transfer_id.is_some() {
        return Err(AppError::InvalidInput(
            "a transfer cannot be edited: delete it and create it again".to_string(),
        ));
    }

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
    delete_general_transaction_in(db.inner(), general_transaction_id).await
}

/// Apagar uma das duas linhas de uma transferência apaga a outra junto: uma
/// perna sozinha desbalancearia o saldo das contas.
pub async fn delete_general_transaction_in(
    db: &DatabaseConnection,
    general_transaction_id: i32,
) -> Result<(), AppError> {
    let existing = general_transaction::Entity::find_by_id(general_transaction_id)
        .one(db)
        .await?;
    match existing.and_then(|tx| tx.transfer_id) {
        Some(transfer_id) => {
            general_transaction::Entity::delete_many()
                .filter(general_transaction::Column::TransferId.eq(transfer_id))
                .exec(db)
                .await?;
        }
        None => {
            general_transaction::Entity::delete_by_id(general_transaction_id)
                .exec(db)
                .await?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Transferência entre contas (Fase 19)
// ---------------------------------------------------------------------------

pub const TRANSFER_OUT: &str = "transferencia_saida";
pub const TRANSFER_IN: &str = "transferencia_entrada";

#[derive(Deserialize)]
pub struct CreateTransferRequest {
    pub workspace_id: i32,
    pub from_account_id: i32,
    pub to_account_id: i32,
    /// Quanto saiu, na moeda da conta de origem.
    pub valor_saida: f64,
    /// Quanto chegou, na moeda da conta de destino. Na mesma moeda tem de ser
    /// igual a `valor_saida`; em moedas diferentes é o que o extrato mostra,
    /// com spread e tarifa já embutidos.
    pub valor_entrada: f64,
    pub transaction_date: String,
    pub notes: Option<String>,
}

#[derive(Serialize)]
pub struct TransferResult {
    pub transfer_id: i32,
    pub from_currency: String,
    pub to_currency: String,
    /// Entrada ÷ saída; `None` quando as duas contas têm a mesma moeda.
    pub effective_rate: Option<f64>,
}

#[tauri::command]
pub async fn create_transfer(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateTransferRequest,
) -> Result<TransferResult, AppError> {
    create_transfer_in(db.inner(), request).await
}

pub async fn create_transfer_in(
    db: &DatabaseConnection,
    request: CreateTransferRequest,
) -> Result<TransferResult, AppError> {
    for (field, value) in [("valor_saida", request.valor_saida), ("valor_entrada", request.valor_entrada)] {
        if !value.is_finite() || value <= 0.0 {
            return Err(AppError::InvalidInput(format!("{field} must be positive")));
        }
    }
    NaiveDate::parse_from_str(&request.transaction_date, "%Y-%m-%d")
        .map_err(|e| AppError::InvalidInput(format!("transaction_date inválida: {e}")))?;
    if request.from_account_id == request.to_account_id {
        return Err(AppError::InvalidInput("choose two different accounts".to_string()));
    }

    let txn = db.begin().await?;
    let from = ensure_account_in_workspace(&txn, request.from_account_id, request.workspace_id).await?;
    let to = ensure_account_in_workspace(&txn, request.to_account_id, request.workspace_id).await?;
    if from.currency == to.currency && (request.valor_entrada - request.valor_saida).abs() > 1e-9 {
        return Err(AppError::InvalidInput(
            "accounts in the same currency must receive exactly what left".to_string(),
        ));
    }

    let leg = |account_id: i32, kind: &str, valor: f64| general_transaction::ActiveModel {
        workspace_id: Set(request.workspace_id),
        bank_account_id: Set(account_id),
        transaction_type: Set(kind.to_string()),
        category_id: Set(None),
        valor: Set(valor),
        transaction_date: Set(request.transaction_date.clone()),
        notes: Set(request.notes.clone()),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    };
    let out = leg(from.id, TRANSFER_OUT, request.valor_saida).insert(&txn).await?;
    let transfer_id = out.id;
    let mut out: general_transaction::ActiveModel = out.into();
    out.transfer_id = Set(Some(transfer_id));
    out.update(&txn).await?;
    let mut incoming = leg(to.id, TRANSFER_IN, request.valor_entrada);
    incoming.transfer_id = Set(Some(transfer_id));
    incoming.insert(&txn).await?;
    txn.commit().await?;

    let same_currency = from.currency == to.currency;
    Ok(TransferResult {
        transfer_id,
        effective_rate: (!same_currency).then(|| request.valor_entrada / request.valor_saida),
        from_currency: from.currency,
        to_currency: to.currency,
    })
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
