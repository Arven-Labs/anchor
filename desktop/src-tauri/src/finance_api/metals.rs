// Fase 14.4 (fatia 3) — porta `collect_metal_ticker` (Sessão 55) pra Rust.
// Mesmo papel que `finance_api::crypto::collect_ticker` cumpre pra cripto:
// grava nas mesmas tabelas genéricas `stock_quotes`/`stock_price_history`
// que todo o resto do app já lê — sem Fear & Greed aqui (é coisa cripto).
use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, Set};

use crate::entity::{stock_price_history, stock_quotes};
use crate::error::AppError;
use crate::finance_api::{client, insert_ignoring_conflicts};

pub struct MetalTickerResult {
    pub ticker: String,
    pub name: String,
    pub price: f64,
    pub history_points: usize,
}

pub async fn collect_ticker(
    db: &DatabaseConnection,
    ticker: &str,
) -> Result<MetalTickerResult, AppError> {
    let quote = client::fetch_metal_quote(ticker).await?;
    let history = client::fetch_metal_price_history(ticker).await?;
    let now = Utc::now().to_rfc3339();

    // Grava com o ticker original (ex.: "XAU", maiúsculo — convenção do
    // frontend), não `quote.metal_code`/`history.metal_code` (o catálogo da
    // Finance API usa código minúsculo na URL, ver `client::fetch_metal_quote`).
    stock_quotes::ActiveModel {
        id: ActiveValue::NotSet,
        ticker: Set(ticker.to_string()),
        price: Set(quote.price),
        source: Set("yahoo_finance".to_string()),
        fetched_at: Set(now.clone()),
        name: Set(Some(quote.name.clone())),
        exchange: Set(Some("COMEX".to_string())),
        currency: Set(Some("USD".to_string())),
    }
    .insert(db)
    .await?;

    let history_points = history.data.len();
    let models: Vec<stock_price_history::ActiveModel> = history
        .data
        .into_iter()
        .map(|point| stock_price_history::ActiveModel {
            id: ActiveValue::NotSet,
            ticker: Set(ticker.to_string()),
            price_date: Set(point.price_date.to_string()),
            close_price: Set(point.close_price),
            source: Set("yahoo_finance".to_string()),
            fetched_at: Set(now.clone()),
        })
        .collect();
    insert_ignoring_conflicts::<stock_price_history::Entity>(
        db,
        models,
        OnConflict::columns([
            stock_price_history::Column::Ticker,
            stock_price_history::Column::PriceDate,
        ])
        .do_nothing()
        .to_owned(),
    )
    .await?;

    Ok(MetalTickerResult {
        ticker: ticker.to_string(),
        name: quote.name,
        price: quote.price,
        history_points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    async fn dev_db() -> DatabaseConnection {
        Database::connect("sqlite:///data/anchor.db?mode=rwc")
            .await
            .expect("failed to connect to the real dev database")
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_ticker_writes_real_rows() {
        let db = dev_db().await;
        let result = collect_ticker(&db, "XAU").await.unwrap();
        assert_eq!(result.ticker, "XAU");
        assert!(result.price > 0.0);
        assert!(result.history_points > 0);
    }
}
