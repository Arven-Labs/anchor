use std::collections::{HashMap, HashSet};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set, Unchanged,
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
        mirrored_in_portfolio: Set(false),
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
    /// Omitida = não muda.
    #[serde(default)]
    pub mirrored_in_portfolio: Option<bool>,
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
        mirrored_in_portfolio: Set(request.mirrored_in_portfolio.unwrap_or(existing.mirrored_in_portfolio)),
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
    /// Tem lançamento, dívida, empréstimo ou baixa: a moeda não pode mais mudar.
    pub in_use: bool,
    /// O saldo já está na carteira (renda fixa importada): o patrimônio não o conta de novo no caixa.
    pub mirrored_in_portfolio: bool,
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
/// Efeito do lançamento no saldo da conta: saída negativa, o resto positivo.
pub fn signed_valor(transaction_type: &str, valor: f64) -> f64 {
    match transaction_type {
        "despesa" | "parcela_divida" | "emprestimo_concedido" | "transferencia_saida" => -valor,
        _ => valor,
    }
}

fn compute_balances(
    transactions: &[general_transaction::Model],
    hoje: &str,
) -> HashMap<i32, f64> {
    let mut balances: HashMap<i32, f64> = HashMap::new();
    for tx in transactions {
        if tx.transaction_date.as_str() > hoje {
            continue;
        }
        *balances.entry(tx.bank_account_id).or_insert(0.0) += signed_valor(&tx.transaction_type, tx.valor);
    }
    balances
}

/// Saldo de uma conta no fim de `date` (`YYYY-MM-DD`), na moeda dela: só os lançamentos até essa data.
pub async fn balance_on(db: &DatabaseConnection, bank_account_id: i32, date: &str) -> Result<f64, AppError> {
    let transactions = general_transaction::Entity::find()
        .filter(general_transaction::Column::BankAccountId.eq(bank_account_id))
        .all(db)
        .await?;
    Ok(compute_balances(&transactions, date).get(&bank_account_id).copied().unwrap_or(0.0))
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
    let mut mirrored_balances_in_base = Vec::new();
    for account in &accounts {
        match account.balance_in_base {
            Some(value) => {
                total_in_base += value;
                if account.mirrored_in_portfolio {
                    mirrored_balances_in_base.push(value);
                }
            }
            None => accounts_missing_fx += 1,
        }
    }
    Ok(TotalCash { total_in_base, accounts_missing_fx, mirrored_balances_in_base })
}

/// Caixa do workspace na moeda base. Conta sem cotação do par fica fora da
/// soma e é contada, pra a tela avisar que o total está incompleto.
pub struct TotalCash {
    pub total_in_base: f64,
    pub accounts_missing_fx: usize,
    /// Saldo (na base) das contas marcadas como "já na carteira", em ordem de id; já está em `total_in_base`.
    pub mirrored_balances_in_base: Vec<f64>,
}

/// Quanto do caixa já está na carteira (Fase 35, entrega 8): o valor da renda fixa importada é repartido
/// entre as contas marcadas, na ordem dada, e cada conta cede no máximo o próprio saldo (nunca negativo).
/// Cobertura parcial da B3 não subconta: o que passa do valor da carteira segue como caixa. Com uma conta
/// marcada, a conta é exata; com várias, a ordem é uma convenção.
pub fn mirrored_cash_deduction(mirrored_balances: &[f64], portfolio_fixed_income_value: f64) -> f64 {
    let mut remaining = portfolio_fixed_income_value.max(0.0);
    let mut deduction = 0.0;
    for balance in mirrored_balances {
        let take = balance.max(0.0).min(remaining);
        deduction += take;
        remaining -= take;
    }
    deduction
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
        .order_by_asc(bank_account::Column::Id)
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

    let mut views = Vec::with_capacity(accounts.len());
    for account in accounts {
        let balance = balances.get(&account.id).copied().unwrap_or(0.0);
        views.push(BankAccountView {
            balance_in_base: rates.convert(balance, &account.currency),
            in_use: account_in_use(db, account.id).await?,
            balance,
            id: account.id,
            workspace_id: account.workspace_id,
            nome: account.nome,
            titular: account.titular,
            created_at: account.created_at,
            currency: account.currency,
            mirrored_in_portfolio: account.mirrored_in_portfolio,
        });
    }
    Ok(views)
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
    fn the_mirrored_deduction_never_exceeds_the_portfolio_value_or_the_balance() {
        // Carteira cobre só parte do saldo: só essa parte sai do caixa.
        assert_eq!(mirrored_cash_deduction(&[1000.0], 400.0), 400.0);
        // Carteira passa do saldo: a conta cede o saldo inteiro, nunca mais (o caixa não fica negativo).
        assert_eq!(mirrored_cash_deduction(&[300.0], 1000.0), 300.0);
        // Sem conta marcada ou sem renda fixa na carteira, nada sai.
        assert_eq!(mirrored_cash_deduction(&[], 1000.0), 0.0);
        assert_eq!(mirrored_cash_deduction(&[500.0], 0.0), 0.0);
        // Saldo negativo não gera dedução negativa.
        assert_eq!(mirrored_cash_deduction(&[-50.0], 100.0), 0.0);
    }

    #[test]
    fn the_mirrored_deduction_is_shared_between_marked_accounts_in_order() {
        assert_eq!(mirrored_cash_deduction(&[300.0, 500.0], 600.0), 600.0);
        assert_eq!(mirrored_cash_deduction(&[300.0, 500.0], 2000.0), 800.0);
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
