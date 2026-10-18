// Yahoo Finance (API de gráfico não oficial, a mesma do `yfinance`; pública,
// sem chave) — porta de `lume/api/app/sources/acoes_yahoo.py`.
// `GET https://query1.finance.yahoo.com/v8/finance/chart/{symbol}`: `.SA` pra
// B3, sem sufixo pro resto. Os cálculos são funções puras sobre o JSON
// (testadas com respostas no formato real); os `fetch_*` são finos.
//
// Diferença consciente do Lume: símbolo desconhecido (HTTP 404, ou
// `result: null`) vira `NoData` — o coletor pula o ticker. O Lume devolvia 502
// pra esse caso, o que derrubava o lote inteiro.

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::Deserialize;

use crate::error::AppError;
use crate::finance_core::http;
use crate::finance_core::market::{
    DividendPayment, Instrument, Market, MarketDataProvider, ProviderResult, Quote, Technicals,
};
use crate::finance_core::provider::{cascade, DataProvider, Sourced};

pub const SOURCE: &str = "yahoo_finance";
const CHART_URL: &str = "https://query1.finance.yahoo.com/v8/finance/chart";

/// 5 anos completos de dividendos (descartado o ano corrente) e o CAGR de 10 anos.
const HISTORY_RANGE: &str = "10y";
const DIVIDENDS_YEARS_AVERAGED: usize = 5;
const SMA_WINDOWS: [usize; 3] = [50, 100, 200];
const CAGR_YEARS: [i64; 2] = [5, 10];
const SECONDS_PER_YEAR: f64 = 365.25 * 86400.0;
/// Se o candle mais próximo da data-alvo está mais longe que isso, não há
/// histórico suficiente pro CAGR (ex.: IPO recente): `None` é mais seguro que
/// um CAGR de outro período.
const CAGR_ANCHOR_TOLERANCE_DAYS: f64 = 30.0;
/// Entre o pagamento e o candle costuma haver 1–2 dias (fim de semana/feriado).
const DIVIDEND_PRICE_TOLERANCE_DAYS: f64 = 5.0;

// ---------------------------------------------------------------------------
// Formato da resposta
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Envelope {
    chart: Chart,
}

#[derive(Deserialize)]
struct Chart {
    result: Option<Vec<ChartResult>>,
    error: Option<ChartError>,
}

#[derive(Deserialize)]
struct ChartError {
    description: Option<String>,
}

#[derive(Deserialize)]
pub struct ChartResult {
    meta: Meta,
    timestamp: Option<Vec<i64>>,
    indicators: Option<Indicators>,
    events: Option<Events>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    regular_market_price: Option<f64>,
    long_name: Option<String>,
    short_name: Option<String>,
    full_exchange_name: Option<String>,
    exchange_name: Option<String>,
    currency: Option<String>,
}

#[derive(Deserialize)]
struct Indicators {
    quote: Vec<QuoteIndicator>,
    /// Fechamento ajustado a dividendos e desdobramentos; ausente nas respostas que não o trazem.
    #[serde(default)]
    adjclose: Vec<AdjCloseIndicator>,
}

#[derive(Deserialize)]
struct AdjCloseIndicator {
    adjclose: Vec<Option<f64>>,
}

#[derive(Deserialize)]
struct QuoteIndicator {
    close: Vec<Option<f64>>,
}

#[derive(Deserialize)]
struct Events {
    dividends: Option<BTreeMap<String, DividendEvent>>,
}

#[derive(Deserialize)]
struct DividendEvent {
    date: i64,
    amount: f64,
}

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("Yahoo Finance: {detail}"))
}

pub fn parse_chart(body: &str) -> Result<ChartResult, AppError> {
    let envelope: Envelope = serde_json::from_str(body).map_err(unavailable)?;
    match envelope.chart.result.and_then(|mut r| if r.is_empty() { None } else { Some(r.remove(0)) }) {
        Some(result) => Ok(result),
        None => Err(AppError::NoData(format!(
            "Yahoo Finance: {}",
            envelope.chart.error.and_then(|e| e.description).unwrap_or_else(|| "empty result".into())
        ))),
    }
}

fn date_of(ts: i64) -> Option<NaiveDate> {
    DateTime::<Utc>::from_timestamp(ts, 0).map(|dt| dt.date_naive())
}

/// `(timestamps, closes)` da série diária; erro se a resposta não traz histórico.
fn series(result: &ChartResult) -> Result<(&[i64], &[Option<f64>]), AppError> {
    let timestamps = result.timestamp.as_deref().ok_or_else(|| unavailable("response missing history"))?;
    let closes = result
        .indicators
        .as_ref()
        .and_then(|i| i.quote.first())
        .map(|q| q.close.as_slice())
        .ok_or_else(|| unavailable("response missing history"))?;
    Ok((timestamps, closes))
}

// ---------------------------------------------------------------------------
// Cálculos puros
// ---------------------------------------------------------------------------

pub fn quote_of(result: &ChartResult) -> Result<Quote, AppError> {
    let meta = &result.meta;
    let price = meta.regular_market_price.ok_or_else(|| unavailable("response missing price"))?;
    Ok(Quote {
        price,
        name: meta.long_name.clone().or_else(|| meta.short_name.clone()),
        exchange: meta.full_exchange_name.clone().or_else(|| meta.exchange_name.clone()),
        currency: meta.currency.clone(),
    })
}

/// Um ponto por pregão; dias sem fechamento (`null`) são pulados.
pub fn price_history_of(result: &ChartResult) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let (timestamps, closes) = series(result)?;
    Ok(timestamps
        .iter()
        .zip(closes)
        .filter_map(|(&ts, close)| Some((date_of(ts)?, (*close)?)))
        .collect())
}

/// Fechamento ajustado a dividendos e desdobramentos, um ponto por pregão (retorno total). Dias sem valor
/// são pulados; vazio quando a resposta não traz `adjclose`.
pub fn adjusted_close_history_of(result: &ChartResult) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let timestamps = result.timestamp.as_deref().ok_or_else(|| unavailable("response missing history"))?;
    let adjusted = result
        .indicators
        .as_ref()
        .and_then(|i| i.adjclose.first())
        .map(|a| a.adjclose.as_slice())
        .unwrap_or(&[]);
    Ok(timestamps
        .iter()
        .zip(adjusted)
        .filter_map(|(&ts, value)| Some((date_of(ts)?, (*value)?)))
        .collect())
}

/// Candle mais próximo de `target_ts` ignorando dias sem fechamento; `None` se
/// o mais próximo está além da tolerância. Empate: fica o primeiro (como no Lume).
fn closest_close(timestamps: &[i64], closes: &[Option<f64>], target_ts: f64, tolerance_days: f64) -> Option<f64> {
    let mut best: Option<(i64, f64)> = None;
    for (&ts, close) in timestamps.iter().zip(closes) {
        let Some(close) = *close else { continue };
        if best.map_or(true, |(best_ts, _)| (ts as f64 - target_ts).abs() < (best_ts as f64 - target_ts).abs()) {
            best = Some((ts, close));
        }
    }
    let (best_ts, close) = best?;
    ((best_ts as f64 - target_ts).abs() <= tolerance_days * 86400.0).then_some(close)
}

pub fn technicals_of(result: &ChartResult) -> Result<Technicals, AppError> {
    let (timestamps, closes) = series(result)?;
    let valid: Vec<f64> = closes.iter().filter_map(|c| *c).collect();
    let latest_close = *valid
        .last()
        .ok_or_else(|| AppError::NoData("Yahoo Finance: no trading data".into()))?;
    let latest_ts = *timestamps.last().ok_or_else(|| unavailable("response missing history"))?;

    let sma = |window: usize| -> Option<f64> {
        (valid.len() >= window).then(|| valid[valid.len() - window..].iter().sum::<f64>() / window as f64)
    };
    let cagr = |years: i64| -> Option<f64> {
        let target = latest_ts as f64 - years as f64 * SECONDS_PER_YEAR;
        let anchor = closest_close(timestamps, closes, target, CAGR_ANCHOR_TOLERANCE_DAYS)?;
        (anchor > 0.0).then(|| ((latest_close / anchor).powf(1.0 / years as f64) - 1.0) * 100.0)
    };

    Ok(Technicals {
        sma_50: sma(SMA_WINDOWS[0]),
        sma_100: sma(SMA_WINDOWS[1]),
        sma_200: sma(SMA_WINDOWS[2]),
        cagr_5y: cagr(CAGR_YEARS[0]),
        cagr_10y: cagr(CAGR_YEARS[1]),
    })
}

fn dividend_events(result: &ChartResult) -> Vec<(i64, f64)> {
    let mut events: Vec<(i64, f64)> = result
        .events
        .as_ref()
        .and_then(|e| e.dividends.as_ref())
        .map(|d| d.values().map(|e| (e.date, e.amount)).collect())
        .unwrap_or_default();
    events.sort_by_key(|&(date, _)| date);
    events
}

/// Média, por ano, do total de dividendos dos 5 anos completos mais recentes
/// (o ano `current_year` é parcial e fica de fora). `None` sem ano completo.
pub fn dividends_avg_of(result: &ChartResult, current_year: i32) -> Option<f64> {
    let mut yearly: BTreeMap<i32, f64> = BTreeMap::new();
    for (ts, amount) in dividend_events(result) {
        let Some(date) = date_of(ts) else { continue };
        if date.year() != current_year {
            *yearly.entry(date.year()).or_default() += amount;
        }
    }
    let recent: Vec<f64> = yearly.values().rev().take(DIVIDENDS_YEARS_AVERAGED).copied().collect();
    (!recent.is_empty()).then(|| recent.iter().sum::<f64>() / recent.len() as f64)
}

pub fn dividend_payments_of(result: &ChartResult) -> Result<Vec<DividendPayment>, AppError> {
    let events = dividend_events(result);
    if events.is_empty() {
        return Ok(Vec::new());
    }
    let (timestamps, closes) = series(result)?;
    Ok(events
        .into_iter()
        .filter_map(|(ts, amount)| {
            let payment_date = date_of(ts)?;
            let price = closest_close(timestamps, closes, ts as f64, DIVIDEND_PRICE_TOLERANCE_DAYS);
            Some(DividendPayment {
                payment_date,
                amount,
                price_at_payment: price,
                yield_pct: price.filter(|p| *p != 0.0).map(|p| amount / p * 100.0),
            })
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Fetch
// ---------------------------------------------------------------------------

/// Símbolo no Yahoo: B3 leva `.SA`; o resto vai como veio (`PETR4`→`PETR4.SA`,
/// `AAPL`, `^BVSP`, `GC=F`).
pub fn symbol_of(instrument: &Instrument) -> String {
    match instrument.market {
        Market::B3 => format!("{}.SA", instrument.ticker),
        Market::Global => instrument.ticker.clone(),
    }
}

async fn fetch_chart(instrument: &Instrument, query: &str) -> Result<ChartResult, AppError> {
    let url = format!("{CHART_URL}/{}?{query}", symbol_of(instrument));
    parse_chart(&http::get_text("Yahoo Finance", &url, http::DEFAULT_TIMEOUT).await?)
}

pub struct Yahoo;

impl DataProvider for Yahoo {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl MarketDataProvider for Yahoo {
    fn quote<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Quote> {
        Box::pin(async move { quote_of(&fetch_chart(instrument, "range=5d&interval=1d").await?) })
    }

    fn price_history<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Vec<(NaiveDate, f64)>> {
        Box::pin(async move {
            price_history_of(&fetch_chart(instrument, &format!("range={HISTORY_RANGE}&interval=1d")).await?)
        })
    }

    fn technicals<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Technicals> {
        Box::pin(async move {
            technicals_of(&fetch_chart(instrument, &format!("range={HISTORY_RANGE}&interval=1d")).await?)
        })
    }

    fn dividends_avg<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, f64> {
        Box::pin(async move {
            let chart =
                fetch_chart(instrument, &format!("range={HISTORY_RANGE}&interval=3mo&events=div")).await?;
            dividends_avg_of(&chart, Utc::now().year()).ok_or_else(|| {
                AppError::NoData(format!("{}: no complete year of dividends", instrument.ticker))
            })
        })
    }

    fn dividend_payments<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Vec<DividendPayment>> {
        Box::pin(async move {
            dividend_payments_of(
                &fetch_chart(instrument, &format!("range={HISTORY_RANGE}&interval=1d&events=div")).await?,
            )
        })
    }
}

fn providers() -> Vec<Box<dyn MarketDataProvider>> {
    // Frankfurter só responde pares de moeda (o resto é `NoData`), então fica
    // como reserva do câmbio sem afetar ações.
    let mut providers: Vec<Box<dyn MarketDataProvider>> =
        vec![Box::new(Yahoo), Box::new(crate::finance_core::frankfurter::Frankfurter)];
    // Alpha Vantage é BYO-key: só entra com chave, senão um ticker inexistente
    // viraria "provider indisponível" em vez de "sem dado".
    if crate::finance_core::alpha_vantage::is_configured() {
        providers.push(Box::new(crate::finance_core::alpha_vantage::AlphaVantage {
            key: &crate::finance_core::api_keys::ALPHA_VANTAGE,
        }));
    }
    providers
}

/// Roda `call` em cada provider de mercado, em ordem, e devolve o primeiro que responde.
async fn from_providers<T>(
    metric: &str,
    instrument: &Instrument,
    call: impl for<'a> Fn(&'a dyn MarketDataProvider, &'a Instrument) -> ProviderResult<'a, T>,
) -> Result<Sourced<T>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), call(p.as_ref(), instrument))).collect();
    cascade(&format!("{metric} for '{}'", instrument.ticker), attempts).await
}

pub async fn quote(instrument: &Instrument) -> Result<Sourced<Quote>, AppError> {
    from_providers("quote", instrument, |p, i| p.quote(i)).await
}

/// Fechamento ajustado de 10 anos direto do Yahoo (só ele traz `adjclose`, sem a cadeia de provedores).
pub async fn adjusted_close_history(instrument: &Instrument) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    adjusted_close_history_of(&fetch_chart(instrument, &format!("range={HISTORY_RANGE}&interval=1d")).await?)
}

pub async fn price_history(instrument: &Instrument) -> Result<Sourced<Vec<(NaiveDate, f64)>>, AppError> {
    from_providers("price history", instrument, |p, i| p.price_history(i)).await
}

pub async fn technicals(instrument: &Instrument) -> Result<Sourced<Technicals>, AppError> {
    from_providers("technicals", instrument, |p, i| p.technicals(i)).await
}

pub async fn dividends_avg(instrument: &Instrument) -> Result<Sourced<f64>, AppError> {
    from_providers("dividends average", instrument, |p, i| p.dividends_avg(i)).await
}

pub async fn dividend_payments(instrument: &Instrument) -> Result<Sourced<Vec<DividendPayment>>, AppError> {
    from_providers("dividend payments", instrument, |p, i| p.dividend_payments(i)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;
    /// 2026-01-05 00:00 UTC (segunda).
    const T0: i64 = 1_767_571_200;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// Resposta no formato real do Yahoo; `closes` com `null` vira `None`.
    fn chart_json(timestamps: &[i64], closes: &[Option<f64>], dividends: &[(i64, f64)]) -> String {
        let closes: Vec<String> = closes.iter().map(|c| c.map_or("null".into(), |v| v.to_string())).collect();
        let divs: Vec<String> = dividends
            .iter()
            .map(|(date, amount)| format!(r#""{date}":{{"amount":{amount},"date":{date}}}"#))
            .collect();
        format!(
            r#"{{"chart":{{"result":[{{"meta":{{"currency":"BRL","symbol":"PETR4.SA","exchangeName":"SAO","fullExchangeName":"São Paulo","regularMarketPrice":59.68,"longName":"Petróleo Brasileiro S.A. - Petrobras","shortName":"PETROBRAS   PN      N2"}},"timestamp":{:?},"events":{{"dividends":{{{}}}}},"indicators":{{"quote":[{{"close":[{}]}}]}}}}],"error":null}}}}"#,
            timestamps,
            divs.join(","),
            closes.join(",")
        )
    }

    fn parse(timestamps: &[i64], closes: &[Option<f64>], dividends: &[(i64, f64)]) -> ChartResult {
        parse_chart(&chart_json(timestamps, closes, dividends)).unwrap()
    }

    #[test]
    fn symbols_get_the_b3_suffix_only_for_b3() {
        assert_eq!(symbol_of(&Instrument::b3("PETR4")), "PETR4.SA");
        assert_eq!(symbol_of(&Instrument::global("AAPL")), "AAPL");
        assert_eq!(symbol_of(&Instrument::global("^BVSP")), "^BVSP");
        assert_eq!(symbol_of(&Instrument::global("GC=F")), "GC=F");
    }

    #[test]
    fn unknown_symbol_is_no_data() {
        let body = r#"{"chart":{"result":null,"error":{"code":"Not Found","description":"No data found, symbol may be delisted"}}}"#;
        let err = parse_chart(body).err().unwrap();
        assert!(matches!(&err, AppError::NoData(m) if m.contains("delisted")), "{err}");
    }

    #[test]
    fn garbage_is_provider_unavailable() {
        assert!(matches!(parse_chart("<html>").err().unwrap(), AppError::ProviderUnavailable(_)));
    }

    #[test]
    fn quote_prefers_long_name_and_full_exchange() {
        let q = quote_of(&parse(&[T0], &[Some(1.0)], &[])).unwrap();
        assert_eq!(
            q,
            Quote {
                price: 59.68,
                name: Some("Petróleo Brasileiro S.A. - Petrobras".into()),
                exchange: Some("São Paulo".into()),
                currency: Some("BRL".into()),
            }
        );
    }

    #[test]
    fn quote_without_price_is_an_error() {
        let body = r#"{"chart":{"result":[{"meta":{"currency":"BRL"}}]}}"#;
        assert!(matches!(quote_of(&parse_chart(body).unwrap()), Err(AppError::ProviderUnavailable(_))));
    }

    #[test]
    fn history_skips_days_without_a_close() {
        let r = parse(&[T0, T0 + DAY, T0 + 2 * DAY], &[Some(10.0), None, Some(11.0)], &[]);
        assert_eq!(price_history_of(&r).unwrap(), vec![(d(2026, 1, 5), 10.0), (d(2026, 1, 7), 11.0)]);
    }

    #[test]
    fn the_adjusted_close_comes_from_adjclose_and_skips_days_without_it() {
        let body = r#"{"chart":{"result":[{"meta":{},"timestamp":[1767571200,1767657600,1767744000],
            "indicators":{"quote":[{"close":[10.0,10.5,11.0]}],"adjclose":[{"adjclose":[9.0,null,10.0]}]}}]}}"#;
        let adjusted = adjusted_close_history_of(&parse_chart(body).unwrap()).unwrap();
        assert_eq!(adjusted.len(), 2, "o dia sem ajuste fica de fora");
        assert!((adjusted[0].1 - 9.0).abs() < 1e-9 && (adjusted[1].1 - 10.0).abs() < 1e-9);
        // Sem `adjclose` na resposta, a série é vazia (o backtest pede o backfill).
        let plain = r#"{"chart":{"result":[{"meta":{},"timestamp":[1767571200],"indicators":{"quote":[{"close":[10.0]}]}}]}}"#;
        assert!(adjusted_close_history_of(&parse_chart(plain).unwrap()).unwrap().is_empty());
    }

    #[test]
    fn sma_needs_a_full_window() {
        let closes: Vec<Option<f64>> = (1..=60).map(|i| Some(i as f64)).collect();
        let ts: Vec<i64> = (0..60).map(|i| T0 + i * DAY).collect();
        let t = technicals_of(&parse(&ts, &closes, &[])).unwrap();
        assert_eq!(t.sma_50, Some((11..=60).sum::<i32>() as f64 / 50.0));
        assert_eq!(t.sma_100, None);
        assert_eq!(t.sma_200, None);
    }

    #[test]
    fn cagr_is_none_without_history_near_the_target_date() {
        let t = technicals_of(&parse(&[T0, T0 + DAY], &[Some(10.0), Some(11.0)], &[])).unwrap();
        assert_eq!((t.cagr_5y, t.cagr_10y), (None, None));
    }

    #[test]
    fn cagr_compounds_over_the_period() {
        // Fechamento 100 há exatamente 5 anos (365,25 d) e 200 hoje: (200/100)^(1/5)-1 ≈ 14,87%.
        let latest = T0 + 5 * 365 * DAY + 5 * DAY;
        let anchor = latest - (5.0 * SECONDS_PER_YEAR) as i64;
        let t = technicals_of(&parse(&[anchor, latest], &[Some(100.0), Some(200.0)], &[])).unwrap();
        let expected = (2f64.powf(0.2) - 1.0) * 100.0;
        assert!((t.cagr_5y.unwrap() - expected).abs() < 1e-9, "{:?}", t.cagr_5y);
        assert_eq!(t.cagr_10y, None);
    }

    #[test]
    fn technicals_without_any_close_is_no_data() {
        let err = technicals_of(&parse(&[T0], &[None], &[])).err().unwrap();
        assert!(matches!(err, AppError::NoData(_)));
    }

    #[test]
    fn dividends_avg_skips_the_current_year_and_averages_complete_ones() {
        let at = |y, m, day| chrono::NaiveDate::from_ymd_opt(y, m, day).unwrap().and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp();
        let divs = [
            (at(2026, 3, 1), 99.0), // ano corrente, ignorado
            (at(2025, 3, 1), 1.0),
            (at(2025, 9, 1), 1.0), // 2025: 2,0
            (at(2024, 3, 1), 4.0), // 2024: 4,0
        ];
        let r = parse(&[T0], &[Some(1.0)], &divs);
        assert_eq!(dividends_avg_of(&r, 2026), Some(3.0));
    }

    #[test]
    fn dividends_avg_uses_only_the_five_most_recent_years() {
        let at = |y| chrono::NaiveDate::from_ymd_opt(y, 6, 1).unwrap().and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp();
        let divs: Vec<_> = (2018..=2025).map(|y| (at(y), if y <= 2020 { 1000.0 } else { 2.0 })).collect();
        let r = parse(&[T0], &[Some(1.0)], &divs);
        assert_eq!(dividends_avg_of(&r, 2026), Some(2.0)); // 2021..2025
    }

    #[test]
    fn dividends_avg_is_none_without_a_complete_year() {
        let r = parse(&[T0], &[Some(1.0)], &[]);
        assert_eq!(dividends_avg_of(&r, 2026), None);
        let at = chrono::NaiveDate::from_ymd_opt(2026, 2, 1).unwrap().and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp();
        assert_eq!(dividends_avg_of(&parse(&[T0], &[Some(1.0)], &[(at, 1.0)]), 2026), None);
    }

    #[test]
    fn dividend_payment_uses_the_close_within_five_days() {
        let r = parse(&[T0, T0 + DAY], &[Some(20.0), None], &[(T0 + DAY, 1.0), (T0 + 30 * DAY, 2.0)]);
        let payments = dividend_payments_of(&r).unwrap();
        assert_eq!(payments[0].payment_date, d(2026, 1, 6));
        assert_eq!(payments[0].price_at_payment, Some(20.0)); // candle de 05/01, o dia 06 não tem fechamento
        assert_eq!(payments[0].yield_pct, Some(5.0));
        assert_eq!(payments[1].price_at_payment, None); // 25 dias do último candle
        assert_eq!(payments[1].yield_pct, None);
    }

    #[test]
    fn no_dividends_is_an_empty_list() {
        assert!(dividend_payments_of(&parse(&[T0], &[Some(1.0)], &[])).unwrap().is_empty());
    }

    // Ao vivo (rede): `cargo test --lib -- --ignored finance_core::yahoo`.
    #[tokio::test]
    #[ignore]
    async fn live_b3_us_index_and_future() {
        let q = quote(&Instrument::b3("PETR4")).await.unwrap();
        assert_eq!(q.source, SOURCE);
        assert!(q.value.price > 1.0 && q.value.currency.as_deref() == Some("BRL"));

        // Valores impressos pra conferir a paridade com o Python do Lume
        // (`--nocapture`); as asserções só checam plausibilidade.
        let t = technicals(&Instrument::b3("PETR4")).await.unwrap().value;
        println!("technicals {t:?}");
        assert!(t.sma_200.is_some() && t.cagr_5y.is_some());
        let h = price_history(&Instrument::b3("PETR4")).await.unwrap().value;
        println!("history {} last {:?}", h.len(), h.last());
        assert!(h.len() > 2000, "{} points", h.len());
        let avg = dividends_avg(&Instrument::b3("PETR4")).await.unwrap().value;
        println!("dividends_avg {avg}");
        assert!(avg > 0.0);
        let pay = dividend_payments(&Instrument::b3("PETR4")).await.unwrap().value;
        println!("payments {} last {:?}", pay.len(), pay.last());
        assert!(pay.len() > 10);

        assert_eq!(quote(&Instrument::global("AAPL")).await.unwrap().value.currency.as_deref(), Some("USD"));
        assert!(quote(&Instrument::global("^BVSP")).await.unwrap().value.price > 10_000.0);
        assert!(quote(&Instrument::global("GC=F")).await.unwrap().value.price > 100.0);

        let err = quote(&Instrument::b3("XXXXNOPE")).await.unwrap_err();
        assert!(matches!(err, AppError::NoData(_)), "{err}");
    }

    // Câmbio (Fase 32, seção I): par `{BASE}{QUOTE}=X`, inclusive cross sem BRL.
    #[tokio::test]
    #[ignore]
    async fn live_fx_pairs() {
        let eurusd = quote(&Instrument::global("EURUSD=X")).await.unwrap().value;
        println!("EURUSD {eurusd:?}");
        assert!(eurusd.price > 0.5 && eurusd.price < 2.0);
        assert_eq!(eurusd.currency.as_deref(), Some("USD"));

        let usdbrl = quote(&Instrument::global("USDBRL=X")).await.unwrap().value;
        assert!(usdbrl.price > 2.0 && usdbrl.price < 20.0, "{}", usdbrl.price);

        let gbpjpy = quote(&Instrument::global("GBPJPY=X")).await.unwrap().value;
        assert!(gbpjpy.price > 50.0, "{}", gbpjpy.price);

        let h = price_history(&Instrument::global("EURUSD=X")).await.unwrap().value;
        println!("EURUSD history {} last {:?}", h.len(), h.last());
        assert!(h.len() > 2000, "{} points", h.len());
    }
}
