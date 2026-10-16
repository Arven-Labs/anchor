// bolsai (fundamentos de ações da B3) — porta de
// `lume/api/app/sources/acoes_bolsai.py`. `GET https://api.usebolsai.com/api/v1/fundamentals/{ticker}`
// com `X-API-Key` (chave grátis, login Google). É o primeiro provider BYO-key:
// a chave vem do keyring/ambiente (`api_keys`).
//
// Aviso herdado do Lume: o `roe` da bolsai mistura lucro trimestral e TTM sem
// dizer qual; o Anchor usa o ROE da CVM (`cvm_code` serve pra encadear). Aqui
// o dado sai como a fonte entrega.

use serde::Deserialize;

use crate::error::AppError;
use crate::finance_core::api_keys::{self, ApiKeySpec};
use crate::finance_core::http;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "bolsai";
const BASE_URL: &str = "https://api.usebolsai.com/api/v1";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq)]
pub struct BrFundamentals {
    pub lpa: f64,
    pub vpa: f64,
    pub roe: f64,
    pub shares_outstanding: f64,
    pub cvm_code: String,
}

#[derive(Deserialize)]
struct Payload {
    lpa: Option<f64>,
    vpa: Option<f64>,
    roe: Option<f64>,
    shares_outstanding: Option<f64>,
    cvm_code: Option<serde_json::Value>,
}

pub fn parse_fundamentals(body: &str) -> Result<BrFundamentals, AppError> {
    let bad = |detail: String| AppError::ProviderUnavailable(format!("bolsai: {detail}"));
    let payload: Payload = serde_json::from_str(body).map_err(|e| bad(e.to_string()))?;
    let field = |name: &str, value: Option<f64>| value.ok_or_else(|| bad(format!("response missing '{name}'")));
    let cvm_code = match payload.cvm_code {
        Some(serde_json::Value::String(s)) => s,
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => return Err(bad("response missing 'cvm_code'".into())),
    };
    Ok(BrFundamentals {
        lpa: field("lpa", payload.lpa)?,
        vpa: field("vpa", payload.vpa)?,
        roe: field("roe", payload.roe)?,
        shares_outstanding: field("shares_outstanding", payload.shares_outstanding)?,
        cvm_code,
    })
}

/// LPA, VPA, ROE, ações e código CVM de uma empresa da B3.
pub trait BrFundamentalsProvider: DataProvider {
    fn fundamentals<'a>(&'a self, ticker: &'a str) -> BoxFuture<'a, Result<BrFundamentals, AppError>>;
}

pub struct Bolsai {
    key: &'static ApiKeySpec,
}

impl DataProvider for Bolsai {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl BrFundamentalsProvider for Bolsai {
    fn fundamentals<'a>(&'a self, ticker: &'a str) -> BoxFuture<'a, Result<BrFundamentals, AppError>> {
        Box::pin(async move {
            let key = api_keys::resolve(self.key)?;
            let url = format!("{BASE_URL}/fundamentals/{ticker}");
            parse_fundamentals(&http::get_text_with_headers("bolsai", &url, TIMEOUT, &[("X-API-Key", &key)]).await?)
        })
    }
}

fn providers() -> Vec<Box<dyn BrFundamentalsProvider>> {
    vec![Box::new(Bolsai { key: &api_keys::BOLSAI })]
}

pub async fn fundamentals(ticker: &str) -> Result<Sourced<BrFundamentals>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.fundamentals(ticker))).collect();
    cascade(&format!("fundamentals for '{ticker}'"), attempts).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_and_string_cvm_codes() {
        let body = |cvm: &str| {
            format!(r#"{{"lpa":1.5,"vpa":20.0,"roe":12.3,"shares_outstanding":1000000,"cvm_code":{cvm},"extra":1}}"#)
        };
        let expected = |cvm: &str| BrFundamentals {
            lpa: 1.5,
            vpa: 20.0,
            roe: 12.3,
            shares_outstanding: 1_000_000.0,
            cvm_code: cvm.into(),
        };
        assert_eq!(parse_fundamentals(&body("9512")).unwrap(), expected("9512"));
        assert_eq!(parse_fundamentals(&body("\"9512\"")).unwrap(), expected("9512"));
    }

    #[test]
    fn missing_or_null_fields_are_provider_unavailable() {
        for body in [
            r#"{"lpa":1,"vpa":2,"roe":3,"shares_outstanding":4}"#,
            r#"{"lpa":null,"vpa":2,"roe":3,"shares_outstanding":4,"cvm_code":1}"#,
            "not json",
        ] {
            assert!(matches!(parse_fundamentals(body), Err(AppError::ProviderUnavailable(_))), "{body}");
        }
    }
}
