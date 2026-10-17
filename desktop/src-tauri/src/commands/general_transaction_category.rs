use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, Unchanged,
};
use serde::{Deserialize, Serialize};

use crate::commands::currency::{base_currency, load_fx_history, DEFAULT_BASE_CURRENCY};
use crate::domain::currency_conversion::FxHistory;
use crate::entity::{bank_account, general_transaction, general_transaction_category};
use crate::error::AppError;

#[derive(Deserialize)]
pub struct CreateGeneralTransactionCategoryRequest {
    pub workspace_id: i32,
    pub nome: String,
    pub limite_mensal: Option<f64>,
}

#[tauri::command]
pub async fn create_general_transaction_category(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateGeneralTransactionCategoryRequest,
) -> Result<general_transaction_category::Model, AppError> {
    let category = general_transaction_category::ActiveModel {
        workspace_id: Set(request.workspace_id),
        nome: Set(request.nome),
        limite_mensal: Set(request.limite_mensal),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?;

    Ok(category)
}

#[derive(Deserialize)]
pub struct UpdateGeneralTransactionCategoryRequest {
    pub general_transaction_category_id: i32,
    pub nome: String,
    pub limite_mensal: Option<f64>,
}

#[tauri::command]
pub async fn update_general_transaction_category(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateGeneralTransactionCategoryRequest,
) -> Result<general_transaction_category::Model, AppError> {
    let existing =
        general_transaction_category::Entity::find_by_id(request.general_transaction_category_id)
            .one(db.inner())
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "general transaction category {}",
                    request.general_transaction_category_id
                ))
            })?;

    let updated = general_transaction_category::ActiveModel {
        id: Unchanged(existing.id),
        workspace_id: Unchanged(existing.workspace_id),
        nome: Set(request.nome),
        limite_mensal: Set(request.limite_mensal),
        ..Default::default()
    }
    .update(db.inner())
    .await?;

    Ok(updated)
}

#[tauri::command]
pub async fn delete_general_transaction_category(
    db: tauri::State<'_, DatabaseConnection>,
    general_transaction_category_id: i32,
) -> Result<(), AppError> {
    general_transaction_category::Entity::delete_by_id(general_transaction_category_id)
        .exec(db.inner())
        .await?;

    Ok(())
}

#[derive(Serialize)]
pub struct GeneralTransactionCategoryView {
    pub id: i32,
    pub workspace_id: i32,
    pub nome: String,
    pub limite_mensal: Option<f64>,
    pub created_at: String,
    /// Fase 19 — gasto do mês na moeda base do workspace, que é também a moeda
    /// do `limite_mensal`.
    pub spent_this_month: f64,
    pub base_currency: String,
    /// Despesas do mês deixadas de fora do gasto por falta de rota de câmbio.
    pub spent_missing_fx: usize,
}

/// Soma o `valor` das transações `despesa` daquela categoria cujo
/// `transaction_date` cai no mês `month_prefix` (`"YYYY-MM"`) — limite de
/// orçamento é sobre gasto, não sobre `receita`/`parcela_divida`. Cada despesa
/// entra na moeda base, pela moeda da conta e a taxa da data dela; sem rota de
/// câmbio fica fora da soma e é contada. Função pura (sem I/O), mesmo espírito
/// de `compute_balances` (`commands/bank_account.rs`).
fn compute_spent_this_month(
    transactions: &[general_transaction::Model],
    category_id: i32,
    month_prefix: &str,
    account_currency: &HashMap<i32, String>,
    base: &str,
    history: &FxHistory,
) -> (f64, usize) {
    let mut spent = 0.0;
    let mut missing = 0;
    for tx in transactions.iter().filter(|tx| {
        tx.transaction_type == "despesa"
            && tx.category_id == Some(category_id)
            && tx.transaction_date.starts_with(month_prefix)
    }) {
        let currency = account_currency.get(&tx.bank_account_id).map_or(DEFAULT_BASE_CURRENCY, String::as_str);
        let rate = NaiveDate::parse_from_str(&tx.transaction_date, "%Y-%m-%d")
            .ok()
            .and_then(|date| history.rate_on(currency, base, date));
        match rate {
            Some(rate) => spent += tx.valor * rate,
            None => missing += 1,
        }
    }
    (spent, missing)
}

#[tauri::command]
pub async fn list_general_transaction_categories(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<GeneralTransactionCategoryView>, AppError> {
    list_categories_view(db.inner(), workspace_id).await
}

pub async fn list_categories_view(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Vec<GeneralTransactionCategoryView>, AppError> {
    let categories = general_transaction_category::Entity::find()
        .filter(general_transaction_category::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;

    let transactions = general_transaction::Entity::find()
        .filter(general_transaction::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;

    let account_currency: HashMap<i32, String> = bank_account::Entity::find()
        .filter(bank_account::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?
        .into_iter()
        .map(|a| (a.id, a.currency))
        .collect();
    let base = base_currency(db, workspace_id).await?;
    let currencies: HashSet<String> = account_currency.values().cloned().collect();
    let history = load_fx_history(db, &base, &currencies).await?;

    let month_prefix = chrono::Utc::now().format("%Y-%m").to_string();

    Ok(categories
        .into_iter()
        .map(|category| {
            let (spent_this_month, spent_missing_fx) = compute_spent_this_month(
                &transactions,
                category.id,
                &month_prefix,
                &account_currency,
                &base,
                &history,
            );
            GeneralTransactionCategoryView {
                spent_this_month,
                spent_missing_fx,
                base_currency: base.clone(),
                id: category.id,
                workspace_id: category.workspace_id,
                nome: category.nome,
                limite_mensal: category.limite_mensal,
                created_at: category.created_at,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tx(
        category_id: Option<i32>,
        transaction_type: &str,
        valor: f64,
        date: &str,
    ) -> general_transaction::Model {
        general_transaction::Model {
            id: 0,
            workspace_id: 1,
            bank_account_id: 1,
            transaction_type: transaction_type.to_string(),
            category_id,
            valor,
            transaction_date: date.to_string(),
            notes: None,
            created_at: "2026-08-11T00:00:00Z".to_string(),
            transfer_id: None,
        }
    }

    // Conta 1 em BRL com base BRL: a taxa é 1 sem precisar de histórico de câmbio.
    fn spent(transactions: &[general_transaction::Model], category_id: i32, month: &str) -> f64 {
        let accounts = HashMap::from([(1, "BRL".to_string())]);
        compute_spent_this_month(
            transactions,
            category_id,
            month,
            &accounts,
            "BRL",
            &FxHistory::new(HashMap::new()),
        )
        .0
    }

    #[test]
    fn compute_spent_this_month_converts_each_expense_at_its_own_date() {
        let history = FxHistory::new(HashMap::from([(
            "USDBRL".to_string(),
            vec![
                (NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(), 5.0),
                (NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(), 6.0),
            ],
        )]));
        let accounts = HashMap::from([(1, "BRL".to_string()), (2, "USD".to_string())]);
        let mut usd_early = tx(Some(1), "despesa", 10.0, "2026-08-02");
        usd_early.bank_account_id = 2;
        let mut usd_late = tx(Some(1), "despesa", 10.0, "2026-08-19");
        usd_late.bank_account_id = 2;
        let transactions = vec![tx(Some(1), "despesa", 100.0, "2026-08-05"), usd_early, usd_late];

        let (total, missing) =
            compute_spent_this_month(&transactions, 1, "2026-08", &accounts, "BRL", &history);
        assert_eq!(missing, 0);
        assert!((total - (100.0 + 10.0 * 5.0 + 10.0 * 6.0)).abs() < 1e-9);
    }

    #[test]
    fn compute_spent_this_month_leaves_out_and_counts_an_expense_without_a_rate() {
        let accounts = HashMap::from([(1, "BRL".to_string()), (2, "USD".to_string())]);
        let mut usd = tx(Some(1), "despesa", 10.0, "2026-08-02");
        usd.bank_account_id = 2;
        let transactions = vec![tx(Some(1), "despesa", 100.0, "2026-08-05"), usd];

        let (total, missing) = compute_spent_this_month(
            &transactions,
            1,
            "2026-08",
            &accounts,
            "BRL",
            &FxHistory::new(HashMap::new()),
        );
        assert_eq!((total, missing), (100.0, 1));
    }

    #[test]
    fn compute_spent_this_month_is_zero_for_no_transactions() {
        assert_eq!(spent(&[], 1, "2026-08"), 0.0);
    }

    #[test]
    fn compute_spent_this_month_sums_despesa_for_category_and_month() {
        let transactions = vec![
            tx(Some(1), "despesa", 100.0, "2026-08-05"),
            tx(Some(1), "despesa", 50.0, "2026-08-20"),
        ];
        assert_eq!(spent(&transactions, 1, "2026-08"), 150.0);
    }

    #[test]
    fn compute_spent_this_month_ignores_receita() {
        let transactions = vec![tx(Some(1), "receita", 100.0, "2026-08-05")];
        assert_eq!(spent(&transactions, 1, "2026-08"), 0.0);
    }

    #[test]
    fn compute_spent_this_month_ignores_other_category() {
        let transactions = vec![tx(Some(2), "despesa", 100.0, "2026-08-05")];
        assert_eq!(spent(&transactions, 1, "2026-08"), 0.0);
    }

    #[test]
    fn compute_spent_this_month_ignores_other_month() {
        let transactions = vec![tx(Some(1), "despesa", 100.0, "2026-07-31")];
        assert_eq!(spent(&transactions, 1, "2026-08"), 0.0);
    }
}
