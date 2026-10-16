// CoinGecko (dados de mercado de cripto, pública e sem chave) — porta de
// `lume/api/app/sources/cripto_coingecko.py`. Resolve o símbolo livre (`ETH`)
// pro id da CoinGecko e devolve a série diária de preços em USD; a última
// ponta é a cotação atual, então uma chamada serve de cotação e de histórico.
// O limite da API grátis é baixo (429 vira `ProviderUnavailable`), por isso o
// resultado fica em cache de 5 min na memória, compartilhado entre os dois usos.

use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{DateTime, NaiveDate};
use serde::Deserialize;

use crate::error::AppError;
use crate::finance_core::cache::{Cached, TtlCache};
use crate::finance_core::http;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "coingecko";
const BASE_URL: &str = "https://api.coingecko.com/api/v3";

const NVT_MA_WINDOW_DAYS: usize = 90;
const PRICE_HISTORY_DAYS: u32 = 365;
/// Cotação e histórico saem da mesma chamada.
const MARKET_TTL: Duration = Duration::from_secs(300);
/// O id de uma moeda quase nunca muda.
const RESOLUTION_TTL: Duration = Duration::from_secs(24 * 3600);

#[derive(Debug, Clone, PartialEq)]
pub struct CoinMarket {
    pub coin_id: String,
    pub name: String,
    /// Fechamento diário em USD, do mais antigo ao mais recente.
    pub prices: Vec<(NaiveDate, f64)>,
}

fn bad(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("CoinGecko: {detail}"))
}

// ---------------------------------------------------------------------------
// Parsing e cálculos puros
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    coins: Vec<SearchCoin>,
}

#[derive(Deserialize)]
struct SearchCoin {
    id: String,
    name: String,
    #[serde(default)]
    symbol: String,
    market_cap_rank: Option<u32>,
}

/// Só aceita símbolo idêntico (sem caixa); moedas diferentes dividem símbolo,
/// então entre as idênticas vence a de menor `market_cap_rank`. Nenhuma: `NoData`.
pub fn pick_coin(body: &str, symbol: &str) -> Result<(String, String), AppError> {
    let response: SearchResponse = serde_json::from_str(body).map_err(bad)?;
    response
        .coins
        .into_iter()
        .filter(|c| c.symbol.eq_ignore_ascii_case(symbol))
        .min_by_key(|c| c.market_cap_rank.unwrap_or(u32::MAX))
        .map(|c| (c.id, c.name))
        .ok_or_else(|| AppError::NoData(format!("CoinGecko: no coin with symbol '{symbol}'")))
}

#[derive(Deserialize)]
struct MarketChart {
    #[serde(default)]
    prices: Vec<(f64, f64)>,
    #[serde(default)]
    market_caps: Vec<(f64, f64)>,
    #[serde(default)]
    total_volumes: Vec<(f64, f64)>,
}

fn date_of_ms(ms: f64) -> Option<NaiveDate> {
    DateTime::from_timestamp_millis(ms as i64).map(|dt| dt.date_naive())
}

/// Um preço por dia. Perto da fronteira o dia pode vir duas vezes (a vela de
/// hoje ainda se formando): fica o último, em ordem cronológica.
pub fn parse_market_chart(body: &str) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let chart: MarketChart = serde_json::from_str(body).map_err(bad)?;
    let mut by_date: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for (ms, price) in chart.prices {
        by_date.insert(date_of_ms(ms).ok_or_else(|| bad("invalid timestamp"))?, price);
    }
    Ok(by_date.into_iter().collect())
}

/// NVT de hoje dividido pela média das 90 velas fechadas anteriores. "Volume"
/// aqui é volume negociado em corretoras (proxy), não o liquidado on-chain.
pub fn nvt_vs_ma90(body: &str) -> Result<f64, AppError> {
    let chart: MarketChart = serde_json::from_str(body).map_err(bad)?;
    let daily: Vec<f64> = chart
        .market_caps
        .iter()
        .zip(&chart.total_volumes)
        .map(|(&(_, cap), &(_, volume))| {
            if volume == 0.0 {
                Err(bad("zero volume in NVT series"))
            } else {
                Ok(cap / volume)
            }
        })
        .collect::<Result<_, _>>()?;
    let (today, previous) = daily.split_last().ok_or_else(|| bad("empty NVT series"))?;
    if previous.is_empty() {
        return Err(bad("not enough NVT history"));
    }
    let ma = previous.iter().sum::<f64>() / previous.len() as f64;
    if ma == 0.0 {
        return Err(bad("zero NVT moving average"));
    }
    Ok(today / ma)
}

// ---------------------------------------------------------------------------
// Fetch
// ---------------------------------------------------------------------------

pub async fn fetch_nvt_ratio_vs_ma90() -> Result<f64, AppError> {
    let url = format!(
        "{BASE_URL}/coins/ethereum/market_chart?vs_currency=usd&days={NVT_MA_WINDOW_DAYS}&interval=daily"
    );
    nvt_vs_ma90(&http::get_text("CoinGecko", &url, http::DEFAULT_TIMEOUT).await?)
}

static RESOLUTIONS: OnceLock<TtlCache<String, (String, String)>> = OnceLock::new();

async fn resolve_coin(symbol: &str) -> Result<(String, String), AppError> {
    let cached = RESOLUTIONS
        .get_or_init(TtlCache::new)
        .get_or_refresh(symbol.to_string(), RESOLUTION_TTL, || async {
            let url = format!("{BASE_URL}/search?query={symbol}");
            pick_coin(&http::get_text("CoinGecko", &url, http::DEFAULT_TIMEOUT).await?, symbol)
        })
        .await?;
    Ok(cached.value)
}

async fn fetch_market(symbol: &str) -> Result<CoinMarket, AppError> {
    let (coin_id, name) = resolve_coin(symbol).await?;
    let url = format!(
        "{BASE_URL}/coins/{coin_id}/market_chart?vs_currency=usd&days={PRICE_HISTORY_DAYS}&interval=daily"
    );
    let prices = parse_market_chart(&http::get_text("CoinGecko", &url, http::DEFAULT_TIMEOUT).await?)?;
    if prices.is_empty() {
        return Err(bad(format!("no price history for '{symbol}'")));
    }
    Ok(CoinMarket { coin_id, name, prices })
}

pub trait CryptoMarketProvider: DataProvider {
    /// `symbol` já em maiúsculas.
    fn market<'a>(&'a self, symbol: &'a str) -> BoxFuture<'a, Result<CoinMarket, AppError>>;
}

pub struct CoinGecko;

impl DataProvider for CoinGecko {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl CryptoMarketProvider for CoinGecko {
    fn market<'a>(&'a self, symbol: &'a str) -> BoxFuture<'a, Result<CoinMarket, AppError>> {
        Box::pin(fetch_market(symbol))
    }
}

fn providers() -> Vec<Box<dyn CryptoMarketProvider>> {
    vec![Box::new(CoinGecko)]
}

static MARKETS: OnceLock<TtlCache<String, Sourced<CoinMarket>>> = OnceLock::new();

/// Cotação (último ponto) e histórico diário de 365 dias de uma moeda, em USD.
pub async fn coin_market(symbol: &str) -> Result<Cached<Sourced<CoinMarket>>, AppError> {
    let symbol = symbol.to_uppercase();
    MARKETS
        .get_or_init(TtlCache::new)
        .get_or_refresh(symbol.clone(), MARKET_TTL, || async {
            let providers = providers();
            let attempts = providers.iter().map(|p| (p.id(), p.market(&symbol))).collect();
            cascade(&format!("crypto market '{symbol}'"), attempts).await
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn exact_symbol_with_best_rank_wins() {
        let body = r#"{"coins":[
            {"id":"ethereum-wormhole","name":"ETH (Wormhole)","symbol":"ETH","market_cap_rank":900},
            {"id":"ethereum","name":"Ethereum","symbol":"ETH","market_cap_rank":2},
            {"id":"ethereum-classic","name":"Ethereum Classic","symbol":"ETC","market_cap_rank":30},
            {"id":"unranked","name":"Unranked","symbol":"eth","market_cap_rank":null}
        ]}"#;
        assert_eq!(pick_coin(body, "ETH").unwrap(), ("ethereum".into(), "Ethereum".into()));
    }

    #[test]
    fn no_exact_symbol_is_no_data() {
        let body = r#"{"coins":[{"id":"ethereum-classic","name":"Ethereum Classic","symbol":"ETC","market_cap_rank":30}]}"#;
        assert!(matches!(pick_coin(body, "ETH"), Err(AppError::NoData(_))));
        assert!(matches!(pick_coin(r#"{"coins":[]}"#, "ETH"), Err(AppError::NoData(_))));
    }

    #[test]
    fn duplicate_day_keeps_the_last_price_and_output_is_sorted() {
        // 2026-10-15, 2026-10-16 (duas vezes: a vela de hoje ainda se formando) e 2026-10-14 fora de ordem.
        let body = r#"{"prices":[[1792022400000,100.0],[1792108800000,110.0],[1792108800000,111.5],[1791936000000,90.0]]}"#;
        assert_eq!(
            parse_market_chart(body).unwrap(),
            vec![(d(2026, 10, 14), 90.0), (d(2026, 10, 15), 100.0), (d(2026, 10, 16), 111.5)]
        );
    }

    #[test]
    fn nvt_compares_today_with_the_average_of_the_previous_days() {
        // NVT diário: 10, 20, 30, 40 → hoje 40, média anterior 20 → 2,0.
        let body = r#"{"market_caps":[[1,100],[2,200],[3,300],[4,400]],"total_volumes":[[1,10],[2,10],[3,10],[4,10]]}"#;
        assert!((nvt_vs_ma90(body).unwrap() - 40.0 / 20.0).abs() < 1e-12);
    }

    #[test]
    fn nvt_rejects_degenerate_series() {
        for body in [
            r#"{"market_caps":[[1,100]],"total_volumes":[[1,10]]}"#,
            r#"{"market_caps":[[1,100],[2,100]],"total_volumes":[[1,10],[2,0]]}"#,
            r#"{}"#,
        ] {
            assert!(matches!(nvt_vs_ma90(body), Err(AppError::ProviderUnavailable(_))), "{body}");
        }
    }

    // Ao vivo (rede): `cargo test --lib -- --ignored finance_core::coingecko`.
    #[tokio::test]
    #[ignore]
    async fn live_btc_market_and_nvt() {
        let btc = coin_market("btc").await.unwrap();
        assert_eq!(btc.value.source, SOURCE);
        assert_eq!(btc.value.value.coin_id, "bitcoin");
        assert!(btc.value.value.prices.len() > 300);
        let (date, price) = *btc.value.value.prices.last().unwrap();
        println!("BTC {date} {price}");
        assert!(price > 1000.0);
        assert!(coin_market("BTC").await.unwrap().cached);
        let nvt = fetch_nvt_ratio_vs_ma90().await.unwrap();
        println!("NVT vs MA90 {nvt}");
        assert!(nvt > 0.1 && nvt < 10.0);
        assert!(matches!(coin_market("ZZZNOPE9").await.unwrap_err(), AppError::NoData(_)));
    }
}
