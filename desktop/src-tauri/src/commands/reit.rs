use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set, Unchanged};
use serde::Deserialize;

use crate::entity::{reit_fundamentals, reit_manual_indicators};
use crate::error::AppError;

/// Fase 10, item 8 — REIT. Indicadores imobiliários via SEC EDGAR, mesmo
/// padrão time-series de `list_stock_fundamentals` (todas as linhas, mais
/// recente primeiro — filtro por ticker fica client-side, igual o resto do
/// app já faz pra `stock_fundamentals`/`stock_dcf_fundamentals`).
#[tauri::command]
pub async fn list_reit_fundamentals(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<reit_fundamentals::Model>, AppError> {
    let rows = reit_fundamentals::Entity::find()
        .order_by_desc(reit_fundamentals::Column::FetchedAt)
        .all(db.inner())
        .await?;

    Ok(rows)
}

#[tauri::command]
pub async fn get_reit_manual_indicators(
    db: tauri::State<'_, DatabaseConnection>,
    ticker: String,
) -> Result<Option<reit_manual_indicators::Model>, AppError> {
    let row = reit_manual_indicators::Entity::find()
        .filter(reit_manual_indicators::Column::Ticker.eq(ticker))
        .one(db.inner())
        .await?;

    Ok(row)
}

/// FFO/AFFO por ação e taxa de ocupação — os indicadores "de verdade" de
/// REIT que não existem como tag XBRL (confirmado ao vivo antes de
/// implementar, ver docstring da migration `m20260803_140000_create_reit_tables`).
/// Campo manual editável, find-then-upsert por ticker.
#[derive(Deserialize)]
pub struct SaveReitManualIndicatorsRequest {
    pub ticker: String,
    pub ffo_per_share: Option<f64>,
    pub affo_per_share: Option<f64>,
    pub occupancy_pct: Option<f64>,
}

#[tauri::command]
pub async fn save_reit_manual_indicators(
    db: tauri::State<'_, DatabaseConnection>,
    request: SaveReitManualIndicatorsRequest,
) -> Result<reit_manual_indicators::Model, AppError> {
    let db = db.inner();
    let ticker = request.ticker.trim().to_uppercase();

    let existing = reit_manual_indicators::Entity::find()
        .filter(reit_manual_indicators::Column::Ticker.eq(ticker.clone()))
        .one(db)
        .await?;

    let now = chrono::Utc::now().to_rfc3339();
    let saved = if let Some(existing) = existing {
        reit_manual_indicators::ActiveModel {
            id: Unchanged(existing.id),
            ticker: Set(ticker),
            ffo_per_share: Set(request.ffo_per_share),
            affo_per_share: Set(request.affo_per_share),
            occupancy_pct: Set(request.occupancy_pct),
            updated_at: Set(now),
        }
        .update(db)
        .await?
    } else {
        reit_manual_indicators::ActiveModel {
            ticker: Set(ticker),
            ffo_per_share: Set(request.ffo_per_share),
            affo_per_share: Set(request.affo_per_share),
            occupancy_pct: Set(request.occupancy_pct),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await?
    };

    Ok(saved)
}
