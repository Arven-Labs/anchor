// Fase 31 — fontes públicas de indicadores cripto coletadas direto pelo
// Anchor (sem passar pelo Lume): derivativos de ETH na Binance Futures e o
// staking yield real (APY do stETH na DefiLlama menos a inflação bruta do
// ETH no CoinMetrics Community). Todas sem chave. Os cálculos são funções
// puras (testadas com JSON copiado das respostas reais); os `fetch_*` são
// finos.
//
// Binance restringe por região: se uma chamada falhar, o erro diz qual
// fonte caiu, e o coletor segue com os demais indicadores.

use std::time::Duration;

use serde::Deserialize;

use crate::error::AppError;

const BINANCE_FAPI: &str = "https://fapi.binance.com";
const DEFILLAMA_YIELDS: &str = "https://yields.llama.fi";
const COINMETRICS_COMMUNITY: &str = "https://community-api.coinmetrics.io/v4";
/// Pool stETH (Lido, Ethereum) na DefiLlama — o id é estável.
const STETH_POOL_ID: &str = "747c1d2a-c668-4682-b9f9-296708a3dd90";

/// Funding da Binance sai a cada 8h: 3 por dia, 7 dias.
const FUNDING_PRINTS_PER_DAY: f64 = 3.0;
const FUNDING_WINDOW: usize = 21;
/// A Binance só guarda ~30 dias de histórico de OI e de long/short.
const HISTORY_DAYS: usize = 30;
const LONG_SHORT_WINDOW: usize = 7;

// ---------------------------------------------------------------------------
// Cálculos puros
// ---------------------------------------------------------------------------

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

/// Média das taxas de funding por período de 8h, anualizada em percentual
/// (ex.: 0,0001 por 8h = 10,95% ao ano).
pub fn funding_annualized_pct(rates: &[f64]) -> Option<f64> {
    mean(rates).map(|avg| avg * FUNDING_PRINTS_PER_DAY * 365.0 * 100.0)
}

/// Variação percentual entre o ponto mais antigo e o mais recente.
pub fn pct_change(oldest: f64, newest: f64) -> Option<f64> {
    if oldest.abs() < f64::EPSILON {
        return None;
    }
    Some((newest / oldest - 1.0) * 100.0)
}

/// Yield real do staking: APY nominal menos a inflação bruta anual do ETH
/// (emissão diária × 365 ÷ oferta circulante), tudo em percentual.
pub fn real_staking_yield(apy_pct: f64, daily_issuance: f64, supply: f64) -> Option<f64> {
    if supply <= 0.0 {
        return None;
    }
    Some(apy_pct - daily_issuance * 365.0 / supply * 100.0)
}

// ---------------------------------------------------------------------------
// Parsers (JSON → números)
// ---------------------------------------------------------------------------

fn source_err(source: &str, detail: impl std::fmt::Display) -> AppError {
    AppError::CryptoSource(format!("{source}: {detail}"))
}

fn parse_f64(source: &str, field: &str, value: &str) -> Result<f64, AppError> {
    value
        .parse::<f64>()
        .map_err(|_| source_err(source, format!("invalid number in '{field}': {value}")))
}

#[derive(Deserialize)]
struct FundingRow {
    #[serde(rename = "fundingRate")]
    funding_rate: String,
}

#[derive(Deserialize)]
struct OpenInterestRow {
    #[serde(rename = "sumOpenInterest")]
    sum_open_interest: String,
}

#[derive(Deserialize)]
struct LongShortRow {
    #[serde(rename = "longShortRatio")]
    long_short_ratio: String,
}

pub fn parse_funding_rates(body: &str) -> Result<Vec<f64>, AppError> {
    let rows: Vec<FundingRow> = serde_json::from_str(body)?;
    rows.iter().map(|r| parse_f64("Binance funding", "fundingRate", &r.funding_rate)).collect()
}

/// Open interest em unidades do ativo (ETH), do mais antigo ao mais recente.
pub fn parse_open_interest(body: &str) -> Result<Vec<f64>, AppError> {
    let rows: Vec<OpenInterestRow> = serde_json::from_str(body)?;
    rows.iter()
        .map(|r| parse_f64("Binance open interest", "sumOpenInterest", &r.sum_open_interest))
        .collect()
}

pub fn parse_long_short(body: &str) -> Result<Vec<f64>, AppError> {
    let rows: Vec<LongShortRow> = serde_json::from_str(body)?;
    rows.iter()
        .map(|r| parse_f64("Binance long/short", "longShortRatio", &r.long_short_ratio))
        .collect()
}

#[derive(Deserialize)]
struct YieldChart {
    data: Vec<YieldPoint>,
}

#[derive(Deserialize)]
struct YieldPoint {
    apy: Option<f64>,
}

/// APY mais recente da série (último ponto com valor).
pub fn parse_latest_apy(body: &str) -> Result<f64, AppError> {
    let chart: YieldChart = serde_json::from_str(body)?;
    chart
        .data
        .iter()
        .rev()
        .find_map(|p| p.apy)
        .ok_or_else(|| source_err("DefiLlama yields", "no APY in the stETH pool series"))
}

#[derive(Deserialize)]
struct CoinMetricsResponse {
    data: Vec<CoinMetricsRow>,
}

#[derive(Deserialize)]
struct CoinMetricsRow {
    #[serde(rename = "IssTotNtv")]
    issuance: String,
    #[serde(rename = "SplyCur")]
    supply: String,
}

/// `(emissão diária em ETH, oferta circulante)` do dia mais recente.
pub fn parse_issuance_and_supply(body: &str) -> Result<(f64, f64), AppError> {
    let response: CoinMetricsResponse = serde_json::from_str(body)?;
    let row = response
        .data
        .last()
        .ok_or_else(|| source_err("CoinMetrics", "empty IssTotNtv/SplyCur series"))?;
    Ok((
        parse_f64("CoinMetrics", "IssTotNtv", &row.issuance)?,
        parse_f64("CoinMetrics", "SplyCur", &row.supply)?,
    ))
}

// ---------------------------------------------------------------------------
// Fetch
// ---------------------------------------------------------------------------

async fn get_text(source: &str, url: &str) -> Result<String, AppError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?;
    let response = client.get(url).send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        let snippet: String = body.chars().take(200).collect();
        return Err(source_err(source, format!("HTTP {status} — {snippet}")));
    }
    Ok(body)
}

/// Funding médio de 7 dias do ETHUSDT perpétuo, anualizado em %.
pub async fn fetch_funding_rate_7d() -> Result<f64, AppError> {
    let url = format!("{BINANCE_FAPI}/fapi/v1/fundingRate?symbol=ETHUSDT&limit={FUNDING_WINDOW}");
    let rates = parse_funding_rates(&get_text("Binance funding", &url).await?)?;
    funding_annualized_pct(&rates).ok_or_else(|| source_err("Binance funding", "no funding data"))
}

/// Variação do open interest em ETH nos últimos ~30 dias, em %.
pub async fn fetch_open_interest_change_30d() -> Result<f64, AppError> {
    let url = format!(
        "{BINANCE_FAPI}/futures/data/openInterestHist?symbol=ETHUSDT&period=1d&limit={HISTORY_DAYS}"
    );
    let series = parse_open_interest(&get_text("Binance open interest", &url).await?)?;
    if series.len() < 2 {
        return Err(source_err("Binance open interest", "not enough history"));
    }
    pct_change(series[0], series[series.len() - 1])
        .ok_or_else(|| source_err("Binance open interest", "zero open interest at the start"))
}

/// Média de 7 dias da razão long/short de contas do ETHUSDT.
pub async fn fetch_long_short_ratio() -> Result<f64, AppError> {
    let url = format!(
        "{BINANCE_FAPI}/futures/data/globalLongShortAccountRatio?symbol=ETHUSDT&period=1d&limit={LONG_SHORT_WINDOW}"
    );
    let ratios = parse_long_short(&get_text("Binance long/short", &url).await?)?;
    mean(&ratios).ok_or_else(|| source_err("Binance long/short", "no data"))
}

/// Staking yield real do ETH: APY do stETH menos a inflação bruta.
pub async fn fetch_staking_yield_real() -> Result<f64, AppError> {
    let apy_url = format!("{DEFILLAMA_YIELDS}/chart/{STETH_POOL_ID}");
    let apy = parse_latest_apy(&get_text("DefiLlama yields", &apy_url).await?)?;

    let cm_url = format!(
        "{COINMETRICS_COMMUNITY}/timeseries/asset-metrics?assets=eth&metrics=IssTotNtv,SplyCur&frequency=1d&limit_per_asset=1&paging_from=end"
    );
    let (issuance, supply) = parse_issuance_and_supply(&get_text("CoinMetrics", &cm_url).await?)?;
    real_staking_yield(apy, issuance, supply)
        .ok_or_else(|| source_err("CoinMetrics", "circulating supply is zero"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trechos copiados das respostas reais de 2026-10-16.
    const FUNDING: &str = r#"[{"symbol":"ETHUSDT","fundingTime":1792080000000,"fundingRate":"0.00005355","markPrice":"2439.89","rateType":"Regular"},{"symbol":"ETHUSDT","fundingTime":1792108800001,"fundingRate":"0.00007466","markPrice":"2441.05","rateType":"Regular"},{"symbol":"ETHUSDT","fundingTime":1792137600001,"fundingRate":"0.00004648","markPrice":"2454.27","rateType":"Regular"}]"#;
    const OPEN_INTEREST: &str = r#"[{"symbol":"ETHUSDT","sumOpenInterest":"2270359.16000000","sumOpenInterestValue":"5668837083.01","CMCCirculatingSupply":"122127761.09","timestamp":1791936000000},{"symbol":"ETHUSDT","sumOpenInterest":"2258306.77400000","sumOpenInterestValue":"5513177087.62","CMCCirculatingSupply":"122133648.45","timestamp":1792108800000}]"#;
    const LONG_SHORT: &str = r#"[{"symbol":"ETHUSDT","longAccount":"0.7267","longShortRatio":"2.6590","shortAccount":"0.2733","timestamp":1791936000000},{"symbol":"ETHUSDT","longAccount":"0.7252","longShortRatio":"2.6390","shortAccount":"0.2748","timestamp":1792108800000}]"#;
    const YIELD_CHART: &str = r#"{"status":"success","data":[{"timestamp":"2026-10-15T23:01:26.957Z","tvlUsd":23771578172,"apy":2.255,"apyBase":2.255,"apyReward":null},{"timestamp":"2026-10-16T12:01:20.741Z","tvlUsd":23855899243,"apy":2.255,"apyBase":2.255,"apyReward":null}]}"#;
    const ISSUANCE: &str = r#"{"data":[{"asset":"eth","time":"2026-10-14T00:00:00.000000000Z","IssTotNtv":"2979.961923327","SplyCur":"122130969.817571701831258692"},{"asset":"eth","time":"2026-10-15T00:00:00.000000000Z","IssTotNtv":"2979.616171783","SplyCur":"122133892.772935516304718648"}]}"#;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn funding_is_annualized_from_the_8h_average() {
        // 0,0001 por 8h × 3 × 365 × 100 = 10,95% ao ano.
        assert!(close(funding_annualized_pct(&[0.0001, 0.0001]).unwrap(), 10.95));
        assert!(funding_annualized_pct(&[]).is_none());
    }

    #[test]
    fn negative_funding_stays_negative() {
        assert!(funding_annualized_pct(&[-0.0001, -0.0003]).unwrap() < 0.0);
    }

    #[test]
    fn pct_change_guards_zero_start() {
        assert!(close(pct_change(200.0, 250.0).unwrap(), 25.0));
        assert!(close(pct_change(200.0, 150.0).unwrap(), -25.0));
        assert!(pct_change(0.0, 10.0).is_none());
    }

    #[test]
    fn real_yield_subtracts_gross_inflation() {
        // 2,255% de APY − (2.980 ETH/dia × 365 ÷ 122,13M × 100) = 2,255 − 0,8905
        // ≈ 1,3645.
        let real = real_staking_yield(2.255, 2979.616171783, 122133892.7729).unwrap();
        assert!(real > 1.364 && real < 1.365);
        assert!(real_staking_yield(2.0, 100.0, 0.0).is_none());
    }

    #[test]
    fn parses_real_binance_funding_payload() {
        let rates = parse_funding_rates(FUNDING).unwrap();
        assert_eq!(rates.len(), 3);
        assert!(close(rates[0], 0.00005355));
    }

    #[test]
    fn parses_open_interest_in_coin_units() {
        let oi = parse_open_interest(OPEN_INTEREST).unwrap();
        assert!(close(oi[0], 2270359.16));
        // Variação de −0,53% entre os dois pontos.
        assert!(pct_change(oi[0], oi[1]).unwrap() < 0.0);
    }

    #[test]
    fn parses_long_short_ratio() {
        let ratios = parse_long_short(LONG_SHORT).unwrap();
        assert!(close(mean(&ratios).unwrap(), 2.649));
    }

    #[test]
    fn parses_latest_apy_skipping_missing_values() {
        assert!(close(parse_latest_apy(YIELD_CHART).unwrap(), 2.255));
        assert!(parse_latest_apy(r#"{"data":[{"apy":null}]}"#).is_err());
    }

    #[test]
    fn parses_latest_issuance_and_supply() {
        let (issuance, supply) = parse_issuance_and_supply(ISSUANCE).unwrap();
        assert!(close(issuance, 2979.616171783));
        assert!(supply > 122_000_000.0);
        assert!(parse_issuance_and_supply(r#"{"data":[]}"#).is_err());
    }

    #[test]
    fn invalid_number_names_the_source() {
        let err = parse_funding_rates(r#"[{"fundingRate":"abc"}]"#).unwrap_err();
        assert!(err.to_string().contains("Binance funding"));
    }

    // Ao vivo: `cargo test --lib -- --ignored crypto_sources`.
    #[tokio::test]
    #[ignore]
    async fn live_sources_return_plausible_values() {
        let funding = fetch_funding_rate_7d().await.unwrap();
        assert!(funding.abs() < 200.0);
        let oi = fetch_open_interest_change_30d().await.unwrap();
        assert!(oi.abs() < 500.0);
        let ls = fetch_long_short_ratio().await.unwrap();
        assert!(ls > 0.0 && ls < 20.0);
        let staking = fetch_staking_yield_real().await.unwrap();
        assert!(staking > -5.0 && staking < 15.0);
    }
}
