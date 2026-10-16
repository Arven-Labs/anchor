use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Deserialize;

use crate::domain::crypto_score::{self, Threshold};
use crate::entity::{crypto_indicators, indicator_thresholds};
use crate::error::AppError;

#[derive(Deserialize)]
pub struct RecordCryptoIndicatorRequest {
    pub coin: String,
    pub indicator: String,
    pub reading_date: String,
    pub raw_value: f64,
    pub source: String,
}

#[tauri::command]
pub async fn record_crypto_indicator(
    db: tauri::State<'_, DatabaseConnection>,
    request: RecordCryptoIndicatorRequest,
) -> Result<crypto_indicators::Model, AppError> {
    let threshold_row = indicator_thresholds::Entity::find()
        .filter(indicator_thresholds::Column::Indicator.eq(&request.indicator))
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::UnknownIndicator(request.indicator.clone()))?;

    let signal = crypto_score::classify(
        request.raw_value,
        &Threshold {
            green_boundary: threshold_row.green_boundary,
            red_boundary: threshold_row.red_boundary,
        },
    )?;

    let reading = crypto_indicators::ActiveModel {
        coin: Set(request.coin),
        indicator: Set(request.indicator),
        reading_date: Set(request.reading_date),
        raw_value: Set(request.raw_value),
        signal: Set(signal.as_str().to_string()),
        source: Set(request.source),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?;

    Ok(reading)
}

/// Score da moeda: última leitura de cada indicador, agrupada em subscores
/// (`domain::crypto_score::compute_score`). Desempate no mesmo dia: a leitura
/// criada por último (`crypto_indicators` é append-only, sem índice único).
#[tauri::command]
pub async fn get_crypto_score(
    db: tauri::State<'_, DatabaseConnection>,
    coin: String,
) -> Result<crypto_score::CryptoScore, AppError> {
    let readings = crypto_indicators::Entity::find()
        .filter(crypto_indicators::Column::Coin.eq(coin))
        .order_by_desc(crypto_indicators::Column::ReadingDate)
        .order_by_desc(crypto_indicators::Column::CreatedAt)
        .order_by_desc(crypto_indicators::Column::Id)
        .all(db.inner())
        .await?;

    let mut latest: Vec<crypto_score::LatestReading> = Vec::new();
    for r in readings {
        if latest.iter().any(|l| l.indicator == r.indicator) {
            continue;
        }
        latest.push(crypto_score::LatestReading {
            indicator: r.indicator,
            raw_value: r.raw_value,
            signal: r.signal,
            reading_date: r.reading_date,
            source: r.source,
        });
    }

    Ok(crypto_score::compute_score(&latest))
}

#[tauri::command]
pub async fn list_crypto_indicators(
    db: tauri::State<'_, DatabaseConnection>,
    coin: String,
) -> Result<Vec<crypto_indicators::Model>, AppError> {
    let readings = crypto_indicators::Entity::find()
        .filter(crypto_indicators::Column::Coin.eq(coin))
        .order_by_desc(crypto_indicators::Column::ReadingDate)
        .all(db.inner())
        .await?;

    Ok(readings)
}
