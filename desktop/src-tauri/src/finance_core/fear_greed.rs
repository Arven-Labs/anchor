// Crypto Fear & Greed Index (alternative.me, pública e sem chave) — porta de
// `lume/api/app/sources/cripto_feargreed.py`. `GET https://api.alternative.me/fng/?limit=1`
// devolve a leitura mais recente: `data: [{value, value_classification, timestamp, ...}]`
// com tudo como string. Índice global do mercado, não por moeda.

use chrono::{DateTime, NaiveDate};
use serde::Deserialize;

use crate::error::AppError;
use crate::finance_core::http;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "alternative.me";
const URL: &str = "https://api.alternative.me/fng/?limit=1";

#[derive(Debug, Clone, PartialEq)]
pub struct FearGreed {
    pub value: i32,
    pub classification: String,
    pub reading_date: NaiveDate,
}

#[derive(Deserialize)]
struct Response {
    data: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    value: String,
    value_classification: String,
    timestamp: String,
}

pub fn parse_latest(body: &str) -> Result<FearGreed, AppError> {
    let bad = |detail: String| AppError::ProviderUnavailable(format!("alternative.me Fear & Greed: {detail}"));
    let response: Response = serde_json::from_str(body).map_err(|e| bad(e.to_string()))?;
    let entry = response.data.into_iter().next().ok_or_else(|| bad("empty data".into()))?;
    let timestamp: i64 = entry.timestamp.parse().map_err(|_| bad(format!("invalid timestamp '{}'", entry.timestamp)))?;
    Ok(FearGreed {
        value: entry.value.parse().map_err(|_| bad(format!("invalid value '{}'", entry.value)))?,
        classification: entry.value_classification,
        reading_date: DateTime::from_timestamp(timestamp, 0)
            .ok_or_else(|| bad("timestamp out of range".into()))?
            .date_naive(),
    })
}

pub trait FearGreedProvider: DataProvider {
    fn latest<'a>(&'a self) -> BoxFuture<'a, Result<FearGreed, AppError>>;
}

pub struct AlternativeMe;

impl DataProvider for AlternativeMe {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl FearGreedProvider for AlternativeMe {
    fn latest<'a>(&'a self) -> BoxFuture<'a, Result<FearGreed, AppError>> {
        Box::pin(async move { parse_latest(&http::get_text("alternative.me", URL, http::DEFAULT_TIMEOUT).await?) })
    }
}

fn providers() -> Vec<Box<dyn FearGreedProvider>> {
    vec![Box::new(AlternativeMe)]
}

pub async fn latest() -> Result<Sourced<FearGreed>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.latest())).collect();
    cascade("Fear & Greed", attempts).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_latest_reading() {
        let body = r#"{"name":"Fear and Greed Index","data":[{"value":"34","value_classification":"Fear","timestamp":"1792022400","time_until_update":"3600"}],"metadata":{"error":null}}"#;
        assert_eq!(
            parse_latest(body).unwrap(),
            FearGreed {
                value: 34,
                classification: "Fear".into(),
                reading_date: NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()
            }
        );
    }

    #[test]
    fn malformed_payloads_are_provider_unavailable() {
        for body in [
            r#"{"data":[]}"#,
            r#"{"data":[{"value":"x","value_classification":"Fear","timestamp":"1"}]}"#,
            "<html>",
        ] {
            assert!(matches!(parse_latest(body), Err(AppError::ProviderUnavailable(_))), "{body}");
        }
    }

    // Ao vivo (rede): `cargo test --lib -- --ignored finance_core::fear_greed`.
    #[tokio::test]
    #[ignore]
    async fn live_latest_reading() {
        let reading = latest().await.unwrap();
        println!("{:?}", reading.value);
        assert!((0..=100).contains(&reading.value.value));
    }
}
