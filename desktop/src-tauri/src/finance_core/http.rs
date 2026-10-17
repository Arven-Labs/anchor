// Cliente HTTP compartilhado das fontes públicas. Toda falha vira um dos
// dois erros do contrato: `NoData` (404, a fonte não tem o dado) ou
// `ProviderUnavailable` (rede, HTTP != 2xx, corpo inválido).

use std::time::Duration;

use crate::error::AppError;

pub const USER_AGENT: &str = concat!("anchor/", env!("CARGO_PKG_VERSION"));
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(2);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(10);

fn snippet(body: &str) -> String {
    body.chars().take(200).collect()
}

/// Mapeia status HTTP + corpo para o erro do contrato. Separado de `get_text`
/// pra ser testável sem rede.
pub fn check_status(source: &str, status: u16, body: String) -> Result<String, AppError> {
    match status {
        200..=299 => Ok(body),
        404 => Err(AppError::NoData(format!("{source}: HTTP 404 — {}", snippet(&body)))),
        429 => Err(AppError::ProviderUnavailable(format!("{source}: rate limited (HTTP 429)"))),
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
    let mut retried = false;
    loop {
        let mut request = client.get(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.send().await.map_err(unavailable)?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = response.text().await.map_err(unavailable)?;
        if status == 429 && !retried {
            retried = true;
            tokio::time::sleep(retry_delay(retry_after.as_deref())).await;
            continue;
        }
        return check_status(source, status, body);
    }
}

/// Espera antes da única nova tentativa após um 429: o `Retry-After` em segundos,
/// limitado a `MAX_RETRY_DELAY` pra uma fonte lenta não travar a coleta.
pub fn retry_delay(retry_after: Option<&str>) -> Duration {
    let requested = retry_after.and_then(|v| v.trim().parse::<u64>().ok()).map(Duration::from_secs);
    requested.unwrap_or(DEFAULT_RETRY_DELAY).min(MAX_RETRY_DELAY)
}

/// Baixa um corpo binário (zips, JSON grande), com cabeçalhos extras (ex.: o
/// `User-Agent` com e-mail de contato da SEC). Erros seguem o contrato de `get_text`.
pub async fn get_bytes_with_headers(
    source: &str,
    url: &str,
    timeout: Duration,
    headers: &[(&str, &str)],
) -> Result<Vec<u8>, AppError> {
    let unavailable = |detail: reqwest::Error| AppError::ProviderUnavailable(format!("{source}: {detail}"));
    let client = reqwest::Client::builder().timeout(timeout).user_agent(USER_AGENT).build().map_err(unavailable)?;
    let mut request = client.get(url);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = request.send().await.map_err(unavailable)?;
    let status = response.status().as_u16();
    let body = response.bytes().await.map_err(unavailable)?;
    if (200..300).contains(&status) {
        return Ok(body.to_vec());
    }
    check_status(source, status, String::from_utf8_lossy(&body).into_owned()).map(|_| Vec::new())
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
    fn rate_limit_is_provider_unavailable_and_says_so() {
        match check_status("x", 429, "slow down".into()) {
            Err(AppError::ProviderUnavailable(msg)) => assert!(msg.contains("rate limited")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn retry_delay_honors_retry_after_up_to_the_cap() {
        assert_eq!(retry_delay(None), Duration::from_secs(2));
        assert_eq!(retry_delay(Some("5")), Duration::from_secs(5));
        assert_eq!(retry_delay(Some("600")), Duration::from_secs(10));
        assert_eq!(retry_delay(Some("Wed, 21 Oct 2026 07:28:00 GMT")), Duration::from_secs(2));
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
