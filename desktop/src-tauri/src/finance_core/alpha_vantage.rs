// Alpha Vantage (chave gratuita do próprio usuário, BYO-key) — segunda fonte de
// cotação e histórico de ações, atrás do Yahoo na cascata de `MarketDataProvider`.
//   GET /query?function=GLOBAL_QUOTE&symbol=IBM&apikey=...
//   GET /query?function=TIME_SERIES_DAILY&symbol=IBM&outputsize=full&apikey=...
// B3 usa o sufixo `.SAO` (`PETR4.SAO`). Só cotação e fechamento diário: técnicos
// e dividendos ficam `NoData` (o Yahoo os atende; a cascata segue). Só entra na
// lista de providers quando há chave configurada, pra um ticker inexistente
// continuar saindo como `NoData` em vez de "provider fora do ar".
//
// A API responde 200 mesmo quando falha: `Error Message` = símbolo inválido
// (`NoData`); `Note`/`Information` = limite diário ou recurso premium
// (`ProviderUnavailable`).

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde_json::Value;

use crate::error::AppError;
use crate::finance_core::api_keys::{self, ApiKeySpec};
use crate::finance_core::http;
use crate::finance_core::market::{
    DividendPayment, Instrument, Market, MarketDataProvider, ProviderResult, Quote, Technicals,
};
use crate::finance_core::provider::DataProvider;

pub const SOURCE: &str = "alpha_vantage";
const URL: &str = "https://www.alphavantage.co/query";

pub struct AlphaVantage {
    pub key: &'static ApiKeySpec,
}

/// Há chave configurada (cofre ou ambiente)? Sem ela o provider nem entra na cascata.
pub fn is_configured() -> bool {
    api_keys::status(&api_keys::ALPHA_VANTAGE) != api_keys::KeySource::None
}

fn unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::ProviderUnavailable(format!("Alpha Vantage: {detail}"))
}

/// Símbolo da Alpha Vantage, ou `None` pro que ela não atende (câmbio `XXXYYY=X`,
/// futuros `GC=F`, índices `^BVSP`).
pub fn symbol_of(instrument: &Instrument) -> Option<String> {
    let ticker = instrument.ticker.trim();
    if ticker.is_empty() || ticker.contains('=') || ticker.starts_with('^') {
        return None;
    }
    Some(match instrument.market {
        Market::B3 => format!("{ticker}.SAO"),
        Market::Global => ticker.to_string(),
    })
}

fn require_symbol(instrument: &Instrument) -> Result<String, AppError> {
    symbol_of(instrument)
        .ok_or_else(|| AppError::NoData(format!("Alpha Vantage: no support for '{}'", instrument.ticker)))
}

/// Erros que a API devolve com HTTP 200.
fn check_envelope(json: &Value) -> Result<(), AppError> {
    if let Some(message) = json.get("Error Message").and_then(Value::as_str) {
        return Err(AppError::NoData(format!("Alpha Vantage: {message}")));
    }
    for field in ["Note", "Information"] {
        if let Some(message) = json.get(field).and_then(Value::as_str) {
            return Err(unavailable(message));
        }
    }
    Ok(())
}

fn parse_json(body: &str) -> Result<Value, AppError> {
    let json: Value = serde_json::from_str(body).map_err(unavailable)?;
    check_envelope(&json)?;
    Ok(json)
}

pub fn parse_quote(body: &str) -> Result<Quote, AppError> {
    let json = parse_json(body)?;
    let price = json
        .get("Global Quote")
        .and_then(|q| q.get("05. price"))
        .and_then(Value::as_str)
        .and_then(|p| p.parse::<f64>().ok())
        .ok_or_else(|| AppError::NoData("Alpha Vantage: empty quote".to_string()))?;
    Ok(Quote { price, name: None, exchange: None, currency: None })
}

/// Fechamento diário do mais antigo ao mais recente.
pub fn parse_daily(body: &str) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let json = parse_json(body)?;
    let series: BTreeMap<String, Value> = json
        .get("Time Series (Daily)")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .ok_or_else(|| AppError::NoData("Alpha Vantage: no daily series".to_string()))?;
    let mut points: Vec<(NaiveDate, f64)> = series
        .iter()
        .filter_map(|(day, row)| {
            let date = NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()?;
            let close = row.get("4. close")?.as_str()?.parse::<f64>().ok()?;
            Some((date, close))
        })
        .collect();
    points.sort_by_key(|(date, _)| *date);
    if points.is_empty() {
        return Err(AppError::NoData("Alpha Vantage: empty daily series".to_string()));
    }
    Ok(points)
}

async fn get(function_and_args: &str, symbol: &str, key: &str) -> Result<String, AppError> {
    let url = format!("{URL}?{function_and_args}&symbol={symbol}&apikey={key}");
    http::get_text("Alpha Vantage", &url, http::DEFAULT_TIMEOUT).await
}

impl DataProvider for AlphaVantage {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl MarketDataProvider for AlphaVantage {
    fn quote<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Quote> {
        Box::pin(async move {
            let symbol = require_symbol(instrument)?;
            let key = api_keys::resolve(self.key)?;
            parse_quote(&get("function=GLOBAL_QUOTE", &symbol, &key).await?)
        })
    }

    fn price_history<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Vec<(NaiveDate, f64)>> {
        Box::pin(async move {
            let symbol = require_symbol(instrument)?;
            let key = api_keys::resolve(self.key)?;
            // A série completa (20 anos) pode ser recurso premium na chave grátis;
            // nesse caso vale a compacta (~100 dias) a ficar sem histórico.
            let full = get("function=TIME_SERIES_DAILY&outputsize=full", &symbol, &key).await?;
            match parse_daily(&full) {
                Err(AppError::ProviderUnavailable(msg)) if msg.contains("premium") => {
                    parse_daily(&get("function=TIME_SERIES_DAILY&outputsize=compact", &symbol, &key).await?)
                }
                other => other,
            }
        })
    }

    fn technicals<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Technicals> {
        Box::pin(async move {
            Err(AppError::NoData(format!("Alpha Vantage: no technicals for '{}'", instrument.ticker)))
        })
    }

    fn dividends_avg<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, f64> {
        Box::pin(async move {
            Err(AppError::NoData(format!("Alpha Vantage: no dividends for '{}'", instrument.ticker)))
        })
    }

    fn dividend_payments<'a>(&'a self, instrument: &'a Instrument) -> ProviderResult<'a, Vec<DividendPayment>> {
        Box::pin(async move {
            Err(AppError::NoData(format!("Alpha Vantage: no dividends for '{}'", instrument.ticker)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_follow_the_market_and_skip_unsupported_instruments() {
        assert_eq!(symbol_of(&Instrument::b3("PETR4")), Some("PETR4.SAO".to_string()));
        assert_eq!(symbol_of(&Instrument::global("AAPL")), Some("AAPL".to_string()));
        assert_eq!(symbol_of(&Instrument::global("USDBRL=X")), None);
        assert_eq!(symbol_of(&Instrument::global("GC=F")), None);
        assert_eq!(symbol_of(&Instrument::global("^BVSP")), None);
    }

    #[test]
    fn quote_reads_the_price_field() {
        let body = r#"{"Global Quote":{"01. symbol":"IBM","05. price":"232.1400"}}"#;
        assert_eq!(parse_quote(body).unwrap().price, 232.14);
        assert!(matches!(parse_quote(r#"{"Global Quote":{}}"#), Err(AppError::NoData(_))));
    }

    #[test]
    fn api_failures_inside_a_200_map_to_the_error_contract() {
        let invalid = r#"{"Error Message":"Invalid API call."}"#;
        assert!(matches!(parse_quote(invalid), Err(AppError::NoData(_))));
        let limit = r#"{"Information":"Thank you for using Alpha Vantage! daily rate limit"}"#;
        assert!(matches!(parse_quote(limit), Err(AppError::ProviderUnavailable(_))));
        let note = r#"{"Note":"call frequency"}"#;
        assert!(matches!(parse_daily(note), Err(AppError::ProviderUnavailable(_))));
        assert!(matches!(parse_quote("<html>"), Err(AppError::ProviderUnavailable(_))));
    }

    #[test]
    fn daily_series_is_sorted_oldest_first() {
        let body = r#"{"Meta Data":{},"Time Series (Daily)":{
            "2026-10-16":{"4. close":"232.1400"},
            "2026-10-14":{"4. close":"230.0000"},
            "2026-10-15":{"4. close":"234.1200"}}}"#;
        let points = parse_daily(body).unwrap();
        let days: Vec<String> = points.iter().map(|(d, _)| d.to_string()).collect();
        assert_eq!(days, vec!["2026-10-14", "2026-10-15", "2026-10-16"]);
        assert_eq!(points[2].1, 232.14);
        assert!(matches!(parse_daily(r#"{"Time Series (Daily)":{}}"#), Err(AppError::NoData(_))));
    }

    #[tokio::test]
    async fn unsupported_instrument_is_no_data_before_asking_for_a_key() {
        let provider = AlphaVantage { key: &api_keys::ALPHA_VANTAGE };
        let err = provider.quote(&Instrument::global("EURUSD=X")).await.unwrap_err();
        assert!(matches!(err, AppError::NoData(_)));
    }
}
