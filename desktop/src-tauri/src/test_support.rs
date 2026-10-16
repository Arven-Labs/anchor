// Apoio dos testes de fluxo: um banco SQLite em memória com o schema **real**
// (as mesmas migrations do app) e construtores das linhas que os fluxos
// precisam. Só compila em teste.

use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set};

use crate::entity::{assets, bank_account, general_transaction, portfolio, transactions, workspace};

/// Banco em memória já migrado. **Uma conexão só**: com `sqlite::memory:` cada
/// conexão do pool enxerga um banco vazio e diferente, e as tabelas criadas
/// pela migration sumiriam nas consultas seguintes.
pub async fn memory_db() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.expect("connect to in-memory sqlite");
    crate::db::migrate(&db).await.expect("run migrations on the in-memory database");
    db
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub async fn workspace(db: &DatabaseConnection, name: &str) -> workspace::Model {
    workspace::ActiveModel {
        name: Set(name.to_string()),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert workspace")
}

pub async fn portfolio(db: &DatabaseConnection, workspace_id: i32, name: &str) -> portfolio::Model {
    portfolio::ActiveModel {
        workspace_id: Set(workspace_id),
        name: Set(name.to_string()),
        description: Set(None),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert portfolio")
}

pub async fn bank_account(db: &DatabaseConnection, workspace_id: i32, nome: &str) -> bank_account::Model {
    bank_account::ActiveModel {
        workspace_id: Set(workspace_id),
        nome: Set(nome.to_string()),
        titular: Set("Test holder".to_string()),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert bank account")
}

pub async fn asset(db: &DatabaseConnection, ticker: &str, asset_class: &str) -> assets::Model {
    assets::ActiveModel {
        ticker: Set(ticker.to_string()),
        name: Set(ticker.to_string()),
        asset_class: Set(asset_class.to_string()),
        currency: Set("BRL".to_string()),
        exchange: Set(None),
        exposure_type: Set("pais".to_string()),
        exposure_value: Set("BR".to_string()),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert asset")
}

/// Compra (`"compra"`) ou venda (`"venda"`) de um ativo.
pub async fn trade(
    db: &DatabaseConnection,
    portfolio_id: i32,
    asset_id: i32,
    transaction_type: &str,
    quantity: f64,
    total_value: f64,
    date: &str,
) -> transactions::Model {
    transactions::ActiveModel {
        portfolio_id: Set(portfolio_id),
        asset_id: Set(Some(asset_id)),
        transaction_type: Set(transaction_type.to_string()),
        quantity: Set(Some(quantity)),
        unit_price: Set(Some(total_value / quantity)),
        total_value: Set(total_value),
        fee: Set(Some(0.0)),
        transaction_date: Set(date.to_string()),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert trade")
}

/// Entrada de caixa (`receita`) numa conta.
pub async fn cash_in(
    db: &DatabaseConnection,
    workspace_id: i32,
    bank_account_id: i32,
    valor: f64,
    date: &str,
) -> general_transaction::Model {
    general_transaction::ActiveModel {
        workspace_id: Set(workspace_id),
        bank_account_id: Set(bank_account_id),
        transaction_type: Set("receita".to_string()),
        category_id: Set(None),
        valor: Set(valor),
        transaction_date: Set(date.to_string()),
        notes: Set(None),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert cash transaction")
}
