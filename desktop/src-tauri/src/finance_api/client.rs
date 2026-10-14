// Cliente HTTP tipado pra Finance API (easybusiness), um método por endpoint
// hoje consumido por `data-collector/sources/finance_api_client.py` (Fase 1.7
// de lá, Sessão 88) — schema levantado direto nos 8 routers do easybusiness
// nesta sessão. Sem chamador real ainda: a Fase 14.4 é quem porta a lógica
// de fetch+write do coletor Python pra cima disto.
use chrono::{DateTime, NaiveDate, Utc};
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::error::AppError;
use crate::finance_api::FinanceApiHandle;

/// Campos presentes em toda resposta da Finance API — achatado (`flatten`)
/// dentro de cada DTO abaixo em vez de repetir os 4 campos 22 vezes.
///
/// `fetched_at` é `DateTime<Utc>`, não `NaiveDateTime` — achado real ao vivo
/// (Sessão 92, continuação): contra a Finance API rodando sobre SQLite (o
/// sidecar embutido, `DateTime(timezone=True)` do SQLAlchemy vira datetime
/// *naive* nesse dialeto), a string vinha sem sufixo `Z`/offset e
/// `NaiveDateTime` dava conta; contra Postgres (`docker compose up` do
/// easybusiness, mesmo tipo de coluna mas timezone-aware de verdade), a
/// string vem com `Z` — e `NaiveDateTime` combinado com `#[serde(flatten)]`
/// quebra com "trailing input" nesse caso (bug real de
/// serde_json+chrono+flatten, reproduzido isolado sem rede nenhuma:
/// `NaiveDateTime` engasga no sufixo de timezone quando o campo seguinte no
/// struct achatado é um escalar cru como `f64`). `DateTime<Utc>` é o tipo
/// certo pro dado em si (timestamp sempre UTC) e não tem esse problema.
#[derive(Debug, Deserialize)]
pub struct ResponseMeta {
    pub source: String,
    pub cached: bool,
    pub stale: bool,
    pub fetched_at: Option<DateTime<Utc>>,
}

/// Ponto `{price_date, close_price}` — mesma forma em stocks/metals/b3-indexes.
#[derive(Debug, Deserialize)]
pub struct PriceHistoryPoint {
    pub price_date: NaiveDate,
    pub close_price: f64,
}

async fn get<T: DeserializeOwned>(handle: &FinanceApiHandle, path: &str) -> Result<T, AppError> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}{path}", handle.base_url))
        .header("X-API-Key", &handle.api_key)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        return Ok(response.json::<T>().await?);
    }

    let body = response.text().await.unwrap_or_default();
    // FastAPI's `HTTPException` sempre serializa como `{"detail": "..."}"`
    // (confirmado nos 8 routers) — cai pro corpo cru se não for esse formato
    // (ex.: um 500 sem handler customizado, texto puro do Starlette).
    let message = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("detail")
                .and_then(|detail| detail.as_str().map(str::to_string))
        })
        .unwrap_or(body);

    if status == reqwest::StatusCode::NOT_FOUND {
        Err(AppError::FinanceApiNotFound(message))
    } else {
        Err(AppError::FinanceApi(format!("{status}: {message}")))
    }
}

// ---------------------------------------------------------------------------
// stocks — /v1/stocks/{ticker}/...
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct StockQuoteResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub price: f64,
    pub name: Option<String>,
    pub exchange: Option<String>,
    pub currency: Option<String>,
}

pub async fn fetch_stock_quote(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockQuoteResponse, AppError> {
    get(handle, &format!("/v1/stocks/{ticker}/quote")).await
}

#[derive(Debug, Deserialize)]
pub struct StockTechnicalsResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub sma_50: Option<f64>,
    pub sma_100: Option<f64>,
    pub sma_200: Option<f64>,
    pub cagr_5y: Option<f64>,
    pub cagr_10y: Option<f64>,
}

pub async fn fetch_stock_technicals(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockTechnicalsResponse, AppError> {
    get(handle, &format!("/v1/stocks/{ticker}/technicals")).await
}

#[derive(Debug, Deserialize)]
pub struct StockDividendsAvgResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub avg_dividend_5y: f64,
}

pub async fn fetch_stock_dividends_avg(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockDividendsAvgResponse, AppError> {
    get(handle, &format!("/v1/stocks/{ticker}/dividends-avg")).await
}

#[derive(Debug, Deserialize)]
pub struct StockPriceHistoryResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<PriceHistoryPoint>,
}

pub async fn fetch_stock_price_history(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockPriceHistoryResponse, AppError> {
    get(handle, &format!("/v1/stocks/{ticker}/price-history")).await
}

#[derive(Debug, Deserialize)]
pub struct StockDividendPaymentPoint {
    pub payment_date: NaiveDate,
    pub amount: f64,
    pub price_at_payment: Option<f64>,
    pub yield_pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct StockDividendPaymentsResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<StockDividendPaymentPoint>,
}

pub async fn fetch_stock_dividend_payments(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockDividendPaymentsResponse, AppError> {
    get(handle, &format!("/v1/stocks/{ticker}/dividend-payments")).await
}

#[derive(Debug, Deserialize)]
pub struct StockBolsaiFundamentalsResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub lpa: f64,
    pub vpa: f64,
    pub roe: f64,
    pub shares_outstanding: f64,
    pub cvm_code: String,
}

pub async fn fetch_stock_bolsai_fundamentals(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockBolsaiFundamentalsResponse, AppError> {
    get(handle, &format!("/v1/stocks/{ticker}/bolsai-fundamentals")).await
}

// ---------------------------------------------------------------------------
// companies — /v1/companies/{cvm_code}/...
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CompanyRoeResponse {
    pub cvm_code: i32,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub reference_year: i32,
    pub roe: f64,
}

pub async fn fetch_company_roe(
    handle: &FinanceApiHandle,
    cvm_code: i32,
) -> Result<CompanyRoeResponse, AppError> {
    get(handle, &format!("/v1/companies/{cvm_code}/roe")).await
}

#[derive(Debug, Deserialize)]
pub struct CompanyPayoutResponse {
    pub cvm_code: i32,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub payout_avg_5y: f64,
}

pub async fn fetch_company_payout(
    handle: &FinanceApiHandle,
    cvm_code: i32,
) -> Result<CompanyPayoutResponse, AppError> {
    get(handle, &format!("/v1/companies/{cvm_code}/payout")).await
}

#[derive(Debug, Deserialize)]
pub struct DcfFundamentals {
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub reference_year: i32,
    pub ebit: f64,
    pub tax_rate: Option<f64>,
    pub depreciation_amortization: Option<f64>,
    pub capex: Option<f64>,
    pub nwc_change: f64,
    pub total_debt: f64,
    pub cash: f64,
    pub revenue: f64,
    pub inventory: f64,
}

#[derive(Debug, Deserialize)]
pub struct CompanyDcfFundamentalsResponse {
    pub cvm_code: i32,
    #[serde(flatten)]
    pub fundamentals: DcfFundamentals,
}

pub async fn fetch_company_dcf_fundamentals(
    handle: &FinanceApiHandle,
    cvm_code: i32,
) -> Result<CompanyDcfFundamentalsResponse, AppError> {
    get(handle, &format!("/v1/companies/{cvm_code}/dcf-fundamentals")).await
}

// ---------------------------------------------------------------------------
// crypto — /v1/crypto/...
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CryptoIndicatorResponse {
    pub indicator_code: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub raw_value: f64,
}

pub async fn fetch_eth_indicator(
    handle: &FinanceApiHandle,
    indicator_code: &str,
) -> Result<CryptoIndicatorResponse, AppError> {
    get(handle, &format!("/v1/crypto/eth-indicators/{indicator_code}")).await
}

#[derive(Debug, Deserialize)]
pub struct CryptoFearGreedResponse {
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub value: i32,
    pub classification: String,
    pub reading_date: NaiveDate,
}

pub async fn fetch_fear_greed(handle: &FinanceApiHandle) -> Result<CryptoFearGreedResponse, AppError> {
    get(handle, "/v1/crypto/fear-greed").await
}

#[derive(Debug, Deserialize)]
pub struct CryptoQuoteResponse {
    pub symbol: String,
    pub coin_id: String,
    pub name: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub price: f64,
}

pub async fn fetch_crypto_quote(
    handle: &FinanceApiHandle,
    symbol: &str,
) -> Result<CryptoQuoteResponse, AppError> {
    get(handle, &format!("/v1/crypto/{symbol}/quote")).await
}

#[derive(Debug, Deserialize)]
pub struct CryptoPriceHistoryPoint {
    pub price_date: NaiveDate,
    pub price: f64,
}

#[derive(Debug, Deserialize)]
pub struct CryptoPriceHistoryResponse {
    pub symbol: String,
    pub coin_id: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<CryptoPriceHistoryPoint>,
}

pub async fn fetch_crypto_price_history(
    handle: &FinanceApiHandle,
    symbol: &str,
) -> Result<CryptoPriceHistoryResponse, AppError> {
    get(handle, &format!("/v1/crypto/{symbol}/price-history")).await
}

// ---------------------------------------------------------------------------
// metals — /v1/metals/{metal_code}/...
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct MetalQuoteResponse {
    pub metal_code: String,
    pub name: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub price: f64,
}

// Fase 1.7 do easybusiness (Sessão 88) achou que o catálogo de metais usa
// código minúsculo na URL (`xau`, não `XAU`) — mesmo cuidado aplicado aqui
// pra não reproduzir o mesmo bug do lado Rust.
pub async fn fetch_metal_quote(
    handle: &FinanceApiHandle,
    metal_code: &str,
) -> Result<MetalQuoteResponse, AppError> {
    get(
        handle,
        &format!("/v1/metals/{}/quote", metal_code.to_lowercase()),
    )
    .await
}

#[derive(Debug, Deserialize)]
pub struct MetalPriceHistoryResponse {
    pub metal_code: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<PriceHistoryPoint>,
}

pub async fn fetch_metal_price_history(
    handle: &FinanceApiHandle,
    metal_code: &str,
) -> Result<MetalPriceHistoryResponse, AppError> {
    get(
        handle,
        &format!("/v1/metals/{}/price-history", metal_code.to_lowercase()),
    )
    .await
}

// ---------------------------------------------------------------------------
// fiis — /v1/fiis/{cnpj}/...
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct FiiMonthlyIndicatorsResponse {
    pub cnpj: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub reference_date: NaiveDate,
    pub patrimonio_liquido: f64,
    pub valor_patrimonial_cota: f64,
    pub numero_cotistas: Option<i32>,
    pub dividend_yield_mes: Option<f64>,
    pub rentabilidade_efetiva_mes: Option<f64>,
}

pub async fn fetch_fii_monthly_indicators(
    handle: &FinanceApiHandle,
    cnpj: &str,
) -> Result<FiiMonthlyIndicatorsResponse, AppError> {
    get(handle, &format!("/v1/fiis/{cnpj}/monthly-indicators")).await
}

#[derive(Debug, Deserialize)]
pub struct FiiPropertyPoint {
    pub nome_imovel: String,
    pub reference_date: NaiveDate,
    pub endereco: Option<String>,
    pub area_m2: Option<f64>,
    pub percentual_vacancia: Option<f64>,
    pub percentual_inadimplencia: Option<f64>,
    pub percentual_receitas_fii: Option<f64>,
    pub percentual_locado: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct FiiPropertiesResponse {
    pub cnpj: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<FiiPropertyPoint>,
}

pub async fn fetch_fii_properties(
    handle: &FinanceApiHandle,
    cnpj: &str,
) -> Result<FiiPropertiesResponse, AppError> {
    get(handle, &format!("/v1/fiis/{cnpj}/properties")).await
}

// ---------------------------------------------------------------------------
// macro-series — /v1/macro-series/{series_code}
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct MacroSeriesPoint {
    pub reference_month: NaiveDate,
    pub value_pct: f64,
}

#[derive(Debug, Deserialize)]
pub struct MacroSeriesResponse {
    pub series_code: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<MacroSeriesPoint>,
}

pub async fn fetch_benchmark_series(
    handle: &FinanceApiHandle,
    series_code: &str,
) -> Result<MacroSeriesResponse, AppError> {
    get(handle, &format!("/v1/macro-series/{series_code}")).await
}

// ---------------------------------------------------------------------------
// b3-indexes — /v1/b3-indexes/{index_code}/history
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct B3IndexHistoryResponse {
    pub index_code: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<PriceHistoryPoint>,
}

// Mesma pegadinha de casing de `fetch_metal_quote` — catálogo usa código
// minúsculo (`ifix`, não `IFIX`) na URL.
pub async fn fetch_b3_index_history(
    handle: &FinanceApiHandle,
    index_code: &str,
) -> Result<B3IndexHistoryResponse, AppError> {
    get(
        handle,
        &format!("/v1/b3-indexes/{}/history", index_code.to_lowercase()),
    )
    .await
}

// ---------------------------------------------------------------------------
// us-stocks — /v1/us-stocks/{ticker}/...
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UsStockFundamentalsResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub lpa: f64,
    pub vpa: f64,
    pub roe: f64,
    pub shares_outstanding: f64,
}

pub async fn fetch_us_stock_fundamentals(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockFundamentalsResponse, AppError> {
    get(handle, &format!("/v1/us-stocks/{ticker}/fundamentals")).await
}

#[derive(Debug, Deserialize)]
pub struct UsStockDcfFundamentalsResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub fundamentals: DcfFundamentals,
}

pub async fn fetch_us_stock_dcf_fundamentals(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockDcfFundamentalsResponse, AppError> {
    get(handle, &format!("/v1/us-stocks/{ticker}/dcf-fundamentals")).await
}

#[derive(Debug, Deserialize)]
pub struct UsStockPayoutResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub payout_avg_5y: f64,
}

pub async fn fetch_us_stock_payout(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockPayoutResponse, AppError> {
    get(handle, &format!("/v1/us-stocks/{ticker}/payout")).await
}

// Fase 1.11.1 do easybusiness (Sessão 92 do Anchor) — mesmos 5 recursos de
// `/v1/stocks/{ticker}/...` acima, só que pra ticker sem sufixo `.SA` (ação
// americana, ETF-US, REIT, ou um índice sem sufixo como `^BVSP`/IBOV — sem
// tratamento especial pra índice, mesma decisão que o coletor Python já
// tomava). DTOs idênticos aos `Stock*Response` — mesmos campos.

#[derive(Debug, Deserialize)]
pub struct UsStockQuoteResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub price: f64,
    pub name: Option<String>,
    pub exchange: Option<String>,
    pub currency: Option<String>,
}

pub async fn fetch_us_stock_quote(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockQuoteResponse, AppError> {
    get(handle, &format!("/v1/us-stocks/{ticker}/quote")).await
}

#[derive(Debug, Deserialize)]
pub struct UsStockTechnicalsResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub sma_50: Option<f64>,
    pub sma_100: Option<f64>,
    pub sma_200: Option<f64>,
    pub cagr_5y: Option<f64>,
    pub cagr_10y: Option<f64>,
}

pub async fn fetch_us_stock_technicals(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockTechnicalsResponse, AppError> {
    get(handle, &format!("/v1/us-stocks/{ticker}/technicals")).await
}

#[derive(Debug, Deserialize)]
pub struct UsStockDividendsAvgResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub avg_dividend_5y: f64,
}

pub async fn fetch_us_stock_dividends_avg(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockDividendsAvgResponse, AppError> {
    get(handle, &format!("/v1/us-stocks/{ticker}/dividends-avg")).await
}

#[derive(Debug, Deserialize)]
pub struct UsStockPriceHistoryResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<PriceHistoryPoint>,
}

pub async fn fetch_us_stock_price_history(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockPriceHistoryResponse, AppError> {
    get(handle, &format!("/v1/us-stocks/{ticker}/price-history")).await
}

#[derive(Debug, Deserialize)]
pub struct UsStockDividendPaymentsResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<StockDividendPaymentPoint>,
}

pub async fn fetch_us_stock_dividend_payments(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockDividendPaymentsResponse, AppError> {
    get(handle, &format!("/v1/us-stocks/{ticker}/dividend-payments")).await
}

// Fase 1.11.2 do easybusiness — indicadores imobiliários de REIT via SEC
// EDGAR. Time-series do lado da API (append-only por `reference_year`), por
// isso `data` é uma lista, não um objeto único.
#[derive(Debug, Deserialize)]
pub struct ReitFundamentalsPoint {
    pub reference_year: i32,
    pub revenue: f64,
    pub real_estate_property_net: Option<f64>,
    pub real_estate_property_at_cost: Option<f64>,
    pub stockholders_equity: f64,
    pub net_income: Option<f64>,
    pub eps_diluted: f64,
}

#[derive(Debug, Deserialize)]
pub struct ReitFundamentalsResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<ReitFundamentalsPoint>,
}

pub async fn fetch_reit_fundamentals(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<ReitFundamentalsResponse, AppError> {
    get(handle, &format!("/v1/us-stocks/{ticker}/reit-fundamentals")).await
}

// ---------------------------------------------------------------------------
// fiis (ticker-keyed) — /v1/fiis/resolve/{ticker}
// ---------------------------------------------------------------------------

// Fase 1.11.3 do easybusiness — resolução ticker->CNPJ de FII (cruza bolsai
// + CVM do lado de lá). Router separado de `/v1/fiis/{cnpj}/...` acima
// porque é keyed por ticker, não CNPJ.
#[derive(Debug, Deserialize)]
pub struct FiiCnpjResolutionResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub cnpj: String,
    pub fund_name: String,
}

pub async fn fetch_fii_cnpj_resolution(
    handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<FiiCnpjResolutionResponse, AppError> {
    get(handle, &format!("/v1/fiis/resolve/{ticker}")).await
}

// ---------------------------------------------------------------------------
// Testes `#[ignore]` — precisam de uma Finance API real rodando em
// `http://localhost:8000` com `API_KEYS=local-dev-key-change-me` (mesmos
// valores de `sidecar::DEV_BASE_URL`/`DEV_API_KEY`), ex.: `sidecar_main.py`
// do easybusiness contra SQLite, ou `docker compose up` de lá. Rodar com
// `cargo test --lib -- --ignored finance_api`. Fora do `cargo test --lib`
// padrão de propósito — não podem depender de rede/serviço externo no CI.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::finance_api::FinanceApiHandle;

    fn handle() -> FinanceApiHandle {
        FinanceApiHandle::for_test(
            "http://localhost:8000".to_string(),
            "local-dev-key-change-me".to_string(),
        )
    }

    #[tokio::test]
    #[ignore]
    async fn live_fetch_benchmark_series() {
        let series = fetch_benchmark_series(&handle(), "cdi").await.unwrap();
        assert_eq!(series.series_code, "cdi");
        assert!(!series.data.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn live_fetch_fear_greed() {
        let reading = fetch_fear_greed(&handle()).await.unwrap();
        assert!(reading.value >= 0 && reading.value <= 100);
    }

    #[tokio::test]
    #[ignore]
    async fn live_fetch_metal_quote_lowercases_the_code() {
        let quote = fetch_metal_quote(&handle(), "XAU").await.unwrap();
        assert_eq!(quote.metal_code, "xau");
        assert!(quote.price > 0.0);
    }

    #[tokio::test]
    #[ignore]
    async fn live_fetch_b3_index_history_lowercases_the_code() {
        let history = fetch_b3_index_history(&handle(), "IFIX").await.unwrap();
        assert_eq!(history.index_code, "ifix");
        assert!(!history.data.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn live_fetch_stock_quote() {
        let quote = fetch_stock_quote(&handle(), "PETR4").await.unwrap();
        assert_eq!(quote.ticker, "PETR4");
        assert!(quote.price > 0.0);
    }

    #[tokio::test]
    #[ignore]
    async fn live_fetch_unknown_indicator_is_not_found() {
        let err = fetch_eth_indicator(&handle(), "does-not-exist")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::FinanceApiNotFound(_)));
    }
}
