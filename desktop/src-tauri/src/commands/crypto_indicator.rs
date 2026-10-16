use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};

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
    record_crypto_indicator_in(db.inner(), request).await
}

pub async fn record_crypto_indicator_in(
    db: &DatabaseConnection,
    request: RecordCryptoIndicatorRequest,
) -> Result<crypto_indicators::Model, AppError> {
    let threshold_row = indicator_thresholds::Entity::find()
        .filter(indicator_thresholds::Column::Indicator.eq(&request.indicator))
        .one(db)
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
    .insert(db)
    .await?;

    Ok(reading)
}

// ---------------------------------------------------------------------------
// Thresholds editáveis (Fase 31)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct IndicatorThresholdView {
    pub indicator: String,
    pub green_boundary: f64,
    pub red_boundary: f64,
    pub default_green: Option<f64>,
    pub default_red: Option<f64>,
    pub is_default: bool,
    pub higher_is_better: bool,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct UpdateIndicatorThresholdRequest {
    pub indicator: String,
    pub green_boundary: f64,
    pub red_boundary: f64,
}

fn threshold_view(row: indicator_thresholds::Model) -> IndicatorThresholdView {
    let defaults = crypto_score::default_threshold(&row.indicator);
    IndicatorThresholdView {
        is_default: defaults == Some((row.green_boundary, row.red_boundary)),
        default_green: defaults.map(|(green, _)| green),
        default_red: defaults.map(|(_, red)| red),
        higher_is_better: crypto_score::higher_is_better(row.green_boundary, row.red_boundary),
        indicator: row.indicator,
        green_boundary: row.green_boundary,
        red_boundary: row.red_boundary,
        updated_at: row.updated_at,
    }
}

/// Todos os thresholds, na ordem dos grupos do score (indicadores fora dos
/// grupos, se existirem, vão no fim).
#[tauri::command]
pub async fn list_indicator_thresholds(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<IndicatorThresholdView>, AppError> {
    let mut rows = indicator_thresholds::Entity::find().all(db.inner()).await?;
    let order: Vec<&str> = crypto_score::IndicatorGroup::ALL
        .iter()
        .flat_map(|g| g.indicators().iter().copied())
        .collect();
    rows.sort_by_key(|r| {
        (order.iter().position(|name| *name == r.indicator).unwrap_or(usize::MAX), r.indicator.clone())
    });
    Ok(rows.into_iter().map(threshold_view).collect())
}

/// Grava os limites e **reclassifica as leituras já guardadas** do indicador
/// (o `raw_value` fica; só o `signal` muda), na mesma transação. Sem isso o
/// score e os alertas, que leem o `signal` gravado, ficariam com a regra
/// antiga até a próxima coleta.
pub(crate) async fn apply_threshold(
    db: &DatabaseConnection,
    indicator: &str,
    green_boundary: f64,
    red_boundary: f64,
) -> Result<IndicatorThresholdView, AppError> {
    crypto_score::validate_boundaries(green_boundary, red_boundary)?;
    let threshold = Threshold { green_boundary, red_boundary };

    let txn = db.begin().await?;

    let row = indicator_thresholds::Entity::find()
        .filter(indicator_thresholds::Column::Indicator.eq(indicator))
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::UnknownIndicator(indicator.to_string()))?;
    let mut active: indicator_thresholds::ActiveModel = row.into();
    active.green_boundary = Set(green_boundary);
    active.red_boundary = Set(red_boundary);
    active.updated_at = Set(chrono::Utc::now().to_rfc3339());
    let updated = active.update(&txn).await?;

    let readings = crypto_indicators::Entity::find()
        .filter(crypto_indicators::Column::Indicator.eq(indicator))
        .all(&txn)
        .await?;
    for reading in readings {
        let signal = crypto_score::classify(reading.raw_value, &threshold)?.as_str();
        if reading.signal != signal {
            let mut reading: crypto_indicators::ActiveModel = reading.into();
            reading.signal = Set(signal.to_string());
            reading.update(&txn).await?;
        }
    }

    txn.commit().await?;
    Ok(threshold_view(updated))
}

#[tauri::command]
pub async fn update_indicator_threshold(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateIndicatorThresholdRequest,
) -> Result<IndicatorThresholdView, AppError> {
    apply_threshold(db.inner(), &request.indicator, request.green_boundary, request.red_boundary).await
}

/// Volta o indicador aos valores de fábrica (e reclassifica as leituras).
#[tauri::command]
pub async fn reset_indicator_threshold(
    db: tauri::State<'_, DatabaseConnection>,
    indicator: String,
) -> Result<IndicatorThresholdView, AppError> {
    let (green, red) = crypto_score::default_threshold(&indicator)
        .ok_or_else(|| AppError::UnknownIndicator(indicator.clone()))?;
    apply_threshold(db.inner(), &indicator, green, red).await
}

/// Score da moeda: última leitura de cada indicador, agrupada em subscores
/// (`domain::crypto_score::compute_score`). Desempate no mesmo dia: a leitura
/// criada por último (`crypto_indicators` é append-only, sem índice único).
#[tauri::command]
pub async fn get_crypto_score(
    db: tauri::State<'_, DatabaseConnection>,
    coin: String,
) -> Result<crypto_score::CryptoScore, AppError> {
    crypto_score_for(db.inner(), coin).await
}

pub async fn crypto_score_for(
    db: &DatabaseConnection,
    coin: String,
) -> Result<crypto_score::CryptoScore, AppError> {
    let readings = crypto_indicators::Entity::find()
        .filter(crypto_indicators::Column::Coin.eq(coin))
        .order_by_desc(crypto_indicators::Column::ReadingDate)
        .order_by_desc(crypto_indicators::Column::CreatedAt)
        .order_by_desc(crypto_indicators::Column::Id)
        .all(db)
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

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::ConnectionTrait;

    async fn memory_db() -> DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE indicator_thresholds (id INTEGER PRIMARY KEY, indicator TEXT UNIQUE, \
             green_boundary REAL, red_boundary REAL, updated_at TEXT)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE TABLE crypto_indicators (id INTEGER PRIMARY KEY, coin TEXT, indicator TEXT, \
             reading_date TEXT, raw_value REAL, signal TEXT, source TEXT, created_at TEXT)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO indicator_thresholds (indicator, green_boundary, red_boundary, updated_at) \
             VALUES ('funding_rate_7d', 5.0, 20.0, 'x'), ('mvrv_z_score', 0.0, 7.0, 'x')",
        )
        .await
        .unwrap();
        for (indicator, date, raw, signal) in [
            ("funding_rate_7d", "2026-10-14", 12.0, "NEUTRAL"),
            ("funding_rate_7d", "2026-10-15", 4.0, "GREEN"),
            ("mvrv_z_score", "2026-10-15", 3.0, "NEUTRAL"),
        ] {
            db.execute_unprepared(&format!(
                "INSERT INTO crypto_indicators (coin, indicator, reading_date, raw_value, signal, source, created_at) \
                 VALUES ('ETH', '{indicator}', '{date}', {raw}, '{signal}', 't', 'x')"
            ))
            .await
            .unwrap();
        }
        db
    }

    async fn signals(db: &DatabaseConnection, indicator: &str) -> Vec<String> {
        crypto_indicators::Entity::find()
            .filter(crypto_indicators::Column::Indicator.eq(indicator))
            .order_by_asc(crypto_indicators::Column::ReadingDate)
            .all(db)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.signal)
            .collect()
    }

    #[tokio::test]
    async fn updating_a_threshold_reclassifies_stored_readings_of_that_indicator() {
        let db = memory_db().await;

        // Funding passa a ser verde só abaixo de 2% e vermelho acima de 10%:
        // 12 → RED, 4 → NEUTRAL.
        let view = apply_threshold(&db, "funding_rate_7d", 2.0, 10.0).await.unwrap();

        assert_eq!((view.green_boundary, view.red_boundary), (2.0, 10.0));
        assert!(!view.is_default);
        assert_eq!(signals(&db, "funding_rate_7d").await, ["RED", "NEUTRAL"]);
        // Outro indicador não é tocado.
        assert_eq!(signals(&db, "mvrv_z_score").await, ["NEUTRAL"]);
    }

    #[tokio::test]
    async fn invalid_or_unknown_threshold_changes_nothing() {
        let db = memory_db().await;

        assert!(matches!(
            apply_threshold(&db, "funding_rate_7d", 7.0, 7.0).await,
            Err(AppError::EqualThresholds)
        ));
        assert!(matches!(
            apply_threshold(&db, "made_up", 1.0, 2.0).await,
            Err(AppError::UnknownIndicator(_))
        ));
        assert_eq!(signals(&db, "funding_rate_7d").await, ["NEUTRAL", "GREEN"]);
    }

    #[tokio::test]
    async fn resetting_restores_factory_values_and_signals() {
        let db = memory_db().await;
        apply_threshold(&db, "funding_rate_7d", 2.0, 10.0).await.unwrap();

        let (green, red) = crypto_score::default_threshold("funding_rate_7d").unwrap();
        let view = apply_threshold(&db, "funding_rate_7d", green, red).await.unwrap();

        assert!(view.is_default);
        assert_eq!(signals(&db, "funding_rate_7d").await, ["NEUTRAL", "GREEN"]);
    }
}
