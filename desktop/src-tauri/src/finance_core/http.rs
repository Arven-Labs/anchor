// Cliente HTTP compartilhado das fontes públicas. Toda falha vira um dos
// dois erros do contrato: `NoData` (404, a fonte não tem o dado) ou
// `ProviderUnavailable` (rede, HTTP != 2xx, corpo inválido).

use std::time::Duration;

use crate::error::AppError;

pub const USER_AGENT: &str = concat!("anchor/", env!("CARGO_PKG_VERSION"));
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

fn snippet(body: &str) -> String {
    body.chars().take(200).collect()
}

/// Mapeia status HTTP + corpo para o erro do contrato. Separado de `get_text`
/// pra ser testável sem rede.
pub fn check_status(source: &str, status: u16, body: String) -> Result<String, AppError> {
    match status {
        200..=299 => Ok(body),
        404 => Err(AppError::NoData(format!("{source}: HTTP 404 — {}", snippet(&body)))),
        _ => Err(AppError::ProviderUnavailable(format!(
            "{source}: HTTP {status} — {}",
            snippet(&body)
        ))),
    }
}

pub async fn get_text(source: &str, url: &str, timeout: Duration) -> Result<String, AppError> {
    get_text_with_headers(source, url, timeout, &[]).await
}

/// Igual a `get_text`, com cabeçalhos extras (ex.: `X-API-Key` de provider com chave).
pub async fn get_text_with_headers(
    source: &str,
    url: &str,
    timeout: Duration,
    headers: &[(&str, &str)],
) -> Result<String, AppError> {
    let unavailable = |detail: reqwest::Error| {
        AppError::ProviderUnavailable(format!("{source}: {detail}"))
    };
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(USER_AGENT)
        .build()
        .map_err(unavailable)?;
    let mut request = client.get(url);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = request.send().await.map_err(unavailable)?;
    let status = response.status().as_u16();
    let body = response.text().await.map_err(unavailable)?;
    check_status(source, status, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_returns_the_body() {
        assert_eq!(check_status("x", 200, "ok".into()).unwrap(), "ok");
    }

    #[test]
    fn not_found_is_no_data() {
        assert!(matches!(check_status("x", 404, "nope".into()), Err(AppError::NoData(_))));
    }

    #[test]
    fn other_failures_are_provider_unavailable() {
        for status in [400, 406, 500, 502] {
            assert!(
                matches!(check_status("x", status, "bad".into()), Err(AppError::ProviderUnavailable(_))),
                "status {status}"
            );
        }
    }
}
