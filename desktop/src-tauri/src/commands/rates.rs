// Fase 17 — juros ao vivo pro widget da Home e pra tela "Interest rates" do
// Research. A Selic vem de `selic_daily` (Fase 26, BCB direto); a curva DI vem
// da B3 (cache de 1 h na memória) e é montada a cada chamada.

use chrono::{Duration, NaiveDate, Utc};
use sea_orm::DatabaseConnection;
use serde::Serialize;

use crate::domain::rates;
use crate::error::AppError;
use crate::finance_api::{client, skip_not_found};

/// Prazos-chave da curva DI, em anos.
const KEY_TENORS_YEARS: [f64; 5] = [1.0, 2.0, 3.0, 5.0, 10.0];
/// Quantos dias voltar procurando um pregão com curva publicada.
const MAX_DAYS_BACK: i64 = 10;

#[derive(Serialize)]
pub struct SelicChange {
    pub date: NaiveDate,
    pub rate_pct: f64,
}

#[derive(Serialize)]
pub struct SelicView {
    pub current_pct: f64,
    /// Desde quando a meta atual vale (última decisão do Copom).
    pub since: NaiveDate,
    pub changes: Vec<SelicChange>,
}

#[derive(Serialize)]
pub struct CurveVertex {
    pub years: f64,
    pub rate_pct: f64,
}

#[derive(Serialize)]
pub struct KeyRate {
    pub years: f64,
    pub rate_pct: Option<f64>,
    pub rate_pct_month_ago: Option<f64>,
}

#[derive(Serialize)]
pub struct DiCurveView {
    pub reference_date: NaiveDate,
    pub vertices: Vec<CurveVertex>,
    pub month_ago_reference_date: Option<NaiveDate>,
    pub month_ago_vertices: Vec<CurveVertex>,
    pub key_rates: Vec<KeyRate>,
}

/// As duas partes falham de forma independente (fontes diferentes: BCB e
/// B3) — uma fora do ar não esconde a outra; o erro vai em `*_error`.
#[derive(Serialize)]
pub struct InterestRatesView {
    pub selic: Option<SelicView>,
    pub selic_error: Option<String>,
    /// `None` sem erro = nenhuma curva publicada nos últimos dias.
    pub di: Option<DiCurveView>,
    pub di_error: Option<String>,
}

/// Curva do último pregão em ou antes de `from` (volta dia a dia; dia sem
/// pregão é `NoData`).
async fn latest_curve(from: NaiveDate) -> Result<Option<client::DiCurveResponse>, AppError> {
    for back in 0..=MAX_DAYS_BACK {
        let date = from - Duration::days(back);
        if let Some(curve) = skip_not_found(client::fetch_di_futures_curve(date)).await? {
            if !curve.data.is_empty() {
                return Ok(Some(curve));
            }
        }
    }
    Ok(None)
}

fn to_pairs(curve: &client::DiCurveResponse) -> Vec<(i32, f64)> {
    curve.data.iter().map(|v| (v.dias_uteis, v.rate_pct)).collect()
}

fn to_vertices(curve: &client::DiCurveResponse) -> Vec<CurveVertex> {
    curve
        .data
        .iter()
        .map(|v| CurveVertex {
            years: v.dias_uteis as f64 / 252.0,
            rate_pct: v.rate_pct,
        })
        .collect()
}

async fn selic_view(db: &DatabaseConnection) -> Result<SelicView, AppError> {
    let response = client::fetch_selic(db, "meta").await?;
    let points: Vec<(NaiveDate, f64)> =
        response.data.iter().map(|p| (p.reference_date, p.value_pct)).collect();
    let changes = rates::selic_changes(&points);
    let &(since, current_pct) = changes
        .last()
        .ok_or_else(|| AppError::NotFound("Selic series is empty".to_string()))?;
    Ok(SelicView {
        current_pct,
        since,
        changes: changes
            .into_iter()
            .map(|(date, rate_pct)| SelicChange { date, rate_pct })
            .collect(),
    })
}

async fn di_view() -> Result<Option<DiCurveView>, AppError> {
    let Some(curve) = latest_curve(Utc::now().date_naive()).await? else {
        return Ok(None);
    };
    let month_ago = latest_curve(curve.reference_date - Duration::days(30)).await?;
    let now_pairs = to_pairs(&curve);
    let ago_pairs = month_ago.as_ref().map(to_pairs).unwrap_or_default();
    Ok(Some(DiCurveView {
        reference_date: curve.reference_date,
        vertices: to_vertices(&curve),
        month_ago_reference_date: month_ago.as_ref().map(|c| c.reference_date),
        month_ago_vertices: month_ago.as_ref().map(to_vertices).unwrap_or_default(),
        key_rates: KEY_TENORS_YEARS
            .iter()
            .map(|&years| KeyRate {
                years,
                rate_pct: rates::rate_at_years(&now_pairs, years),
                rate_pct_month_ago: rates::rate_at_years(&ago_pairs, years),
            })
            .collect(),
    }))
}

#[tauri::command]
pub async fn get_interest_rates(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<InterestRatesView, AppError> {
    let (selic, di) = tokio::join!(selic_view(&db), di_view());
    let (selic, selic_error) = match selic {
        Ok(view) => (Some(view), None),
        Err(err) => (None, Some(err.to_string())),
    };
    let (di, di_error) = match di {
        Ok(view) => (view, None),
        Err(err) => (None, Some(err.to_string())),
    };
    Ok(InterestRatesView { selic, selic_error, di, di_error })
}

// Testes `#[ignore]` — vão às fontes públicas (BCB e B3) de verdade. Rodar com
// `cargo test --lib -- --ignored commands::rates`.
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn live_latest_curve_finds_a_trading_day() {
        let curve = latest_curve(Utc::now().date_naive())
            .await
            .unwrap()
            .expect("a published DI curve in the last days");

        assert!(curve.data.len() > 200, "only {} vertices", curve.data.len());
        let one_year = rates::rate_at_years(&to_pairs(&curve), 1.0).unwrap();
        assert!(one_year > 2.0 && one_year < 40.0, "1y DI {one_year}% looks wrong");
    }

    #[tokio::test]
    #[ignore]
    async fn live_selic_current_is_plausible() {
        let db = crate::test_support::memory_db().await;
        let response = client::fetch_selic(&db, "meta").await.unwrap();
        let points: Vec<(NaiveDate, f64)> =
            response.data.iter().map(|p| (p.reference_date, p.value_pct)).collect();
        let (_, current) = *rates::selic_changes(&points).last().unwrap();

        assert!(current > 2.0 && current < 40.0, "Selic {current}% looks wrong");
    }
}
