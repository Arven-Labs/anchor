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
        .filter(indicator_thresholds::Column::Coin.eq(crypto_score::threshold_coin(&request.coin)))
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
    /// Moeda dona dos limites (`ETH`, `BTC` ou `ADA`); as demais usam os do ETH.
    pub coin: String,
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
    pub coin: String,
    pub indicator: String,
    pub green_boundary: f64,
    pub red_boundary: f64,
}

fn threshold_view(row: indicator_thresholds::Model) -> IndicatorThresholdView {
    let defaults = crypto_score::default_threshold(&row.coin, &row.indicator);
    IndicatorThresholdView {
        is_default: defaults == Some((row.green_boundary, row.red_boundary)),
        default_green: defaults.map(|(green, _)| green),
        default_red: defaults.map(|(_, red)| red),
        higher_is_better: crypto_score::higher_is_better(row.green_boundary, row.red_boundary),
        coin: row.coin,
        indicator: row.indicator,
        green_boundary: row.green_boundary,
        red_boundary: row.red_boundary,
        updated_at: row.updated_at,
    }
}

/// Os thresholds da moeda, na ordem dos grupos do score (indicadores fora dos
/// grupos, se existirem, vão no fim). Moeda sem conjunto próprio mostra o do ETH.
#[tauri::command]
pub async fn list_indicator_thresholds(
    db: tauri::State<'_, DatabaseConnection>,
    coin: String,
) -> Result<Vec<IndicatorThresholdView>, AppError> {
    let mut rows = indicator_thresholds::Entity::find()
        .filter(indicator_thresholds::Column::Coin.eq(crypto_score::threshold_coin(&coin)))
        .all(db.inner())
        .await?;
    let order: Vec<&str> = crypto_score::IndicatorGroup::ALL
        .iter()
        .flat_map(|g| g.indicators(&coin).iter().copied())
        .collect();
    rows.sort_by_key(|r| {
        (order.iter().position(|name| *name == r.indicator).unwrap_or(usize::MAX), r.indicator.clone())
    });
    Ok(rows.into_iter().map(threshold_view).collect())
}

/// Grava os limites e **reclassifica as leituras já guardadas** do indicador
/// (o `raw_value` fica; só o `signal` muda), na mesma transação. Sem isso o
/// score e os alertas, que leem o `signal` gravado, ficariam com a regra
/// antiga até a próxima coleta. Só as leituras que usam esses limites: editar
/// o BTC mexe só nas leituras do BTC, o ADA só nas do ADA; editar o ETH, nas das
/// demais moedas.
pub(crate) async fn apply_threshold(
    db: &DatabaseConnection,
    coin: &str,
    indicator: &str,
    green_boundary: f64,
    red_boundary: f64,
) -> Result<IndicatorThresholdView, AppError> {
    crypto_score::validate_boundaries(green_boundary, red_boundary)?;
    let threshold = Threshold { green_boundary, red_boundary };
    let owner = crypto_score::threshold_coin(coin);

    let txn = db.begin().await?;

    let row = indicator_thresholds::Entity::find()
        .filter(indicator_thresholds::Column::Coin.eq(owner))
        .filter(indicator_thresholds::Column::Indicator.eq(indicator))
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::UnknownIndicator(indicator.to_string()))?;
    let mut active: indicator_thresholds::ActiveModel = row.into();
    active.green_boundary = Set(green_boundary);
    active.red_boundary = Set(red_boundary);
    active.updated_at = Set(chrono::Utc::now().to_rfc3339());
    let updated = active.update(&txn).await?;

    let owned_readings = crypto_indicators::Entity::find()
        .filter(crypto_indicators::Column::Indicator.eq(indicator));
    let readings = if owner == crypto_score::COIN_ETH {
        // O ETH é o dono das moedas sem conjunto próprio: tudo que não é de outro dono.
        owned_readings
            .filter(crypto_indicators::Column::Coin.is_not_in([crypto_score::COIN_BTC, crypto_score::COIN_ADA]))
    } else {
        owned_readings.filter(crypto_indicators::Column::Coin.eq(owner))
    }
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
    apply_threshold(
        db.inner(),
        &request.coin,
        &request.indicator,
        request.green_boundary,
        request.red_boundary,
    )
    .await
}

/// Volta o indicador aos valores de fábrica (e reclassifica as leituras).
#[tauri::command]
pub async fn reset_indicator_threshold(
    db: tauri::State<'_, DatabaseConnection>,
    coin: String,
    indicator: String,
) -> Result<IndicatorThresholdView, AppError> {
    let (green, red) = crypto_score::default_threshold(&coin, &indicator)
        .ok_or_else(|| AppError::UnknownIndicator(indicator.clone()))?;
    apply_threshold(db.inner(), &coin, &indicator, green, red).await
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
        .filter(crypto_indicators::Column::Coin.eq(&coin))
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

    Ok(crypto_score::compute_score(&coin, &latest))
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
            "CREATE TABLE indicator_thresholds (id INTEGER PRIMARY KEY, coin TEXT, indicator TEXT, \
             green_boundary REAL, red_boundary REAL, updated_at TEXT, UNIQUE (coin, indicator))",
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
            "INSERT INTO indicator_thresholds (coin, indicator, green_boundary, red_boundary, updated_at) \
             VALUES ('ETH', 'funding_rate_7d', 5.0, 20.0, 'x'), ('ETH', 'mvrv_z_score', 0.0, 7.0, 'x'), \
             ('BTC', 'funding_rate_7d', 5.0, 15.0, 'x')",
        )
        .await
        .unwrap();
        for (coin, indicator, date, raw, signal) in [
            ("ETH", "funding_rate_7d", "2026-10-14", 12.0, "NEUTRAL"),
            ("ETH", "funding_rate_7d", "2026-10-15", 4.0, "GREEN"),
            ("ETH", "mvrv_z_score", "2026-10-15", 3.0, "NEUTRAL"),
            // BTC e SOL guardam o mesmo valor de 12%, classificado por regras diferentes.
            ("BTC", "funding_rate_7d", "2026-10-15", 12.0, "NEUTRAL"),
            ("SOL", "funding_rate_7d", "2026-10-15", 12.0, "NEUTRAL"),
        ] {
            db.execute_unprepared(&format!(
                "INSERT INTO crypto_indicators (coin, indicator, reading_date, raw_value, signal, source, created_at) \
                 VALUES ('{coin}', '{indicator}', '{date}', {raw}, '{signal}', 't', 'x')"
            ))
            .await
            .unwrap();
        }
        db
    }

    async fn signals(db: &DatabaseConnection, coin: &str, indicator: &str) -> Vec<String> {
        crypto_indicators::Entity::find()
            .filter(crypto_indicators::Column::Coin.eq(coin))
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
        let view = apply_threshold(&db, "ETH", "funding_rate_7d", 2.0, 10.0).await.unwrap();

        assert_eq!((view.green_boundary, view.red_boundary), (2.0, 10.0));
        assert_eq!(view.coin, "ETH");
        assert!(!view.is_default);
        assert_eq!(signals(&db, "ETH", "funding_rate_7d").await, ["RED", "NEUTRAL"]);
        // Outro indicador não é tocado.
        assert_eq!(signals(&db, "ETH", "mvrv_z_score").await, ["NEUTRAL"]);
    }

    #[tokio::test]
    async fn editing_eth_reclassifies_eth_and_other_coins_but_not_btc() {
        let db = memory_db().await;

        apply_threshold(&db, "ETH", "funding_rate_7d", 2.0, 10.0).await.unwrap();

        // SOL usa os limites do ETH: 12 → RED. O BTC tem os seus: segue NEUTRAL.
        assert_eq!(signals(&db, "SOL", "funding_rate_7d").await, ["RED"]);
        assert_eq!(signals(&db, "BTC", "funding_rate_7d").await, ["NEUTRAL"]);
    }

    #[tokio::test]
    async fn editing_btc_touches_only_btc_readings_and_its_own_row() {
        let db = memory_db().await;

        let view = apply_threshold(&db, "BTC", "funding_rate_7d", 2.0, 10.0).await.unwrap();

        assert_eq!(view.coin, "BTC");
        assert_eq!(signals(&db, "BTC", "funding_rate_7d").await, ["RED"]);
        assert_eq!(signals(&db, "ETH", "funding_rate_7d").await, ["NEUTRAL", "GREEN"]);
        assert_eq!(signals(&db, "SOL", "funding_rate_7d").await, ["NEUTRAL"]);
        // Os limites do ETH continuam os de antes.
        let eth = indicator_thresholds::Entity::find()
            .filter(indicator_thresholds::Column::Coin.eq("ETH"))
            .filter(indicator_thresholds::Column::Indicator.eq("funding_rate_7d"))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!((eth.green_boundary, eth.red_boundary), (5.0, 20.0));
    }

    #[tokio::test]
    async fn a_reading_is_classified_by_its_coins_thresholds() {
        let db = memory_db().await;
        let record = |coin: &str| RecordCryptoIndicatorRequest {
            coin: coin.to_string(),
            indicator: "funding_rate_7d".to_string(),
            reading_date: "2026-10-16".to_string(),
            raw_value: 17.0,
            source: "t".to_string(),
        };

        // 17% de funding: BTC (vermelho a partir de 15) → RED; ETH e SOL (a partir de 20) → NEUTRAL.
        assert_eq!(record_crypto_indicator_in(&db, record("BTC")).await.unwrap().signal, "RED");
        assert_eq!(record_crypto_indicator_in(&db, record("ETH")).await.unwrap().signal, "NEUTRAL");
        assert_eq!(record_crypto_indicator_in(&db, record("SOL")).await.unwrap().signal, "NEUTRAL");
    }

    #[tokio::test]
    async fn invalid_or_unknown_threshold_changes_nothing() {
        let db = memory_db().await;

        assert!(matches!(
            apply_threshold(&db, "ETH", "funding_rate_7d", 7.0, 7.0).await,
            Err(AppError::EqualThresholds)
        ));
        assert!(matches!(
            apply_threshold(&db, "ETH", "made_up", 1.0, 2.0).await,
            Err(AppError::UnknownIndicator(_))
        ));
        // BTC sem a linha do indicador (no teste só tem o funding) também é erro.
        assert!(matches!(
            apply_threshold(&db, "BTC", "mvrv_z_score", 1.0, 2.0).await,
            Err(AppError::UnknownIndicator(_))
        ));
        assert_eq!(signals(&db, "ETH", "funding_rate_7d").await, ["NEUTRAL", "GREEN"]);
    }

    #[tokio::test]
    async fn resetting_restores_factory_values_and_signals() {
        let db = memory_db().await;
        apply_threshold(&db, "ETH", "funding_rate_7d", 2.0, 10.0).await.unwrap();

        let (green, red) = crypto_score::default_threshold("ETH", "funding_rate_7d").unwrap();
        let view = apply_threshold(&db, "ETH", "funding_rate_7d", green, red).await.unwrap();

        assert!(view.is_default);
        assert_eq!(signals(&db, "ETH", "funding_rate_7d").await, ["NEUTRAL", "GREEN"]);
    }

    #[tokio::test]
    async fn resetting_btc_uses_the_btc_factory_values() {
        let db = memory_db().await;
        apply_threshold(&db, "BTC", "funding_rate_7d", 2.0, 10.0).await.unwrap();

        let (green, red) = crypto_score::default_threshold("BTC", "funding_rate_7d").unwrap();
        assert_eq!((green, red), (5.0, 15.0));
        let view = apply_threshold(&db, "BTC", "funding_rate_7d", green, red).await.unwrap();

        assert!(view.is_default);
        assert_eq!(signals(&db, "BTC", "funding_rate_7d").await, ["NEUTRAL"]);
    }
}
