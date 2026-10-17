// Câmbio multi-moeda (Fase 32, seção I) — mesmo papel de `finance_api::metals`:
// grava o par nas tabelas genéricas `stock_quotes`/`stock_price_history` que o
// resto do app já lê, com o código do par (`EURUSD`) no lugar do ticker.
use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, Set};

use crate::entity::{stock_price_history, stock_quotes};
use crate::error::AppError;
use crate::finance_api::{client, insert_ignoring_conflicts};

pub struct FxPairResult {
    pub name: String,
    pub price: f64,
    pub quote_currency: String,
    pub history_points: usize,
}

pub async fn collect_pair(
    db: &DatabaseConnection,
    base: &str,
    quote: &str,
) -> Result<FxPairResult, AppError> {
    let q = client::fetch_fx_quote(base, quote).await?;
    let history = client::fetch_fx_price_history(base, quote).await?;
    let now = Utc::now().to_rfc3339();

    stock_quotes::ActiveModel {
        id: ActiveValue::NotSet,
        ticker: Set(q.pair_code.clone()),
        price: Set(q.price),
        source: Set("yahoo_finance".to_string()),
        fetched_at: Set(now.clone()),
        name: Set(Some(q.name.clone())),
        exchange: Set(Some("FX".to_string())),
        currency: Set(Some(q.quote_currency.clone())),
    }
    .insert(db)
    .await?;

    let history_points = history.data.len();
    let models: Vec<stock_price_history::ActiveModel> = history
        .data
        .into_iter()
        .map(|point| stock_price_history::ActiveModel {
            id: ActiveValue::NotSet,
            ticker: Set(q.pair_code.clone()),
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

    Ok(FxPairResult {
        name: q.name,
        price: q.price,
        quote_currency: q.quote_currency,
        history_points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    #[tokio::test]
    async fn unknown_pair_fails_before_any_network_call() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let err = collect_pair(&db, "USD", "VND").await.err().unwrap();
        assert!(matches!(err, AppError::NoData(_)));
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_pair_writes_real_rows() {
        let db = Database::connect("sqlite:///data/anchor.db?mode=rwc")
            .await
            .expect("failed to connect to the real dev database");
        let result = collect_pair(&db, "USD", "BRL").await.unwrap();
        assert_eq!(result.name, "USD/BRL");
        assert!(result.price > 0.0);
        assert!(result.history_points > 0);
    }
}
