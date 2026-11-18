// Sugestão do `g` (crescimento dos dividendos) para Gordon e Teto Projetado: lê os
// proventos já coletados do ticker e devolve o CAGR anual. Só sugere; o formulário
// nunca preenche sozinho (ver `domain::dividend_growth`).

use chrono::{NaiveDate, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::domain::dividend_growth::{self, DividendGrowth};
use crate::entity::stock_dividend_payments;
use crate::error::AppError;

async fn dividend_growth_in(
    db: &DatabaseConnection,
    ticker: &str,
    today: NaiveDate,
) -> Result<Option<DividendGrowth>, AppError> {
    let payments: Vec<(NaiveDate, f64)> = stock_dividend_payments::Entity::find()
        .filter(stock_dividend_payments::Column::Ticker.eq(ticker.trim().to_uppercase()))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|p| NaiveDate::parse_from_str(&p.payment_date, "%Y-%m-%d").ok().map(|d| (d, p.amount)))
        .collect();
    Ok(dividend_growth::dividend_growth(&payments, today))
}

#[tauri::command]
pub async fn get_dividend_growth(
    db: tauri::State<'_, DatabaseConnection>,
    ticker: String,
) -> Result<Option<DividendGrowth>, AppError> {
    dividend_growth_in(db.inner(), &ticker, Utc::now().date_naive()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::memory_db;
    use sea_orm::{ActiveModelTrait, Set};

    async fn paid(db: &DatabaseConnection, ticker: &str, date: &str, amount: f64) {
        stock_dividend_payments::ActiveModel {
            ticker: Set(ticker.to_string()),
            payment_date: Set(date.to_string()),
            amount: Set(amount),
            source: Set("test".to_string()),
            fetched_at: Set("2026-11-01T10:00:00+00:00".to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert dividend payment");
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 10).unwrap()
    }

    #[tokio::test]
    async fn uses_only_the_payments_of_the_asked_ticker_and_accepts_lowercase() {
        let db = memory_db().await;
        for (year, amount) in [(2022, 1.0), (2023, 2.0), (2024, 4.0), (2025, 8.0)] {
            paid(&db, "AAAA3", &format!("{year}-06-15"), amount).await;
            paid(&db, "BBBB3", &format!("{year}-06-15"), 1.0).await;
        }

        let got = dividend_growth_in(&db, " aaaa3 ", today()).await.unwrap().expect("sugere");

        assert!((got.cagr_pct - 100.0).abs() < 1e-6);
        assert_eq!((got.from_year, got.to_year), (2022, 2025));
    }

    #[tokio::test]
    async fn ticker_without_payments_gives_nothing() {
        let db = memory_db().await;
        assert_eq!(dividend_growth_in(&db, "NADA3", today()).await.unwrap(), None);
    }
}
