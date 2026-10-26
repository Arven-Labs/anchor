// Fase 14.4 (fatia 2) — porta pra Rust os dois caminhos de cripto do
// coletor Python: `collect_crypto_ticker` (quote+histórico de preço de
// qualquer moeda, usado pelo Portfolio/Research) e `main_crypto`/
// `_record_crypto_indicator` (os indicadores do ciclo ETH, hardcoded
// `coin="ETH"`, usados só na tela de score cripto — sem relação com o
// primeiro). Eram 4 automatizados (Sessão 91); mais 4 se juntaram na
// automação depois que a Fase 1.12 do easybusiness achou fonte gratuita
// pra eles (CoinMetrics, ver `ETH_INDICATORS` abaixo). Na Fase 31 o
// `staking_yield` também virou automático e entraram 3 de derivativos,
// todos coletados direto pelo Anchor (`crypto_sources`), sem o Lume.
use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::crypto_sources;
use crate::domain::crypto_score::{self, Threshold};
use crate::entity::{crypto_fear_greed, crypto_indicators, indicator_thresholds, stock_price_history, stock_quotes};
use crate::error::AppError;
use crate::finance_api::{client, insert_ignoring_conflicts};

pub struct CryptoTickerResult {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub history_points: usize,
    pub fear_greed_value: i32,
    pub fear_greed_classification: String,
}

/// Quote + histórico diário pra uma moeda qualquer (Fase 10, item 8 — cripto
/// virou classe de ativo do Portfolio). Uma chamada de `market_chart` do
/// lado da Finance API já dá os dois; Fear & Greed é global de mercado (não
/// por moeda), atualizado de graça junto — `ON CONFLICT DO NOTHING` no
/// índice único de `reading_date`, só grava a primeira vez no dia.
pub async fn collect_ticker(
    db: &DatabaseConnection,
    symbol: &str,
) -> Result<CryptoTickerResult, AppError> {
    let quote = client::fetch_crypto_quote(symbol).await?;
    let history = client::fetch_crypto_price_history(symbol).await?;
    if history.data.is_empty() {
        return Err(AppError::NoData(format!(
            "no price history for '{symbol}' ({})",
            quote.coin_id
        )));
    }

    let now = Utc::now().to_rfc3339();
    let history_points = history.data.len();

    stock_quotes::ActiveModel {
        id: ActiveValue::NotSet,
        ticker: Set(symbol.to_string()),
        price: Set(quote.price),
        source: Set("coingecko".to_string()),
        fetched_at: Set(now.clone()),
        name: Set(Some(quote.name.clone())),
        exchange: Set(Some("CoinGecko".to_string())),
        currency: Set(Some("USD".to_string())),
    }
    .insert(db)
    .await?;

    let price_models: Vec<stock_price_history::ActiveModel> = history
        .data
        .into_iter()
        .map(|point| stock_price_history::ActiveModel {
            id: ActiveValue::NotSet,
            ticker: Set(symbol.to_string()),
            price_date: Set(point.price_date.to_string()),
            close_price: Set(point.price),
            source: Set("coingecko".to_string()),
            fetched_at: Set(now.clone()),
        })
        .collect();
    insert_ignoring_conflicts::<stock_price_history::Entity>(
        db,
        price_models,
        OnConflict::columns([
            stock_price_history::Column::Ticker,
            stock_price_history::Column::PriceDate,
        ])
        .do_nothing()
        .to_owned(),
    )
    .await?;

    let fear_greed = client::fetch_fear_greed().await?;
    insert_ignoring_conflicts::<crypto_fear_greed::Entity>(
        db,
        vec![crypto_fear_greed::ActiveModel {
            id: ActiveValue::NotSet,
            value: Set(fear_greed.value),
            classification: Set(fear_greed.classification.clone()),
            reading_date: Set(fear_greed.reading_date.to_string()),
            source: Set("alternative.me".to_string()),
            fetched_at: Set(now),
        }],
        OnConflict::column(crypto_fear_greed::Column::ReadingDate)
            .do_nothing()
            .to_owned(),
    )
    .await?;

    Ok(CryptoTickerResult {
        symbol: symbol.to_string(),
        name: quote.name,
        price: quote.price,
        history_points,
        fear_greed_value: fear_greed.value,
        fear_greed_classification: fear_greed.classification,
    })
}

// Os 4 primeiros são automatizados desde a Fase 1.6 do easybusiness (Sessão
// 91 deste repo); os 4 seguintes foram desbloqueados na Fase 1.12 dele
// (CoinMetrics Community API, sem chave, achado ao vivo nesta sessão) e os 2
// últimos entraram no lote 3 da Fase 31 (mesma fonte). Este array só tem os
// que vêm de `finance_core::eth_indicators`; os coletados direto pelo Anchor
// estão em `collect_eth_indicators`.
const ETH_INDICATORS: [(&str, &str, &str); 11] = [
    // (nome interno / chave de indicator_thresholds, código na Finance API, source)
    ("tvl_trend", "tvl-trend", "defillama"),
    ("net_issuance", "net-issuance", "ultrasound.money"),
    ("fees_vs_emission", "fees-vs-emission", "ultrasound.money"),
    ("nvt_ratio", "nvt-ratio", "coingecko"),
    ("mvrv_z_score", "mvrv-z-score", "coinmetrics"),
    ("puell_multiple", "puell-multiple", "coinmetrics"),
    ("exchange_netflow", "exchange-netflow", "coinmetrics"),
    ("active_addresses_trend", "active-addresses-trend", "coinmetrics"),
    ("exchange_supply_change_30d", "exchange-supply-trend", "coinmetrics"),
    ("eth_btc_trend_30d", "eth-btc-trend", "coinmetrics"),
    // Lote 4 (Market sentiment): preço ÷ média de 200 dias, ETH e BTC.
    ("mayer_multiple", "mayer-multiple", "coinmetrics"),
];

async fn record_crypto_indicator(
    db: &DatabaseConnection,
    coin: &str,
    indicator: &str,
    raw_value: f64,
    source: &str,
) -> Result<crypto_indicators::Model, AppError> {
    let threshold = indicator_thresholds::Entity::find()
        .filter(indicator_thresholds::Column::Coin.eq(crypto_score::threshold_coin(coin)))
        .filter(indicator_thresholds::Column::Indicator.eq(indicator))
        .one(db)
        .await?
        .ok_or_else(|| AppError::UnknownIndicator(indicator.to_string()))?;

    let signal = crypto_score::classify(
        raw_value,
        &Threshold {
            green_boundary: threshold.green_boundary,
            red_boundary: threshold.red_boundary,
        },
    )?;

    let model = crypto_indicators::ActiveModel {
        id: ActiveValue::NotSet,
        coin: Set(coin.to_string()),
        indicator: Set(indicator.to_string()),
        reading_date: Set(Utc::now().date_naive().to_string()),
        raw_value: Set(raw_value),
        signal: Set(signal.as_str().to_string()),
        source: Set(source.to_string()),
        created_at: Set(Utc::now().to_rfc3339()),
    }
    .insert(db)
    .await?;

    Ok(model)
}

/// Resultado de uma rodada de coleta: o que foi gravado e o que falhou. Uma
/// fonte fora do ar (ex.: Binance bloqueada na região) não derruba as demais.
pub struct IndicatorCollection {
    pub readings: Vec<crypto_indicators::Model>,
    /// Uma linha por indicador que falhou, já com o nome dele.
    pub errors: Vec<String>,
}

/// Indicadores do score de `coin` (`run_crypto_collector`), gravados sob essa
/// moeda. Só ETH, BTC, ADA e XRP têm score próprio; cada um coleta o seu conjunto
/// (`domain::crypto_score::IndicatorGroup::indicators`): 20 no ETH, 14 no BTC,
/// 10 no ADA, 9 no XRP.
/// Os que passam por `finance_core::eth_indicators` (CoinMetrics, CoinGecko,
/// DefiLlama, ultrasound.money) e, desde a Fase 31, os coletados direto pelo
/// Anchor (`crypto_sources`) — derivativos (Binance, com fallback Bybit),
/// opções (Deribit), stablecoins (DefiLlama) e, só no ETH, `staking_yield` real.
/// Só devolve erro se **nenhum** indicador foi coletado.
pub async fn collect_indicators(
    db: &DatabaseConnection,
    coin: &str,
) -> Result<IndicatorCollection, AppError> {
    let coin = coin.to_ascii_uppercase();
    let scored = [crypto_score::COIN_ETH, crypto_score::COIN_BTC, crypto_score::COIN_ADA, crypto_score::COIN_XRP];
    if !scored.contains(&coin.as_str()) {
        return Err(AppError::InvalidInput(format!(
            "the crypto score collector only supports ETH, BTC, ADA and XRP, not '{coin}'"
        )));
    }
    let wanted: Vec<&str> = crypto_score::IndicatorGroup::ALL
        .iter()
        .flat_map(|group| group.indicators(&coin).iter().copied())
        .collect();
    let wants = |indicator: &str| wanted.contains(&indicator);

    let mut readings = Vec::with_capacity(wanted.len());
    let mut errors = Vec::new();

    for (internal_name, api_code, source) in ETH_INDICATORS {
        if !wants(internal_name) {
            continue;
        }
        let fetched = client::fetch_crypto_indicator(&coin, api_code)
            .await
            .map(|response| response.raw_value);
        record_or_collect_error(db, &coin, internal_name, source, fetched, &mut readings, &mut errors).await;
    }

    // Cada fonte devolve o valor e quem respondeu (os de derivativos tentam
    // a Binance e caem pra Bybit; os demais têm fonte única).
    let mut direct: Vec<(&str, Result<crypto_sources::Sourced, AppError>)> = vec![
        ("funding_rate_7d", crypto_sources::fetch_funding_rate_7d(&coin).await),
        ("open_interest_change_30d", crypto_sources::fetch_open_interest_change_30d(&coin).await),
        ("long_short_ratio", crypto_sources::fetch_long_short_ratio(&coin).await),
        (
            "stablecoin_supply_change_30d",
            crypto_sources::fetch_stablecoin_supply_change_30d().await.map(|v| (v, "defillama")),
        ),
    ];
    // A Deribit só lista opções e DVOL de BTC e ETH: no ADA os dois ficam fora do
    // conjunto, e buscá-los só geraria erro a cada rodada.
    if wants("dvol") {
        direct.push(("dvol", crypto_sources::fetch_dvol(&coin).await.map(|v| (v, "deribit"))));
    }
    if wants("put_call_ratio") {
        direct.push((
            "put_call_ratio",
            crypto_sources::fetch_put_call_ratio(&coin).await.map(|v| (v, "deribit")),
        ));
    }
    if wants("staking_yield") {
        direct.push((
            "staking_yield",
            crypto_sources::fetch_staking_yield_real().await.map(|v| (v, "defillama+coinmetrics")),
        ));
    }
    // Market sentiment (lote 4): o Fear & Greed é do mercado todo (o mesmo valor
    // para as duas moedas) e a dominância do BTC só entra no score do ETH.
    direct.push((
        "fear_greed_index",
        client::fetch_fear_greed().await.map(|r| (f64::from(r.value), "alternative.me")),
    ));
    if wants("btc_dominance") {
        direct.push((
            "btc_dominance",
            crypto_sources::fetch_btc_dominance().await.map(|v| (v, "coingecko")),
        ));
    }
    for (internal_name, fetched) in direct {
        match fetched {
            Ok((raw_value, source)) => {
                record_or_collect_error(db, &coin, internal_name, source, Ok(raw_value), &mut readings, &mut errors)
                    .await;
            }
            Err(e) => errors.push(format!("{internal_name}: {e}")),
        }
    }

    if readings.is_empty() && !errors.is_empty() {
        return Err(AppError::CryptoSource(errors.join("; ")));
    }
    Ok(IndicatorCollection { readings, errors })
}

async fn record_or_collect_error(
    db: &DatabaseConnection,
    coin: &str,
    indicator: &str,
    source: &str,
    fetched: Result<f64, AppError>,
    readings: &mut Vec<crypto_indicators::Model>,
    errors: &mut Vec<String>,
) {
    let outcome = match fetched {
        Ok(raw_value) => record_crypto_indicator(db, coin, indicator, raw_value, source).await,
        Err(e) => Err(e),
    };
    match outcome {
        Ok(model) => readings.push(model),
        Err(e) => errors.push(format!("{indicator}: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Testes `#[ignore]` — mesma convenção de `finance_api::stocks::tests`:
// vão às fontes públicas de verdade (rede) e precisam do banco real de dev.
// Rodar com `cargo test --lib -- --ignored finance_api::crypto`.
// ---------------------------------------------------------------------------
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
        let result = collect_ticker(&db, "BTC").await.unwrap();
        assert!(result.price > 0.0);
        assert!(result.history_points > 0);
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_eth_indicators_writes_twenty_readings() {
        let db = dev_db().await;
        let collection = collect_indicators(&db, "ETH").await.unwrap();
        assert_eq!(collection.readings.len() + collection.errors.len(), 20);
        for reading in &collection.readings {
            assert!(matches!(reading.signal.as_str(), "GREEN" | "NEUTRAL" | "RED"));
            assert_eq!(reading.coin, "ETH");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_btc_indicators_writes_fourteen_readings() {
        let db = dev_db().await;
        let collection = collect_indicators(&db, "BTC").await.unwrap();
        assert_eq!(collection.readings.len() + collection.errors.len(), 14);
        for reading in &collection.readings {
            assert!(matches!(reading.signal.as_str(), "GREEN" | "NEUTRAL" | "RED"));
            assert_eq!(reading.coin, "BTC");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_ada_indicators_writes_ten_readings() {
        let db = dev_db().await;
        let collection = collect_indicators(&db, "ADA").await.unwrap();
        assert_eq!(collection.readings.len() + collection.errors.len(), 10);
        for reading in &collection.readings {
            assert!(matches!(reading.signal.as_str(), "GREEN" | "NEUTRAL" | "RED"));
            assert_eq!(reading.coin, "ADA");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_xrp_indicators_writes_nine_readings() {
        let db = dev_db().await;
        let collection = collect_indicators(&db, "XRP").await.unwrap();
        assert_eq!(collection.readings.len() + collection.errors.len(), 9);
        for reading in &collection.readings {
            assert!(matches!(reading.signal.as_str(), "GREEN" | "NEUTRAL" | "RED"));
            assert_eq!(reading.coin, "XRP");
        }
    }

    #[tokio::test]
    async fn only_coins_with_their_own_score_can_be_collected() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let err = collect_indicators(&db, "SOL").await.err().expect("SOL is rejected");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }
}
