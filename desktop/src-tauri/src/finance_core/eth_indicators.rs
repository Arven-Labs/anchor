// Indicadores do ciclo do ETH que o Lume servia em `/v1/crypto/eth-indicators/{code}`
// (portados de `cripto_defillama.py`, `cripto_ultrasound.py`, `cripto_coinmetrics.py`
// e do NVT de `cripto_coingecko.py`). Só devolvem o valor bruto: a classificação
// verde/vermelho é regra do Anchor (`domain::crypto_score`). Os outros 7
// indicadores do score já vêm direto de `crypto_sources` (Fase 31).
//
// Cada fonte é um `EthIndicatorProvider` que declara quais códigos atende;
// `indicator` só consulta quem atende o código pedido.

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::error::AppError;
use crate::finance_core::coingecko;
use crate::finance_core::http;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

const DEFILLAMA: &str = "https://api.llama.fi";
const ULTRASOUND: &str = "https://ultrasound.money/api/v2/fees";
const COINMETRICS: &str = "https://community-api.coinmetrics.io/v4";

/// Séries diárias: "30 dias atrás" é contar 30 posições a partir do fim.
const TREND_LOOKBACK_DAYS: usize = 30;
const SUPPLY_WINDOW_DAYS: f64 = 30.0;
const DAYS_IN_YEAR: f64 = 365.0;
const PUELL_MA_WINDOW_DAYS: usize = 365;
const NETFLOW_WINDOW_DAYS: usize = 30;
/// Teto da página do plano Community do CoinMetrics (acima disso, HTTP 400).
const FULL_HISTORY_PAGE_SIZE: usize = 10_000;

fn bad(source: &str, detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("{source}: {detail}"))
}

// ---------------------------------------------------------------------------
// DefiLlama — tvl-trend
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TvlPoint {
    tvl: f64,
}

/// Variação % do TVL entre hoje e ~30 dias atrás.
pub fn tvl_trend_mom(body: &str) -> Result<f64, AppError> {
    let history: Vec<TvlPoint> = serde_json::from_str(body).map_err(|e| bad("DefiLlama TVL", e))?;
    pct_change_back(history.iter().map(|p| p.tvl).collect(), "DefiLlama TVL")
}

/// `(último − o de 30 posições atrás) ÷ o de 30 atrás × 100`.
fn pct_change_back(values: Vec<f64>, source: &str) -> Result<f64, AppError> {
    if values.len() <= TREND_LOOKBACK_DAYS {
        return Err(bad(source, "not enough history"));
    }
    let latest = values[values.len() - 1];
    let previous = values[values.len() - 1 - TREND_LOOKBACK_DAYS];
    if previous == 0.0 {
        return Err(bad(source, "zero baseline"));
    }
    Ok((latest - previous) / previous * 100.0)
}

// ---------------------------------------------------------------------------
// ultrasound.money — net-issuance, fees-vs-emission
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SupplyPoint {
    supply: f64,
}

#[derive(Deserialize)]
struct SupplyOverTime {
    d30: Vec<SupplyPoint>,
}

fn supply_window(body: &str) -> Result<(f64, f64), AppError> {
    let supply: SupplyOverTime = serde_json::from_str(body).map_err(|e| bad("ultrasound.money supply", e))?;
    match (supply.d30.first(), supply.d30.last()) {
        (Some(first), Some(last)) => Ok((first.supply, last.supply)),
        _ => Err(bad("ultrasound.money supply", "empty d30 window")),
    }
}

/// Variação % da oferta de ETH em 30 dias, anualizada. Positivo: oferta
/// crescendo (emissão > queima); negativo: "ultra sound money".
pub fn net_issuance_annualized_pct(supply_body: &str) -> Result<f64, AppError> {
    let (start, end) = supply_window(supply_body)?;
    if start == 0.0 {
        return Err(bad("ultrasound.money supply", "zero supply"));
    }
    Ok((end - start) / start * 100.0 * (DAYS_IN_YEAR / SUPPLY_WINDOW_DAYS))
}

/// Queima (taxas) ÷ emissão bruta de ETH em 30 dias. A API só dá a oferta
/// líquida (emissão − queima) e a queima, então emissão = variação líquida + queima.
pub fn fees_vs_emission_ratio(supply_body: &str, burn_body: &str) -> Result<f64, AppError> {
    let (start, end) = supply_window(supply_body)?;
    let burn: Value = serde_json::from_str(burn_body).map_err(|e| bad("ultrasound.money burn", e))?;
    let burn_eth = burn["d30"]["sum"]["eth"]
        .as_f64()
        .ok_or_else(|| bad("ultrasound.money burn", "missing d30.sum.eth"))?;
    let gross = (end - start) + burn_eth;
    if gross == 0.0 {
        return Err(bad("ultrasound.money", "zero gross issuance"));
    }
    Ok(burn_eth / gross)
}

// ---------------------------------------------------------------------------
// CoinMetrics Community — mvrv-z-score, puell-multiple, exchange-netflow, active-addresses-trend
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct MetricsResponse {
    data: Vec<Map<String, Value>>,
}

/// Coluna `metric` de todas as linhas, como número (o CoinMetrics manda tudo
/// como string pra preservar precisão), do mais antigo ao mais recente.
pub fn metric_column(body: &str, metric: &str) -> Result<Vec<f64>, AppError> {
    let response: MetricsResponse = serde_json::from_str(body).map_err(|e| bad("CoinMetrics", e))?;
    response
        .data
        .iter()
        .map(|row| {
            let raw = row.get(metric).ok_or_else(|| bad("CoinMetrics", format!("row missing '{metric}'")))?;
            match raw {
                Value::String(s) => s.parse::<f64>().map_err(|_| bad("CoinMetrics", format!("invalid '{metric}': {s}"))),
                Value::Number(n) => n.as_f64().ok_or_else(|| bad("CoinMetrics", format!("invalid '{metric}'"))),
                _ => Err(bad("CoinMetrics", format!("invalid '{metric}'"))),
            }
        })
        .collect()
}

/// MVRV Z-Score: `(MarketCap − RealizedCap) ÷ desvio-padrão(MarketCap)`. O
/// `CapRealUSD` é bloqueado no plano grátis, então o RealizedCap sai de
/// `MarketCap ÷ CapMVRVCur`.
pub fn mvrv_z_score(market_caps: &[f64], mvrv: &[f64]) -> Result<f64, AppError> {
    if market_caps.is_empty() || market_caps.len() != mvrv.len() {
        return Err(bad("CoinMetrics MVRV Z-Score", "mismatched or empty series"));
    }
    if mvrv.iter().any(|&m| m == 0.0) {
        return Err(bad("CoinMetrics MVRV Z-Score", "zero MVRV"));
    }
    let n = market_caps.len() as f64;
    let mean = market_caps.iter().sum::<f64>() / n;
    let stddev = (market_caps.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / n).sqrt(); // populacional
    if stddev == 0.0 {
        return Err(bad("CoinMetrics MVRV Z-Score", "zero market cap stddev"));
    }
    let last = market_caps.len() - 1;
    let realized_last = market_caps[last] / mvrv[last];
    Ok((market_caps[last] - realized_last) / stddev)
}

/// Puell Multiple: emissão bruta de hoje (USD) ÷ média móvel de 365 dias.
pub fn puell_multiple(issuance_usd: &[f64]) -> Result<f64, AppError> {
    if issuance_usd.len() < PUELL_MA_WINDOW_DAYS {
        return Err(bad("CoinMetrics Puell Multiple", "not enough issuance history"));
    }
    let window = &issuance_usd[issuance_usd.len() - PUELL_MA_WINDOW_DAYS..];
    let ma = window.iter().sum::<f64>() / PUELL_MA_WINDOW_DAYS as f64;
    if ma == 0.0 {
        return Err(bad("CoinMetrics Puell Multiple", "zero issuance moving average"));
    }
    Ok(issuance_usd[issuance_usd.len() - 1] / ma)
}

/// Variação % de endereços ativos entre hoje e ~30 dias atrás.
pub fn active_addresses_trend_mom(active: Vec<f64>) -> Result<f64, AppError> {
    pct_change_back(active, "CoinMetrics active addresses")
}

/// `Σ(entrada − saída) ÷ Σ(entrada + saída)` nos últimos 30 dias, em [−1, 1]:
/// negativo = saída líquida (acumulação), positivo = entrada líquida (pressão
/// vendedora). Uma razão suaviza o ruído dia a dia desses números "flash".
pub fn exchange_netflow_ratio(inflow: &[f64], outflow: &[f64]) -> Result<f64, AppError> {
    let tail = |v: &[f64]| v[v.len().saturating_sub(NETFLOW_WINDOW_DAYS)..].iter().sum::<f64>();
    let (inflow, outflow) = (tail(inflow), tail(outflow));
    let total = inflow + outflow;
    if total == 0.0 {
        return Err(bad("CoinMetrics exchange netflow", "zero total flow"));
    }
    Ok((inflow - outflow) / total)
}

fn metrics_url(metrics: &[&str], page_size: usize) -> String {
    format!(
        "{COINMETRICS}/timeseries/asset-metrics?assets=eth&metrics={}&frequency=1d&page_size={page_size}",
        metrics.join(",")
    )
}

async fn coinmetrics(metrics: &[&str], page_size: usize) -> Result<String, AppError> {
    http::get_text("CoinMetrics", &metrics_url(metrics, page_size), http::DEFAULT_TIMEOUT).await
}

// ---------------------------------------------------------------------------
// Providers
// ---------------------------------------------------------------------------

pub trait EthIndicatorProvider: DataProvider {
    /// Códigos da Finance API que este provider atende (`tvl-trend`, ...).
    fn supports(&self, code: &str) -> bool;
    fn indicator<'a>(&'a self, code: &'a str) -> BoxFuture<'a, Result<f64, AppError>>;
}

struct DefiLlama;
struct Ultrasound;
struct CoinGeckoNvt;
struct CoinMetrics;

impl DataProvider for DefiLlama {
    fn id(&self) -> &'static str {
        "defillama"
    }
}
impl DataProvider for Ultrasound {
    fn id(&self) -> &'static str {
        "ultrasound.money"
    }
}
impl DataProvider for CoinGeckoNvt {
    fn id(&self) -> &'static str {
        coingecko::SOURCE
    }
}
impl DataProvider for CoinMetrics {
    fn id(&self) -> &'static str {
        "coinmetrics"
    }
}

impl EthIndicatorProvider for DefiLlama {
    fn supports(&self, code: &str) -> bool {
        code == "tvl-trend"
    }
    fn indicator<'a>(&'a self, _code: &'a str) -> BoxFuture<'a, Result<f64, AppError>> {
        Box::pin(async {
            let url = format!("{DEFILLAMA}/v2/historicalChainTvl/Ethereum");
            tvl_trend_mom(&http::get_text("DefiLlama", &url, http::DEFAULT_TIMEOUT).await?)
        })
    }
}

impl EthIndicatorProvider for Ultrasound {
    fn supports(&self, code: &str) -> bool {
        matches!(code, "net-issuance" | "fees-vs-emission")
    }
    fn indicator<'a>(&'a self, code: &'a str) -> BoxFuture<'a, Result<f64, AppError>> {
        Box::pin(async move {
            let supply =
                http::get_text("ultrasound.money", &format!("{ULTRASOUND}/supply-over-time"), http::DEFAULT_TIMEOUT)
                    .await?;
            if code == "net-issuance" {
                return net_issuance_annualized_pct(&supply);
            }
            let burn =
                http::get_text("ultrasound.money", &format!("{ULTRASOUND}/burn-sums"), http::DEFAULT_TIMEOUT).await?;
            fees_vs_emission_ratio(&supply, &burn)
        })
    }
}

impl EthIndicatorProvider for CoinGeckoNvt {
    fn supports(&self, code: &str) -> bool {
        code == "nvt-ratio"
    }
    fn indicator<'a>(&'a self, _code: &'a str) -> BoxFuture<'a, Result<f64, AppError>> {
        Box::pin(coingecko::fetch_nvt_ratio_vs_ma90())
    }
}

impl EthIndicatorProvider for CoinMetrics {
    fn supports(&self, code: &str) -> bool {
        matches!(code, "mvrv-z-score" | "puell-multiple" | "exchange-netflow" | "active-addresses-trend")
    }
    fn indicator<'a>(&'a self, code: &'a str) -> BoxFuture<'a, Result<f64, AppError>> {
        Box::pin(async move {
            match code {
                "mvrv-z-score" => {
                    let body = coinmetrics(&["CapMrktCurUSD", "CapMVRVCur"], FULL_HISTORY_PAGE_SIZE).await?;
                    mvrv_z_score(&metric_column(&body, "CapMrktCurUSD")?, &metric_column(&body, "CapMVRVCur")?)
                }
                "puell-multiple" => {
                    let body = coinmetrics(&["IssTotUSD"], FULL_HISTORY_PAGE_SIZE).await?;
                    puell_multiple(&metric_column(&body, "IssTotUSD")?)
                }
                "active-addresses-trend" => {
                    let body = coinmetrics(&["AdrActCnt"], TREND_LOOKBACK_DAYS + 5).await?;
                    active_addresses_trend_mom(metric_column(&body, "AdrActCnt")?)
                }
                _ => {
                    let body = coinmetrics(&["FlowInExUSD", "FlowOutExUSD"], NETFLOW_WINDOW_DAYS + 5).await?;
                    exchange_netflow_ratio(
                        &metric_column(&body, "FlowInExUSD")?,
                        &metric_column(&body, "FlowOutExUSD")?,
                    )
                }
            }
        })
    }
}

fn providers() -> Vec<Box<dyn EthIndicatorProvider>> {
    vec![Box::new(DefiLlama), Box::new(Ultrasound), Box::new(CoinGeckoNvt), Box::new(CoinMetrics)]
}

/// Valor bruto do indicador `code` (como na Finance API). Código fora do
/// catálogo é `NoData`.
pub async fn indicator(code: &str) -> Result<Sourced<f64>, AppError> {
    let providers = providers();
    let attempts: Vec<_> =
        providers.iter().filter(|p| p.supports(code)).map(|p| (p.id(), p.indicator(code))).collect();
    if attempts.is_empty() {
        return Err(AppError::NoData(format!("unknown ETH indicator '{code}'")));
    }
    cascade(&format!("ETH indicator '{code}'"), attempts).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(metric: &str, values: &[f64]) -> String {
        let data: Vec<String> =
            values.iter().map(|v| format!(r#"{{"asset":"eth","time":"t","{metric}":"{v}"}}"#)).collect();
        format!(r#"{{"data":[{}]}}"#, data.join(","))
    }

    #[test]
    fn tvl_compares_with_thirty_positions_back() {
        let mut tvl: Vec<String> = (0..31).map(|i| format!(r#"{{"date":{i},"tvl":{}}}"#, 100 + i)).collect();
        tvl.push(r#"{"date":99,"tvl":200}"#.into()); // 32 pontos: o de 30 atrás é o índice 1 (101)
        let body = format!("[{}]", tvl.join(","));
        assert!((tvl_trend_mom(&body).unwrap() - (200.0 - 101.0) / 101.0 * 100.0).abs() < 1e-9);
    }

    #[test]
    fn tvl_needs_more_than_thirty_points_and_a_nonzero_baseline() {
        assert!(matches!(tvl_trend_mom(r#"[{"date":1,"tvl":1}]"#), Err(AppError::ProviderUnavailable(_))));
        let zeros: Vec<String> = (0..31).map(|i| format!(r#"{{"date":{i},"tvl":0}}"#)).collect();
        assert!(tvl_trend_mom(&format!("[{}]", zeros.join(","))).is_err());
    }

    #[test]
    fn net_issuance_is_annualized_from_the_thirty_day_window() {
        // Oferta 100 → 100,1 em 30 dias = +0,1%; × 365/30.
        let body = r#"{"d30":[{"supply":100.0},{"supply":100.05},{"supply":100.1}]}"#;
        assert!((net_issuance_annualized_pct(body).unwrap() - 0.1 * 365.0 / 30.0).abs() < 1e-9);
        let deflation = r#"{"d30":[{"supply":100.0},{"supply":99.0}]}"#;
        assert!(net_issuance_annualized_pct(deflation).unwrap() < 0.0);
        assert!(net_issuance_annualized_pct(r#"{"d30":[]}"#).is_err());
    }

    #[test]
    fn fees_vs_emission_reconstructs_gross_issuance() {
        // Oferta líquida +10 ETH, queima 30 ETH → emissão bruta 40; razão 30/40.
        let supply = r#"{"d30":[{"supply":1000.0},{"supply":1010.0}]}"#;
        let burn = r#"{"d30":{"sum":{"eth":30.0,"usd":1}}}"#;
        assert!((fees_vs_emission_ratio(supply, burn).unwrap() - 0.75).abs() < 1e-12);
        assert!(fees_vs_emission_ratio(supply, r#"{"d30":{}}"#).is_err());
    }

    #[test]
    fn metric_column_reads_string_numbers_and_rejects_gaps() {
        assert_eq!(metric_column(&rows("AdrActCnt", &[1.0, 2.5]), "AdrActCnt").unwrap(), vec![1.0, 2.5]);
        assert!(metric_column(&rows("AdrActCnt", &[1.0]), "Other").is_err());
        assert!(metric_column(r#"{"data":[{"AdrActCnt":null}]}"#, "AdrActCnt").is_err());
    }

    #[test]
    fn mvrv_z_score_uses_population_stddev_and_the_derived_realized_cap() {
        // Caps 1,2,3,4: média 2,5; desvio populacional √1,25. Último: cap 4, MVRV 2 → realizado 2.
        let z = mvrv_z_score(&[1.0, 2.0, 3.0, 4.0], &[1.0, 1.0, 1.0, 2.0]).unwrap();
        assert!((z - (4.0 - 2.0) / 1.25f64.sqrt()).abs() < 1e-12, "{z}");
        assert!(mvrv_z_score(&[5.0, 5.0], &[1.0, 1.0]).is_err()); // desvio zero
        assert!(mvrv_z_score(&[1.0, 2.0], &[1.0, 0.0]).is_err());
        assert!(mvrv_z_score(&[], &[]).is_err());
    }

    #[test]
    fn puell_is_today_over_the_365_day_average() {
        let mut issuance = vec![10.0; 364];
        issuance.push(20.0); // média = (364·10 + 20)/365
        let expected = 20.0 / ((364.0 * 10.0 + 20.0) / 365.0);
        assert!((puell_multiple(&issuance).unwrap() - expected).abs() < 1e-12);
        assert!(puell_multiple(&issuance[..300]).is_err());
        assert!(puell_multiple(&vec![0.0; 365]).is_err());
    }

    #[test]
    fn active_addresses_trend_looks_thirty_positions_back() {
        let mut v: Vec<f64> = vec![100.0; 35];
        v[4] = 50.0; // 35 pontos: o de 30 atrás é o índice 4
        let last = v.len() - 1;
        v[last] = 75.0;
        assert!((active_addresses_trend_mom(v).unwrap() - 50.0).abs() < 1e-12);
        assert!(active_addresses_trend_mom(vec![1.0; 10]).is_err());
    }

    #[test]
    fn netflow_ratio_only_counts_the_last_thirty_days() {
        // 35 dias: os 5 primeiros (saída enorme) ficam fora da janela.
        let mut inflow = vec![0.0; 5];
        inflow.extend(vec![30.0; 30]);
        let mut outflow = vec![1000.0; 5];
        outflow.extend(vec![10.0; 30]);
        assert!((exchange_netflow_ratio(&inflow, &outflow).unwrap() - (900.0 - 300.0) / 1200.0).abs() < 1e-12);
        assert!(exchange_netflow_ratio(&[0.0], &[0.0]).is_err());
    }

    #[tokio::test]
    async fn unknown_code_is_no_data() {
        assert!(matches!(indicator("does-not-exist").await.unwrap_err(), AppError::NoData(_)));
    }

    #[test]
    fn every_catalog_code_has_exactly_one_provider() {
        let codes = [
            "tvl-trend",
            "net-issuance",
            "fees-vs-emission",
            "nvt-ratio",
            "mvrv-z-score",
            "puell-multiple",
            "exchange-netflow",
            "active-addresses-trend",
        ];
        let providers = providers();
        for code in codes {
            assert_eq!(providers.iter().filter(|p| p.supports(code)).count(), 1, "{code}");
        }
    }

    // Ao vivo (rede): `cargo test --lib -- --ignored finance_core::eth_indicators`.
    #[tokio::test]
    #[ignore]
    async fn live_all_eight_indicators() {
        for code in [
            "tvl-trend",
            "net-issuance",
            "fees-vs-emission",
            "nvt-ratio",
            "mvrv-z-score",
            "puell-multiple",
            "exchange-netflow",
            "active-addresses-trend",
        ] {
            let got = indicator(code).await.unwrap_or_else(|e| panic!("{code}: {e}"));
            println!("{code} = {} ({})", got.value, got.source);
            assert!(got.value.is_finite(), "{code}");
        }
    }
}
