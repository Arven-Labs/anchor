use std::collections::{HashMap, HashSet};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, Unchanged,
};
use serde::{Deserialize, Serialize};

use crate::commands::currency::{rates_for, validate_currency, DEFAULT_BASE_CURRENCY};
use crate::entity::{bank_account, general_transaction, liability, loan, loan_repayment};
use crate::error::AppError;

#[derive(Deserialize)]
pub struct CreateBankAccountRequest {
    pub workspace_id: i32,
    pub nome: String,
    pub titular: String,
    /// Fase 19 — BRL se omitida.
    #[serde(default)]
    pub currency: Option<String>,
}

#[tauri::command]
pub async fn create_bank_account(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateBankAccountRequest,
) -> Result<bank_account::Model, AppError> {
    create_bank_account_in(db.inner(), request).await
}

pub async fn create_bank_account_in(
    db: &DatabaseConnection,
    request: CreateBankAccountRequest,
) -> Result<bank_account::Model, AppError> {
    let currency = match request.currency.as_deref() {
        Some(code) => validate_currency(code)?,
        None => DEFAULT_BASE_CURRENCY.to_string(),
    };
    let account = bank_account::ActiveModel {
        workspace_id: Set(request.workspace_id),
        nome: Set(request.nome),
        titular: Set(request.titular),
        currency: Set(currency),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db)
    .await?;

    Ok(account)
}

#[derive(Deserialize)]
pub struct UpdateBankAccountRequest {
    pub bank_account_id: i32,
    pub nome: String,
    pub titular: String,
    /// Fase 19 — omitida = não muda. Trocar só é aceito enquanto a conta não
    /// tem uso (lançamento, dívida, empréstimo ou baixa); senão o histórico
    /// seria reinterpretado em outra moeda.
    #[serde(default)]
    pub currency: Option<String>,
}

#[tauri::command]
pub async fn update_bank_account(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateBankAccountRequest,
) -> Result<bank_account::Model, AppError> {
    update_bank_account_in(db.inner(), request).await
}

pub async fn update_bank_account_in(
    db: &DatabaseConnection,
    request: UpdateBankAccountRequest,
) -> Result<bank_account::Model, AppError> {
    let existing = bank_account::Entity::find_by_id(request.bank_account_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("bank account {}", request.bank_account_id)))?;

    let currency = match request.currency.as_deref() {
        Some(code) => validate_currency(code)?,
        None => existing.currency.clone(),
    };
    if currency != existing.currency && account_in_use(db, existing.id).await? {
        return Err(AppError::InvalidInput(
            "the currency of an account with transactions, debts or loans cannot change".to_string(),
        ));
    }

    let updated = bank_account::ActiveModel {
        id: Unchanged(existing.id),
        workspace_id: Unchanged(existing.workspace_id),
        nome: Set(request.nome),
        titular: Set(request.titular),
        currency: Set(currency),
        ..Default::default()
    }
    .update(db)
    .await?;

    Ok(updated)
}

/// Conta com lançamento, dívida, empréstimo ou baixa apontando pra ela.
pub async fn account_in_use(db: &DatabaseConnection, bank_account_id: i32) -> Result<bool, AppError> {
    let has_transactions = general_transaction::Entity::find()
        .filter(general_transaction::Column::BankAccountId.eq(bank_account_id))
        .one(db)
        .await?
        .is_some();
    let has_liabilities = liability::Entity::find()
        .filter(liability::Column::BankAccountId.eq(bank_account_id))
        .one(db)
        .await?
        .is_some();
    let has_loans = loan::Entity::find()
        .filter(loan::Column::BankAccountId.eq(bank_account_id))
        .one(db)
        .await?
        .is_some();
    let has_repayments = loan_repayment::Entity::find()
        .filter(loan_repayment::Column::BankAccountId.eq(bank_account_id))
        .one(db)
        .await?
        .is_some();
    Ok(has_transactions || has_liabilities || has_loans || has_repayments)
}

#[tauri::command]
pub async fn delete_bank_account(
    db: tauri::State<'_, DatabaseConnection>,
    bank_account_id: i32,
) -> Result<(), AppError> {
    delete_bank_account_in(db.inner(), bank_account_id).await
}

pub async fn delete_bank_account_in(
    db: &DatabaseConnection,
    bank_account_id: i32,
) -> Result<(), AppError> {
    // Fase 19 — apagar a conta apaga as pernas das transferências dela, e a
    // perna oposta, na outra conta, ficaria como dinheiro que apareceu do nada.
    let transfer_ids: Vec<i32> = general_transaction::Entity::find()
        .filter(general_transaction::Column::BankAccountId.eq(bank_account_id))
        .filter(general_transaction::Column::TransferId.is_not_null())
        .all(db)
        .await?
        .into_iter()
        .filter_map(|tx| tx.transfer_id)
        .collect();
    if !transfer_ids.is_empty() {
        general_transaction::Entity::delete_many()
            .filter(general_transaction::Column::TransferId.is_in(transfer_ids))
            .exec(db)
            .await?;
    }

    bank_account::Entity::delete_by_id(bank_account_id).exec(db).await?;

    Ok(())
}

#[derive(Serialize)]
pub struct BankAccountView {
    pub id: i32,
    pub workspace_id: i32,
    pub nome: String,
    pub titular: String,
    pub created_at: String,
    pub currency: String,
    /// Saldo na moeda da conta.
    pub balance: f64,
    /// Saldo na moeda base do workspace; `None` sem cotação do par.
    pub balance_in_base: Option<f64>,
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
            "despesa" | "parcela_divida" | "emprestimo_concedido" | "transferencia_saida" => -tx.valor,
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
) -> Result<TotalCash, AppError> {
    let accounts = list_bank_accounts_view(db, workspace_id).await?;
    let mut total_in_base = 0.0;
    let mut accounts_missing_fx = 0;
    for account in &accounts {
        match account.balance_in_base {
            Some(value) => total_in_base += value,
            None => accounts_missing_fx += 1,
        }
    }
    Ok(TotalCash { total_in_base, accounts_missing_fx })
}

/// Caixa do workspace na moeda base. Conta sem cotação do par fica fora da
/// soma e é contada, pra a tela avisar que o total está incompleto.
pub struct TotalCash {
    pub total_in_base: f64,
    pub accounts_missing_fx: usize,
}

#[tauri::command]
pub async fn list_bank_accounts(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<BankAccountView>, AppError> {
    list_bank_accounts_view(db.inner(), workspace_id).await
}

pub async fn list_bank_accounts_view(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Vec<BankAccountView>, AppError> {
    let accounts = bank_account::Entity::find()
        .filter(bank_account::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;

    let transactions = general_transaction::Entity::find()
        .filter(general_transaction::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;

    let hoje = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let balances = compute_balances(&transactions, &hoje);
    let currencies: HashSet<String> = accounts.iter().map(|a| a.currency.clone()).collect();
    let rates = rates_for(db, workspace_id, &currencies).await?;

    Ok(accounts
        .into_iter()
        .map(|account| {
            let balance = balances.get(&account.id).copied().unwrap_or(0.0);
            BankAccountView {
                balance_in_base: rates.convert(balance, &account.currency),
                balance,
                id: account.id,
                workspace_id: account.workspace_id,
                nome: account.nome,
                titular: account.titular,
                created_at: account.created_at,
                currency: account.currency,
            }
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
            transfer_id: None,
        }
    }

    #[test]
    fn compute_balances_moves_money_between_accounts_on_a_transfer() {
        let transactions = vec![
            tx(1, "receita", 1000.0),
            tx(1, "transferencia_saida", 400.0),
            tx(2, "transferencia_entrada", 75.0),
        ];
        let balances = compute_balances(&transactions, "2026-08-09");
        assert_eq!(balances.get(&1), Some(&600.0));
        assert_eq!(balances.get(&2), Some(&75.0));
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
