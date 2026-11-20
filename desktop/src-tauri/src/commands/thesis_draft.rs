// Rascunho de tese a partir dos dados coletados (frente 3 do ROADMAP, sem IA). Só leitura:
// monta o texto e devolve; quem salva a tese é o formulário, depois de o dono editar.
// Os indicadores vêm do screener (mesma conta de DY e razões), os valuations da tabela `valuation`.

use chrono::{NaiveDate, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::commands::screener::{screen_stocks_on, ScreenerFilters};
use crate::domain::thesis_draft::{render_draft, Draft, DraftInput};
use crate::entity::valuation;
use crate::error::AppError;

/// `today` é entrada (não lê o relógio) para o retrato datado ser testável.
pub async fn build_draft_on(db: &DatabaseConnection, ticker: &str, today: NaiveDate) -> Result<Draft, AppError> {
    let _ = (db, ticker, today);
    let _ = (screen_stocks_on, ScreenerFilters::default, render_draft, Utc::now);
    let _: Option<DraftInput> = None;
    let _ = (valuation::Entity::find(), valuation::Column::Ticker.eq(""));
    todo!("build_draft_on")
}

#[tauri::command]
pub async fn build_thesis_draft(db: tauri::State<'_, DatabaseConnection>, ticker: String) -> Result<Draft, AppError> {
    build_draft_on(db.inner(), ticker.trim(), Utc::now().date_naive()).await
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, Set};

    use super::*;
    use crate::entity::{stock_fundamentals, stock_quotes};
    use crate::test_support::memory_db;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
    }

    async fn collected(db: &DatabaseConnection, ticker: &str) {
        let at = "2026-11-01T10:00:00+00:00";
        stock_quotes::ActiveModel {
            ticker: Set(ticker.to_string()),
            price: Set(20.0),
            source: Set("test".to_string()),
            fetched_at: Set(at.to_string()),
            name: Set(Some("Banco do Brasil".to_string())),
            exchange: Set(None),
            currency: Set(Some("BRL".to_string())),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert quote");
        stock_fundamentals::ActiveModel {
            ticker: Set(ticker.to_string()),
            lpa: Set(2.0),
            vpa: Set(10.0),
            roe: Set(15.0),
            source: Set("test".to_string()),
            fetched_at: Set(at.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert fundamentals");
    }

    async fn saved(db: &DatabaseConnection, ticker: &str, model: &str, fair: f64, at: &str) {
        valuation::ActiveModel {
            ticker: Set(ticker.to_string()),
            reference_year: Set(2026),
            current_price: Set(20.0),
            model: Set(model.to_string()),
            fair_price: Set(Some(fair)),
            safety_margin: Set(Some((fair - 20.0) / fair)),
            verdict: Set(Some("CHEAP".to_string())),
            updated_at: Set(at.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert valuation");
    }

    #[tokio::test]
    async fn the_draft_uses_the_collected_numbers_and_the_latest_valuation_of_each_model() {
        let db = memory_db().await;
        collected(&db, "BBAS3").await;
        saved(&db, "BBAS3", "bazin", 24.0, "2026-08-01T00:00:00+00:00").await;
        saved(&db, "BBAS3", "bazin", 25.0, "2026-10-01T00:00:00+00:00").await;
        saved(&db, "OUTRA3", "bazin", 99.0, "2026-10-02T00:00:00+00:00").await;

        let draft = build_draft_on(&db, "bbas3", today()).await.unwrap();
        let text = draft.content_markdown;
        assert_eq!(draft.title, "BBAS3 thesis");
        assert!(text.starts_with("# BBAS3 — Banco do Brasil\n"), "{text}");
        assert!(text.contains("## Snapshot (2026-11-20)"), "{text}");
        assert!(text.contains("- P/E: 10.00"), "{text}");
        assert!(text.contains("- P/B: 2.00"), "{text}");
        assert!(text.contains("- bazin: fair price 25.00, margin 20.0%, CHEAP (updated 2026-10-01)"), "{text}");
        assert!(!text.contains("24.00"), "o valuation antigo do mesmo modelo não entra:\n{text}");
        assert!(!text.contains("99.00"), "valuation de outro ticker não entra:\n{text}");
    }

    #[tokio::test]
    async fn an_unknown_ticker_gets_the_skeleton_and_no_error() {
        let db = memory_db().await;
        let draft = build_draft_on(&db, "ZZZZ3", today()).await.unwrap();
        let text = draft.content_markdown;
        assert!(text.contains("No collected data for ZZZZ3 yet (n/d)."), "{text}");
        assert!(text.contains("No saved valuation (n/d)."), "{text}");
        assert!(text.contains("## Why own it"), "{text}");
    }
}
