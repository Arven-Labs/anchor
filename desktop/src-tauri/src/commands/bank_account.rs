use std::collections::HashMap;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, Unchanged,
};
use serde::{Deserialize, Serialize};

use crate::entity::{bank_account, general_transaction};
use crate::error::AppError;

#[derive(Deserialize)]
pub struct CreateBankAccountRequest {
    pub workspace_id: i32,
    pub nome: String,
    pub titular: String,
}

#[tauri::command]
pub async fn create_bank_account(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateBankAccountRequest,
) -> Result<bank_account::Model, AppError> {
    let account = bank_account::ActiveModel {
        workspace_id: Set(request.workspace_id),
        nome: Set(request.nome),
        titular: Set(request.titular),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?;

    Ok(account)
}

#[derive(Deserialize)]
pub struct UpdateBankAccountRequest {
    pub bank_account_id: i32,
    pub nome: String,
    pub titular: String,
}

#[tauri::command]
pub async fn update_bank_account(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateBankAccountRequest,
) -> Result<bank_account::Model, AppError> {
    let existing = bank_account::Entity::find_by_id(request.bank_account_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("bank account {}", request.bank_account_id)))?;

    let updated = bank_account::ActiveModel {
        id: Unchanged(existing.id),
        workspace_id: Unchanged(existing.workspace_id),
        nome: Set(request.nome),
        titular: Set(request.titular),
        ..Default::default()
    }
    .update(db.inner())
    .await?;

    Ok(updated)
}

#[tauri::command]
pub async fn delete_bank_account(
    db: tauri::State<'_, DatabaseConnection>,
    bank_account_id: i32,
) -> Result<(), AppError> {
    bank_account::Entity::delete_by_id(bank_account_id)
        .exec(db.inner())
        .await?;

    Ok(())
}

#[derive(Serialize)]
pub struct BankAccountView {
    pub id: i32,
    pub workspace_id: i32,
    pub nome: String,
    pub titular: String,
    pub created_at: String,
    pub balance: f64,
}

/// Saldo é sempre derivado, nunca gravado — soma de `receita` menos soma de
/// `despesa`/`parcela_divida`/`emprestimo_concedido` por conta, mesmo espírito do cálculo em
/// memória de `get_portfolio_positions` (`commands/transaction.rs`). Função
/// pura (sem I/O) para ficar testável isoladamente.
///
/// Só conta transações com `transaction_date <= hoje` — achado da Fase 12
/// (`Liability`): parcelas de dívida são geradas **todas de uma vez**, com
/// datas futuras, na hora em que a dívida é cadastrada. Sem esse filtro, o
/// saldo da conta já apareceria descontado de parcelas que ainda nem
/// venceram.
fn compute_balances(
    transactions: &[general_transaction::Model],
    hoje: &str,
) -> HashMap<i32, f64> {
    let mut balances: HashMap<i32, f64> = HashMap::new();
    for tx in transactions {
        if tx.transaction_date.as_str() > hoje {
            continue;
        }
        let signed_value = match tx.transaction_type.as_str() {
            "despesa" | "parcela_divida" | "emprestimo_concedido" => -tx.valor,
            _ => tx.valor,
        };
        *balances.entry(tx.bank_account_id).or_insert(0.0) += signed_value;
    }
    balances
}

/// Soma o saldo (já derivado por `compute_balances`) de todas as contas do
/// workspace — reaproveitado por `commands::net_worth::get_net_worth_summary`
/// (Fase 12), sem duplicar a query/filtro de data que `list_bank_accounts`
/// já faz.
pub async fn compute_total_cash(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<f64, AppError> {
    let transactions = general_transaction::Entity::find()
        .filter(general_transaction::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;

    let hoje = chrono::Utc::now().format("%Y-%m-%d").to_string();
    Ok(compute_balances(&transactions, &hoje).values().sum())
}

#[tauri::command]
pub async fn list_bank_accounts(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<BankAccountView>, AppError> {
    let accounts = bank_account::Entity::find()
        .filter(bank_account::Column::WorkspaceId.eq(workspace_id))
        .all(db.inner())
        .await?;

    let transactions = general_transaction::Entity::find()
        .filter(general_transaction::Column::WorkspaceId.eq(workspace_id))
        .all(db.inner())
        .await?;

    let hoje = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let balances = compute_balances(&transactions, &hoje);

    Ok(accounts
        .into_iter()
        .map(|account| BankAccountView {
            balance: balances.get(&account.id).copied().unwrap_or(0.0),
            id: account.id,
            workspace_id: account.workspace_id,
            nome: account.nome,
            titular: account.titular,
            created_at: account.created_at,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tx(bank_account_id: i32, transaction_type: &str, valor: f64) -> general_transaction::Model {
        general_transaction::Model {
            id: 0,
            workspace_id: 1,
            bank_account_id,
            transaction_type: transaction_type.to_string(),
            category_id: None,
            valor,
            transaction_date: "2026-08-09".to_string(),
            notes: None,
            created_at: "2026-08-09T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn compute_balances_is_empty_for_no_transactions() {
        assert!(compute_balances(&[], "2026-08-09").is_empty());
    }

    #[test]
    fn compute_balances_sums_receita_and_subtracts_despesa() {
        let transactions = vec![tx(1, "receita", 1000.0), tx(1, "despesa", 300.0)];
        let balances = compute_balances(&transactions, "2026-08-09");
        assert_eq!(balances.get(&1), Some(&700.0));
    }

    #[test]
    fn compute_balances_keeps_accounts_separate() {
        let transactions = vec![tx(1, "receita", 500.0), tx(2, "receita", 200.0)];
        let balances = compute_balances(&transactions, "2026-08-09");
        assert_eq!(balances.get(&1), Some(&500.0));
        assert_eq!(balances.get(&2), Some(&200.0));
    }

    #[test]
    fn compute_balances_subtracts_parcela_divida_like_despesa() {
        let transactions = vec![tx(1, "receita", 1000.0), tx(1, "parcela_divida", 250.0)];
        let balances = compute_balances(&transactions, "2026-08-09");
        assert_eq!(balances.get(&1), Some(&750.0));
    }

    #[test]
    fn compute_balances_loan_out_debits_and_repayment_credits() {
        let transactions = vec![
            tx(1, "receita", 1000.0),
            tx(1, "emprestimo_concedido", 400.0),
            tx(1, "recebimento_emprestimo", 150.0),
        ];
        let balances = compute_balances(&transactions, "2026-08-09");
        assert_eq!(balances.get(&1), Some(&750.0));
    }

    #[test]
    fn compute_balances_ignores_future_transactions() {
        let mut future = tx(1, "parcela_divida", 250.0);
        future.transaction_date = "2026-12-01".to_string();
        let transactions = vec![tx(1, "receita", 1000.0), future];
        let balances = compute_balances(&transactions, "2026-08-09");
        assert_eq!(balances.get(&1), Some(&1000.0));
    }
}
