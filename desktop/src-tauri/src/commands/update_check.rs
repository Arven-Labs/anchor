// Fase 6.5 — aviso de atualização disponível. Só avisa: consulta a última
// release publicada no GitHub e compara com a versão do app. Não baixa nem
// instala nada (o updater completo do Tauri está nas pendências). A chamada
// é feita em Rust, e não pelo webview, para não abrir o `connect-src` da CSP.

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::finance_core::http;

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/Arven-Labs/anchor/releases/latest";
// O link vem de dado externo e vira algo que o usuário abre: só o nosso repo.
const RELEASE_URL_PREFIX: &str = "https://github.com/Arven-Labs/anchor/";

#[derive(Debug, Serialize)]
pub struct UpdateStatus {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

/// `MAJOR.MINOR.PATCH`, com `v` opcional na frente. Qualquer outra forma
/// (pré-release, só dois números, texto) é erro: nunca vira "update disponível".
fn parse_version(text: &str) -> Result<(u64, u64, u64), AppError> {
    let invalid = || AppError::InvalidInput(format!("version {text:?} is not MAJOR.MINOR.PATCH"));
    let parts: Vec<&str> = text.strip_prefix('v').unwrap_or(text).split('.').collect();
    if parts.len() != 3 {
        return Err(invalid());
    }
    let mut numbers = [0u64; 3];
    for (slot, part) in numbers.iter_mut().zip(parts) {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
        *slot = part.parse().map_err(|_| invalid())?;
    }
    Ok((numbers[0], numbers[1], numbers[2]))
}

fn is_newer(current: &str, tag: &str) -> Result<bool, AppError> {
    Ok(parse_version(tag)? > parse_version(current)?)
}

/// Transforma a resposta do GitHub (ou o erro de rede) no estado mostrado na
/// tela. 404 = nenhuma release publicada ainda, o que não é erro.
fn interpret(current: &str, fetched: Result<String, AppError>) -> Result<UpdateStatus, AppError> {
    let body = match fetched {
        Ok(body) => body,
        Err(AppError::NoData(_)) => {
            return Ok(UpdateStatus {
                current: current.to_string(),
                latest: None,
                update_available: false,
                release_url: None,
            })
        }
        Err(other) => return Err(other),
    };
    let release: GithubRelease = serde_json::from_str(&body)
        .map_err(|e| AppError::ProviderUnavailable(format!("github: unexpected release response ({e})")))?;
    if !release.html_url.starts_with(RELEASE_URL_PREFIX) {
        return Err(AppError::ProviderUnavailable("github: release link points outside the project".to_string()));
    }
    let (major, minor, patch) = parse_version(&release.tag_name)?;
    Ok(UpdateStatus {
        current: current.to_string(),
        latest: Some(format!("{major}.{minor}.{patch}")),
        update_available: is_newer(current, &release.tag_name)?,
        release_url: Some(release.html_url),
    })
}

#[tauri::command]
pub async fn check_for_update() -> Result<UpdateStatus, AppError> {
    let fetched = http::get_text_with_headers(
        "github",
        LATEST_RELEASE_URL,
        http::DEFAULT_TIMEOUT,
        &[("Accept", "application/vnd.github+json")],
    )
    .await;
    interpret(env!("CARGO_PKG_VERSION"), fetched)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_json(tag: &str, url: &str) -> String {
        format!(r#"{{"tag_name":"{tag}","html_url":"{url}","draft":false}}"#)
    }

    const OK_URL: &str = "https://github.com/Arven-Labs/anchor/releases/tag/v0.2.0";

    #[test]
    fn a_higher_release_is_an_update() {
        let status = interpret("0.1.0", Ok(release_json("v0.2.0", OK_URL))).unwrap();
        assert!(status.update_available);
        assert_eq!(status.latest.as_deref(), Some("0.2.0"));
        assert_eq!(status.release_url.as_deref(), Some(OK_URL));
    }

    #[test]
    fn the_same_version_is_not_an_update() {
        let status = interpret("0.1.0", Ok(release_json("v0.1.0", OK_URL))).unwrap();
        assert!(!status.update_available);
    }

    #[test]
    fn an_older_release_is_never_an_update() {
        let status = interpret("0.2.0", Ok(release_json("v0.1.9", OK_URL))).unwrap();
        assert!(!status.update_available);
    }

    // Comparar como texto diria que "0.1.10" < "0.1.9".
    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert!(is_newer("0.1.9", "v0.1.10").unwrap());
        assert!(!is_newer("0.1.10", "v0.1.9").unwrap());
        assert!(is_newer("0.9.0", "v0.10.0").unwrap());
    }

    #[test]
    fn a_tag_without_the_v_prefix_is_accepted() {
        assert!(is_newer("0.1.0", "0.2.0").unwrap());
    }

    #[test]
    fn a_tag_outside_major_minor_patch_is_an_error_not_an_update() {
        for tag in ["v1.0.0-beta", "latest", "", "v1.2", "v1.2.3.4", "v1.x.0", "v-1.0.0"] {
            assert!(matches!(is_newer("0.1.0", tag), Err(AppError::InvalidInput(_))), "{tag:?}");
        }
    }

    #[test]
    fn an_invalid_app_version_is_an_error() {
        assert!(matches!(is_newer("dev", "v0.2.0"), Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn a_body_without_tag_name_is_an_error() {
        let body = r#"{"html_url":"https://github.com/Arven-Labs/anchor/releases"}"#.to_string();
        assert!(interpret("0.1.0", Ok(body)).is_err());
        assert!(interpret("0.1.0", Ok("not json".to_string())).is_err());
    }

    // O link vem de dado externo e vira algo que o usuário abre: só o nosso repo.
    #[test]
    fn a_release_link_outside_our_repo_is_refused() {
        for url in [
            "https://evil.example/Arven-Labs/anchor/releases/tag/v0.2.0",
            "http://github.com/Arven-Labs/anchor/releases/tag/v0.2.0",
            "https://github.com/Arven-Labs/anchor-evil/releases/tag/v0.2.0",
            "https://github.com/Arven-Labs/anchor.evil.com/x",
            "javascript:alert(1)",
            "",
        ] {
            assert!(interpret("0.1.0", Ok(release_json("v0.2.0", url))).is_err(), "{url:?}");
        }
    }

    #[test]
    fn no_release_published_yet_is_not_an_error() {
        let status = interpret("0.1.0", Err(AppError::NoData("github: HTTP 404".to_string()))).unwrap();
        assert!(!status.update_available);
        assert_eq!(status.latest, None);
        assert_eq!(status.release_url, None);
        assert_eq!(status.current, "0.1.0");
    }

    #[test]
    fn a_network_or_rate_limit_failure_stays_an_error() {
        let rate_limited = AppError::ProviderUnavailable("github: rate limited (HTTP 429)".to_string());
        assert!(matches!(interpret("0.1.0", Err(rate_limited)), Err(AppError::ProviderUnavailable(_))));
    }
}
