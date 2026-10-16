// Fase 26 — `DataProvider`: toda fonte de dado nasce atrás de um trait por
// categoria (ex.: `MacroSeriesProvider`), pra cascata grátis → BYO-key →
// premium sem reescrever os chamadores. `cascade` generaliza o que
// `crypto_sources::binance_then_bybit` já fazia com duas fontes fixas.

use std::future::Future;
use std::pin::Pin;

use crate::error::AppError;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait DataProvider: Send + Sync {
    /// Identificador gravado em `source` junto do dado (proveniência).
    fn id(&self) -> &'static str;
}

/// Valor e o provider que de fato respondeu.
#[derive(Debug, Clone, PartialEq)]
pub struct Sourced<T> {
    pub value: T,
    pub source: &'static str,
}

/// Tenta cada provider em ordem e devolve o primeiro que responde. `NoData` e
/// `ProviderUnavailable` seguem para o próximo (outro provider pode ter o dado).
/// No fim, se todos disseram "sem dado" o erro é `NoData`; se algum caiu, é
/// `ProviderUnavailable`, com o motivo de cada um.
pub async fn cascade<'a, T>(
    metric: &str,
    attempts: Vec<(&'static str, BoxFuture<'a, Result<T, AppError>>)>,
) -> Result<Sourced<T>, AppError> {
    let mut failures: Vec<String> = Vec::new();
    let mut any_unavailable = attempts.is_empty();
    for (source, attempt) in attempts {
        match attempt.await {
            Ok(value) => return Ok(Sourced { value, source }),
            Err(err) => {
                if !matches!(err, AppError::NoData(_)) {
                    any_unavailable = true;
                }
                failures.push(format!("{source} failed ({err})"));
            }
        }
    }
    let detail = format!("{metric}: {}", failures.join("; "));
    Err(if any_unavailable {
        AppError::ProviderUnavailable(detail)
    } else {
        AppError::NoData(detail)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(v: i32) -> BoxFuture<'static, Result<i32, AppError>> {
        Box::pin(async move { Ok(v) })
    }
    fn no_data() -> BoxFuture<'static, Result<i32, AppError>> {
        Box::pin(async { Err(AppError::NoData("none".into())) })
    }
    fn down() -> BoxFuture<'static, Result<i32, AppError>> {
        Box::pin(async { Err(AppError::ProviderUnavailable("down".into())) })
    }

    #[tokio::test]
    async fn first_provider_wins() {
        let got = cascade("m", vec![("a", ok(1)), ("b", ok(2))]).await.unwrap();
        assert_eq!(got, Sourced { value: 1, source: "a" });
    }

    #[tokio::test]
    async fn falls_through_no_data_and_unavailable() {
        assert_eq!(cascade("m", vec![("a", no_data()), ("b", ok(2))]).await.unwrap().source, "b");
        assert_eq!(cascade("m", vec![("a", down()), ("b", ok(2))]).await.unwrap().source, "b");
    }

    #[tokio::test]
    async fn all_no_data_is_no_data() {
        let err = cascade("m", vec![("a", no_data()), ("b", no_data())]).await.unwrap_err();
        assert!(matches!(err, AppError::NoData(_)));
    }

    #[tokio::test]
    async fn any_outage_is_unavailable_and_lists_every_reason() {
        let err = cascade("m", vec![("a", no_data()), ("b", down())]).await.unwrap_err();
        let AppError::ProviderUnavailable(msg) = err else { panic!("wrong variant") };
        assert!(msg.contains("a failed") && msg.contains("b failed"), "{msg}");
    }
}
