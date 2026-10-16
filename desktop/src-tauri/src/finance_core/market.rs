// Fase 26 — dados de mercado de um instrumento (cotação, histórico, técnicos,
// dividendos), a categoria que o Yahoo atende hoje. Outro provider (pago ou
// BYO-key) entra como mais uma implementação de `MarketDataProvider` na lista
// de `providers()` em `finance_core::yahoo`, sem mexer em quem consome.

use chrono::NaiveDate;

use crate::error::AppError;
use crate::finance_core::provider::{BoxFuture, DataProvider};

/// Onde o instrumento negocia: decide o símbolo que o provider usa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Market {
    /// Ação, ETF ou FII da B3 (Yahoo: sufixo `.SA`).
    B3,
    /// Qualquer outro símbolo como o provider o conhece (EUA, índices como
    /// `^BVSP`, futuros de metais como `GC=F`).
    Global,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instrument {
    pub ticker: String,
    pub market: Market,
}

impl Instrument {
    pub fn b3(ticker: &str) -> Self {
        Instrument { ticker: ticker.to_string(), market: Market::B3 }
    }
    pub fn global(ticker: &str) -> Self {
        Instrument { ticker: ticker.to_string(), market: Market::Global }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub price: f64,
    pub name: Option<String>,
    pub exchange: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Technicals {
    pub sma_50: Option<f64>,
    pub sma_100: Option<f64>,
    pub sma_200: Option<f64>,
    /// CAGR em %.
    pub cagr_5y: Option<f64>,
    pub cagr_10y: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DividendPayment {
    pub payment_date: NaiveDate,
    pub amount: f64,
    pub price_at_payment: Option<f64>,
    pub yield_pct: Option<f64>,
}

pub type ProviderResult<'a, T> = BoxFuture<'a, Result<T, AppError>>;

pub trait MarketDataProvider: DataProvider {
    fn quote<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Quote>;
    /// Fechamento diário dos últimos 10 anos, do mais antigo ao mais recente.
    fn price_history<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Vec<(NaiveDate, f64)>>;
    fn technicals<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Technicals>;
    /// Média anual de dividendos por ação nos últimos 5 anos completos.
    /// `NoData` quando não há ano completo de dividendos.
    fn dividends_avg<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, f64>;
    /// Pagamentos dos últimos 10 anos (vazio se não houve; não é erro).
    fn dividend_payments<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Vec<DividendPayment>>;
}
