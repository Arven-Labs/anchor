// Frankfurter (taxas de referência do BCE, pública, sem chave) — segunda fonte
// de câmbio, atrás do Yahoo na cascata de `MarketDataProvider`. Só atende pares
// de moeda (`EURUSD=X`, o mesmo símbolo que o Yahoo usa); qualquer outro
// instrumento é `NoData`, pra a cascata seguir sem erro. O BCE publica um valor
// por dia útil europeu, sem técnicos nem dividendos.
//   GET /v1/latest?base=USD&symbols=BRL
//   GET /v1/{início}..{fim}?base=USD&symbols=BRL   (uma linha por dia útil)

use std::collections::BTreeMap;

use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use serde::Deserialize;

use crate::error::AppError;
use crate::finance_core::http;
use crate::finance_core::market::{
    DividendPayment, Instrument, MarketDataProvider, ProviderResult, Quote, Technicals,
};
use crate::finance_core::provider::DataProvider;

pub const SOURCE: &str = "frankfurter";
const BASE_URL: &str = "https://api.frankfurter.dev/v1";
const HISTORY_YEARS: i64 = 10;

pub struct Frankfurter;

#[derive(Deserialize)]
struct LatestResponse {
    rates: BTreeMap<String, f64>,
}

#[derive(Deserialize)]
struct RangeResponse {
    rates: BTreeMap<String, BTreeMap<String, f64>>,
}

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("Frankfurter: {detail}"))
}

/// `EURUSD=X` → `("EUR", "USD")`. Qualquer outro símbolo (ação, índice, futuro) é `None`.
pub fn pair_of(ticker: &str) -> Option<(String, String)> {
    let code = ticker.strip_suffix("=X")?;
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    Some((code[..3].to_string(), code[3..].to_string()))
}

fn require_pair(instrument: &Instrument) -> Result<(String, String), AppError> {
    pair_of(&instrument.ticker)
        .ok_or_else(|| AppError::NoData(format!("Frankfurter: '{}' is not a currency pair", instrument.ticker)))
}

pub fn parse_latest(body: &str, quote: &str) -> Result<f64, AppError> {
    let parsed: LatestResponse = serde_json::from_str(body).map_err(unavailable)?;
    parsed
        .rates
        .get(quote)
        .copied()
        .ok_or_else(|| AppError::NoData(format!("Frankfurter: no rate for {quote}")))
}

/// Série diária do mais antigo ao mais recente.
pub fn parse_range(body: &str, quote: &str) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let parsed: RangeResponse = serde_json::from_str(body).map_err(unavailable)?;
    let mut points: Vec<(NaiveDate, f64)> = parsed
        .rates
        .iter()
        .filter_map(|(day, rates)| {
            let date = NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()?;
            Some((date, *rates.get(quote)?))
        })
        .collect();
    points.sort_by_key(|(date, _)| *date);
    if points.is_empty() {
        return Err(AppError::NoData(format!("Frankfurter: no history for {quote}")));
    }
    Ok(points)
}

impl DataProvider for Frankfurter {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl MarketDataProvider for Frankfurter {
    fn quote<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Quote> {
        Box::pin(async move {
            let (base, quote) = require_pair(instrument)?;
            let url = format!("{BASE_URL}/latest?base={base}&symbols={quote}");
            let body = http::get_text("Frankfurter", &url, http::DEFAULT_TIMEOUT).await?;
            Ok(Quote {
                price: parse_latest(&body, &quote)?,
                name: Some(format!("{base}/{quote}")),
                exchange: Some("FX".to_string()),
                currency: Some(quote),
            })
        })
    }

    fn price_history<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Vec<(NaiveDate, f64)>> {
        Box::pin(async move {
            let (base, quote) = require_pair(instrument)?;
            let end = Utc::now().date_naive();
            let start = end - ChronoDuration::days(365 * HISTORY_YEARS);
            let url = format!("{BASE_URL}/{start}..{end}?base={base}&symbols={quote}");
            let body = http::get_text("Frankfurter", &url, http::DEFAULT_TIMEOUT).await?;
            parse_range(&body, &quote)
        })
    }

    fn technicals<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Technicals> {
        Box::pin(async move {
            Err(AppError::NoData(format!("Frankfurter: no technicals for '{}'", instrument.ticker)))
        })
    }

    fn dividends_avg<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, f64> {
        Box::pin(async move {
            Err(AppError::NoData(format!("Frankfurter: no dividends for '{}'", instrument.ticker)))
        })
    }

    fn dividend_payments<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Vec<DividendPayment>> {
        Box::pin(async move {
            Err(AppError::NoData(format!("Frankfurter: no dividends for '{}'", instrument.ticker)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_comes_from_the_yahoo_symbol() {
        assert_eq!(pair_of("EURUSD=X"), Some(("EUR".into(), "USD".into())));
        assert_eq!(pair_of("PETR4.SA"), None);
        assert_eq!(pair_of("GC=F"), None);
        assert_eq!(pair_of("^BVSP"), None);
        assert_eq!(pair_of("eurusd=X"), None);
    }

    #[test]
    fn latest_picks_the_requested_quote() {
        let body = r#"{"amount":1.0,"base":"USD","date":"2026-10-16","rates":{"BRL":5.0828}}"#;
        assert_eq!(parse_latest(body, "BRL").unwrap(), 5.0828);
        assert!(matches!(parse_latest(body, "EUR"), Err(AppError::NoData(_))));
    }

    #[test]
    fn range_is_sorted_oldest_first_and_skips_other_quotes() {
        let body = r#"{"base":"USD","rates":{
            "2026-09-03":{"BRL":5.074},
            "2026-09-01":{"BRL":5.1989},
            "2026-09-02":{"EUR":0.9}}}"#;
        let points = parse_range(body, "BRL").unwrap();
        assert_eq!(
            points,
            vec![
                (NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(), 5.1989),
                (NaiveDate::from_ymd_opt(2026, 9, 3).unwrap(), 5.074),
            ]
        );
    }

    #[test]
    fn empty_range_is_no_data_and_garbage_is_unavailable() {
        assert!(matches!(parse_range(r#"{"rates":{}}"#, "BRL"), Err(AppError::NoData(_))));
        assert!(matches!(parse_range("<html>", "BRL"), Err(AppError::ProviderUnavailable(_))));
    }

    #[tokio::test]
    async fn non_pair_instrument_is_no_data_without_network() {
        let err = Frankfurter.quote(&Instrument::global("PETR4")).await.unwrap_err();
        assert!(matches!(err, AppError::NoData(_)));
    }
}
