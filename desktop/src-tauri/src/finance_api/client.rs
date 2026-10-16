// Cliente HTTP tipado pra Finance API (easybusiness), um método por endpoint
// hoje consumido por `data-collector/sources/finance_api_client.py` (Fase 1.7
// de lá, Sessão 88) — schema levantado direto nos 8 routers do easybusiness
// nesta sessão. Sem chamador real ainda: a Fase 14.4 é quem porta a lógica
// de fetch+write do coletor Python pra cima disto.
//
// Os DTOs espelham o contrato completo de cada endpoint, inclusive campos que
// o app ainda não lê (`meta.stale`/`meta.cached`, `cvm_code`, `roe`...). Ficam
// de propósito: documentam a resposta e o `stale` deve virar aviso de "dado
// desatualizado" na tela. Por isso `dead_code` é silenciado no arquivo todo.
#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, Utc};
use serde::de::DeserializeOwned;
use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::error::AppError;
use crate::finance_api::{selic_store, FinanceApiHandle};
use crate::finance_core::market::{self, Instrument};
use crate::finance_core::{
    b3_index_stats, b3_taxa_swap, bcb_sgs, bolsai, coingecko, cvm_dfp, eth_indicators, fear_greed, metals,
    tax_rules, yahoo,
};

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

/// `ResponseMeta` dos endpoints já portados pra Rust (Fase 26): a fonte é
/// quem de fato respondeu, no lugar do que o Lume ecoava.
fn native_meta(source: &str, cached: bool, stale: bool, fetched_at: DateTime<Utc>) -> ResponseMeta {
    ResponseMeta { source: source.to_string(), cached, stale, fetched_at: Some(fetched_at) }
}

/// Instante único usado como `fetched_at` das respostas nativas sem cache.
fn now_meta(source: &str) -> ResponseMeta {
    native_meta(source, false, false, Utc::now())
}

fn price_points(points: Vec<(NaiveDate, f64)>) -> Vec<PriceHistoryPoint> {
    points
        .into_iter()
        .map(|(price_date, close_price)| PriceHistoryPoint { price_date, close_price })
        .collect()
}

fn payment_points(payments: Vec<market::DividendPayment>) -> Vec<StockDividendPaymentPoint> {
    payments
        .into_iter()
        .map(|p| StockDividendPaymentPoint {
            payment_date: p.payment_date,
            amount: p.amount,
            price_at_payment: p.price_at_payment,
            yield_pct: p.yield_pct,
        })
        .collect()
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
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockQuoteResponse, AppError> {
    let q = yahoo::quote(&Instrument::b3(ticker)).await?;
    Ok(StockQuoteResponse {
        ticker: ticker.to_string(),
        meta: now_meta(q.source),
        price: q.value.price,
        name: q.value.name,
        exchange: q.value.exchange,
        currency: q.value.currency,
    })
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
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockTechnicalsResponse, AppError> {
    let t = yahoo::technicals(&Instrument::b3(ticker)).await?;
    Ok(StockTechnicalsResponse {
        ticker: ticker.to_string(),
        meta: now_meta(t.source),
        sma_50: t.value.sma_50,
        sma_100: t.value.sma_100,
        sma_200: t.value.sma_200,
        cagr_5y: t.value.cagr_5y,
        cagr_10y: t.value.cagr_10y,
    })
}

#[derive(Debug, Deserialize)]
pub struct StockDividendsAvgResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub avg_dividend_5y: f64,
}

pub async fn fetch_stock_dividends_avg(
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockDividendsAvgResponse, AppError> {
    let a = yahoo::dividends_avg(&Instrument::b3(ticker)).await?;
    Ok(StockDividendsAvgResponse { ticker: ticker.to_string(), meta: now_meta(a.source), avg_dividend_5y: a.value })
}

#[derive(Debug, Deserialize)]
pub struct StockPriceHistoryResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<PriceHistoryPoint>,
}

pub async fn fetch_stock_price_history(
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockPriceHistoryResponse, AppError> {
    let h = yahoo::price_history(&Instrument::b3(ticker)).await?;
    Ok(StockPriceHistoryResponse {
        ticker: ticker.to_string(),
        meta: now_meta(h.source),
        data: price_points(h.value),
    })
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
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockDividendPaymentsResponse, AppError> {
    let p = yahoo::dividend_payments(&Instrument::b3(ticker)).await?;
    Ok(StockDividendPaymentsResponse {
        ticker: ticker.to_string(),
        meta: now_meta(p.source),
        data: payment_points(p.value),
    })
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
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<StockBolsaiFundamentalsResponse, AppError> {
    let f = bolsai::fundamentals(ticker).await?;
    Ok(StockBolsaiFundamentalsResponse {
        ticker: ticker.to_string(),
        meta: now_meta(f.source),
        lpa: f.value.lpa,
        vpa: f.value.vpa,
        roe: f.value.roe,
        shares_outstanding: f.value.shares_outstanding,
        cvm_code: f.value.cvm_code,
    })
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
    _handle: &FinanceApiHandle,
    cvm_code: i32,
) -> Result<CompanyRoeResponse, AppError> {
    let roe = cvm_dfp::roe(cvm_code).await?;
    Ok(CompanyRoeResponse {
        cvm_code,
        meta: now_meta(roe.source),
        reference_year: roe.value.reference_year,
        roe: roe.value.roe,
    })
}

#[derive(Debug, Deserialize)]
pub struct CompanyPayoutResponse {
    pub cvm_code: i32,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub payout_avg_5y: f64,
}

pub async fn fetch_company_payout(
    _handle: &FinanceApiHandle,
    cvm_code: i32,
) -> Result<CompanyPayoutResponse, AppError> {
    let payout = cvm_dfp::payout(cvm_code).await?;
    Ok(CompanyPayoutResponse { cvm_code, meta: now_meta(payout.source), payout_avg_5y: payout.value })
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
    _handle: &FinanceApiHandle,
    cvm_code: i32,
) -> Result<CompanyDcfFundamentalsResponse, AppError> {
    let dcf = cvm_dfp::dcf_fundamentals(cvm_code).await?;
    let f = dcf.value;
    Ok(CompanyDcfFundamentalsResponse {
        cvm_code,
        fundamentals: DcfFundamentals {
            meta: now_meta(dcf.source),
            reference_year: f.reference_year,
            ebit: f.ebit,
            tax_rate: f.tax_rate,
            depreciation_amortization: f.depreciation_amortization,
            capex: f.capex,
            nwc_change: f.nwc_change,
            total_debt: f.total_debt,
            cash: f.cash,
            revenue: f.revenue,
            inventory: f.inventory,
        },
    })
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
    _handle: &FinanceApiHandle,
    indicator_code: &str,
) -> Result<CryptoIndicatorResponse, AppError> {
    let reading = eth_indicators::indicator(indicator_code).await?;
    Ok(CryptoIndicatorResponse {
        indicator_code: indicator_code.to_string(),
        meta: now_meta(reading.source),
        raw_value: reading.value,
    })
}

#[derive(Debug, Deserialize)]
pub struct CryptoFearGreedResponse {
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub value: i32,
    pub classification: String,
    pub reading_date: NaiveDate,
}

pub async fn fetch_fear_greed(_handle: &FinanceApiHandle) -> Result<CryptoFearGreedResponse, AppError> {
    let reading = fear_greed::latest().await?;
    Ok(CryptoFearGreedResponse {
        meta: now_meta(reading.source),
        value: reading.value.value,
        classification: reading.value.classification,
        reading_date: reading.value.reading_date,
    })
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
    _handle: &FinanceApiHandle,
    symbol: &str,
) -> Result<CryptoQuoteResponse, AppError> {
    let market = coingecko::coin_market(symbol).await?;
    let coin = &market.value.value;
    let (_, price) = *coin.prices.last().expect("coin_market never returns an empty history");
    Ok(CryptoQuoteResponse {
        symbol: symbol.to_uppercase(),
        coin_id: coin.coin_id.clone(),
        name: coin.name.clone(),
        meta: native_meta(market.value.source, market.cached, market.stale, market.fetched_at),
        price,
    })
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
    _handle: &FinanceApiHandle,
    symbol: &str,
) -> Result<CryptoPriceHistoryResponse, AppError> {
    let market = coingecko::coin_market(symbol).await?;
    let coin = &market.value.value;
    Ok(CryptoPriceHistoryResponse {
        symbol: symbol.to_uppercase(),
        coin_id: coin.coin_id.clone(),
        meta: native_meta(market.value.source, market.cached, market.stale, market.fetched_at),
        data: coin
            .prices
            .iter()
            .map(|&(price_date, price)| CryptoPriceHistoryPoint { price_date, price })
            .collect(),
    })
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
    _handle: &FinanceApiHandle,
    metal_code: &str,
) -> Result<MetalQuoteResponse, AppError> {
    let info = metals::metal_info(metal_code)
        .ok_or_else(|| AppError::NoData(format!("unknown metal '{metal_code}'")))?;
    let q = yahoo::quote(&Instrument::global(info.yahoo_symbol)).await?;
    Ok(MetalQuoteResponse {
        metal_code: info.code.to_string(),
        name: info.name.to_string(),
        meta: now_meta(q.source),
        price: q.value.price,
    })
}

#[derive(Debug, Deserialize)]
pub struct MetalPriceHistoryResponse {
    pub metal_code: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<PriceHistoryPoint>,
}

pub async fn fetch_metal_price_history(
    _handle: &FinanceApiHandle,
    metal_code: &str,
) -> Result<MetalPriceHistoryResponse, AppError> {
    let info = metals::metal_info(metal_code)
        .ok_or_else(|| AppError::NoData(format!("unknown metal '{metal_code}'")))?;
    let h = yahoo::price_history(&Instrument::global(info.yahoo_symbol)).await?;
    Ok(MetalPriceHistoryResponse {
        metal_code: info.code.to_string(),
        meta: now_meta(h.source),
        data: price_points(h.value),
    })
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

// Fase 26 — nativo (BCB SGS direto, sem o Lume). `_handle` fica até a
// aposentadoria do sidecar, pra não mexer nos chamadores agora.
pub async fn fetch_benchmark_series(
    _handle: &FinanceApiHandle,
    series_code: &str,
) -> Result<MacroSeriesResponse, AppError> {
    let sourced = bcb_sgs::monthly_series(series_code).await?;
    Ok(MacroSeriesResponse {
        series_code: series_code.to_string(),
        meta: native_meta(sourced.source, false, false, Utc::now()),
        data: sourced
            .value
            .into_iter()
            .map(|(reference_month, value_pct)| MacroSeriesPoint { reference_month, value_pct })
            .collect(),
    })
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
    _handle: &FinanceApiHandle,
    index_code: &str,
) -> Result<B3IndexHistoryResponse, AppError> {
    let history = b3_index_stats::index_history(&index_code.to_lowercase()).await?;
    Ok(B3IndexHistoryResponse {
        index_code: index_code.to_lowercase(),
        meta: now_meta(history.source),
        data: price_points(history.value),
    })
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
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockQuoteResponse, AppError> {
    let q = yahoo::quote(&Instrument::global(ticker)).await?;
    Ok(UsStockQuoteResponse {
        ticker: ticker.to_string(),
        meta: now_meta(q.source),
        price: q.value.price,
        name: q.value.name,
        exchange: q.value.exchange,
        currency: q.value.currency,
    })
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
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockTechnicalsResponse, AppError> {
    let t = yahoo::technicals(&Instrument::global(ticker)).await?;
    Ok(UsStockTechnicalsResponse {
        ticker: ticker.to_string(),
        meta: now_meta(t.source),
        sma_50: t.value.sma_50,
        sma_100: t.value.sma_100,
        sma_200: t.value.sma_200,
        cagr_5y: t.value.cagr_5y,
        cagr_10y: t.value.cagr_10y,
    })
}

#[derive(Debug, Deserialize)]
pub struct UsStockDividendsAvgResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub avg_dividend_5y: f64,
}

pub async fn fetch_us_stock_dividends_avg(
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockDividendsAvgResponse, AppError> {
    let a = yahoo::dividends_avg(&Instrument::global(ticker)).await?;
    Ok(UsStockDividendsAvgResponse { ticker: ticker.to_string(), meta: now_meta(a.source), avg_dividend_5y: a.value })
}

#[derive(Debug, Deserialize)]
pub struct UsStockPriceHistoryResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<PriceHistoryPoint>,
}

pub async fn fetch_us_stock_price_history(
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockPriceHistoryResponse, AppError> {
    let h = yahoo::price_history(&Instrument::global(ticker)).await?;
    Ok(UsStockPriceHistoryResponse {
        ticker: ticker.to_string(),
        meta: now_meta(h.source),
        data: price_points(h.value),
    })
}

#[derive(Debug, Deserialize)]
pub struct UsStockDividendPaymentsResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<StockDividendPaymentPoint>,
}

pub async fn fetch_us_stock_dividend_payments(
    _handle: &FinanceApiHandle,
    ticker: &str,
) -> Result<UsStockDividendPaymentsResponse, AppError> {
    let p = yahoo::dividend_payments(&Instrument::global(ticker)).await?;
    Ok(UsStockDividendPaymentsResponse {
        ticker: ticker.to_string(),
        meta: now_meta(p.source),
        data: payment_points(p.value),
    })
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
// rates — /v1/rates/...
// ---------------------------------------------------------------------------

// Fase 17 — Meta Selic diária (BCB 432, em dias corridos, % a.a.) e curva
// DI×PRE da B3. Curva em dia sem pregão = 404 (`FinanceApiNotFound`), não
// erro — o chamador volta dia a dia via `skip_not_found`.
#[derive(Debug, Deserialize)]
pub struct SelicPoint {
    pub reference_date: NaiveDate,
    pub value_pct: f64,
}

#[derive(Debug, Deserialize)]
pub struct SelicResponse {
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<SelicPoint>,
}

/// Fase 26 — nativo e persistido: lê/atualiza `selic_daily` no `anchor.db`.
pub async fn fetch_selic(db: &DatabaseConnection, series_code: &str) -> Result<SelicResponse, AppError> {
    let series = selic_store::load(db, series_code).await?;
    Ok(SelicResponse {
        meta: native_meta(bcb_sgs::SOURCE, series.cached, series.stale, series.fetched_at),
        data: series
            .points
            .into_iter()
            .map(|(reference_date, value_pct)| SelicPoint { reference_date, value_pct })
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
pub struct DiCurvePoint {
    pub dias_uteis: i32,
    pub rate_pct: f64,
}

#[derive(Debug, Deserialize)]
pub struct DiCurveResponse {
    pub reference_date: NaiveDate,
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub data: Vec<DiCurvePoint>,
}

pub async fn fetch_di_futures_curve(
    _handle: &FinanceApiHandle,
    reference_date: NaiveDate,
) -> Result<DiCurveResponse, AppError> {
    let curve = b3_taxa_swap::di_curve(reference_date).await?;
    Ok(DiCurveResponse {
        reference_date,
        meta: native_meta(curve.value.source, curve.cached, curve.stale, curve.fetched_at),
        data: curve
            .value
            .value
            .into_iter()
            .map(|v| DiCurvePoint { dias_uteis: v.dias_uteis, rate_pct: v.rate_pct })
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// tax-rules — /v1/tax-rules/br
// ---------------------------------------------------------------------------

// Fase 24 — regras de IR curadas no Lume (sem `ResponseMeta`: não é dado
// coletado de fonte externa com cache, é tabela versionada no repo de lá).
#[derive(Debug, Deserialize)]
pub struct TaxRulePoint {
    pub key: String,
    pub value: f64,
    pub effective_from: NaiveDate,
    pub description: String,
    pub source: String,
    pub verified_at: NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct TaxRulesResponse {
    pub rules: Vec<TaxRulePoint>,
}

// Fase 26 — tabela embarcada no binário (`finance_core::tax_rules`).
pub async fn fetch_tax_rules_br(_handle: &FinanceApiHandle) -> Result<TaxRulesResponse, AppError> {
    Ok(TaxRulesResponse {
        rules: tax_rules::rules_br()
            .into_iter()
            .map(|r| TaxRulePoint {
                key: r.key.to_string(),
                value: r.value,
                effective_from: r.effective_from,
                description: r.description.to_string(),
                source: r.source.to_string(),
                verified_at: r.verified_at,
            })
            .collect(),
    })
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
    async fn live_fetch_unknown_indicator_is_no_data() {
        let err = fetch_eth_indicator(&handle(), "does-not-exist")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NoData(_)));
    }
}
