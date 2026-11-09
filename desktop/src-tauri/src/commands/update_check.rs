// Fase 6.5 / P9 — checagem de atualização a partir do `latest.json` da release,
// o mesmo arquivo que o instalador (`update_install.rs`) usa. Antes a checagem
// perguntava à API do GitHub e o instalador ao `latest.json`: o aviso podia
// dizer "há versão nova" e a instalação falhar porque o arquivo ainda não
// subiu. Agora há uma fonte só. A chamada é feita em Rust, e não pelo webview,
// para não abrir o `connect-src` da CSP.

use serde::Serialize;
use tauri::{AppHandle, State, Url};
use tauri_plugin_updater::UpdaterExt;

use super::update_install::PendingUpdate;
use crate::error::AppError;

// O link é montado aqui, a partir de uma versão já validada como MAJOR.MINOR.PATCH,
// e nunca copiado do `latest.json`: dado externo não vira URL que o usuário abre.
const RELEASE_URL_PREFIX: &str = "https://github.com/Arven-Labs/anchor/releases/tag/";

// Ganchos de teste (E2E). Só existem em build de debug: em release `updater_override`
// devolve `None` antes de olhar o ambiente, então ninguém redireciona o instalador
// nem troca a chave pública de um app de verdade.
const ENDPOINT_ENV: &str = "ANCHOR_UPDATER_ENDPOINT";
const PUBKEY_ENV: &str = "ANCHOR_UPDATER_PUBKEY";

#[derive(Debug, Serialize)]
pub struct UpdateStatus {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
}

#[derive(Debug)]
struct UpdaterOverride {
    endpoint: Url,
    pubkey: String,
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

/// Estado mostrado na tela. `found` é a versão que o plugin achou no `latest.json`
/// (`None` = nada mais novo). Confere de novo que ela é maior que a atual: o
/// instalador nunca deve oferecer igual ou mais antiga (downgrade).
fn status_from(current: &str, found: Option<&str>) -> Result<UpdateStatus, AppError> {
    let Some(version) = found else {
        return Ok(UpdateStatus {
            current: current.to_string(),
            latest: Some(current.to_string()),
            update_available: false,
            release_url: None,
        });
    };
    let (major, minor, patch) = parse_version(version)?;
    let latest = format!("{major}.{minor}.{patch}");
    let update_available = is_newer(current, version)?;
    Ok(UpdateStatus {
        current: current.to_string(),
        release_url: update_available.then(|| format!("{RELEASE_URL_PREFIX}v{latest}")),
        latest: Some(latest),
        update_available,
    })
}

/// 404 no `latest.json` = nenhuma release publicada ainda, o que não é erro.
fn no_release_or_error(current: &str, error: AppError) -> Result<UpdateStatus, AppError> {
    match error {
        AppError::NoData(_) => Ok(UpdateStatus {
            current: current.to_string(),
            latest: None,
            update_available: false,
            release_url: None,
        }),
        other => Err(other),
    }
}

/// Traduz o erro do plugin para o do app. Falha de assinatura é recusa (nada foi
/// instalado); o resto é indisponibilidade da fonte.
pub(crate) fn map_updater_error(error: tauri_plugin_updater::Error) -> AppError {
    use tauri_plugin_updater::Error as E;
    match error {
        E::ReleaseNotFound => AppError::NoData("updater: no release published".to_string()),
        E::Minisign(_)
        | E::SignatureUtf8(_)
        | E::Base64(_)
        | E::SignedVersionMismatch { .. }
        | E::MissingSignedVersion => AppError::UpdateRejected(error.to_string()),
        other => AppError::ProviderUnavailable(format!("updater: {other}")),
    }
}

/// Endpoint e chave de teste vindos do ambiente. Só em debug (`debug`); em release
/// ignora o ambiente por completo. Se só uma das duas variáveis vier, é erro (config
/// pela metade nunca cai no endpoint de verdade em silêncio). O endpoint tem de ser
/// loopback: o gancho serve a um servidor local de teste, não a um host qualquer.
fn updater_override(
    debug: bool,
    endpoint: Option<String>,
    pubkey: Option<String>,
) -> Result<Option<UpdaterOverride>, AppError> {
    let (endpoint, pubkey) = match (endpoint, pubkey) {
        (None, None) => return Ok(None),
        (Some(endpoint), Some(pubkey)) => (endpoint, pubkey),
        _ => {
            return Err(AppError::InvalidInput(format!(
                "{ENDPOINT_ENV} and {PUBKEY_ENV} must be set together"
            )))
        }
    };
    let url = Url::parse(&endpoint)
        .map_err(|e| AppError::InvalidInput(format!("{ENDPOINT_ENV} is not a URL ({e})")))?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if !loopback || !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput(format!(
            "{ENDPOINT_ENV} must be an http(s) URL on localhost"
        )));
    }
    let pubkey = pubkey.trim().to_string();
    if pubkey.is_empty() {
        return Err(AppError::InvalidInput(format!("{PUBKEY_ENV} is empty")));
    }
    Ok(Some(UpdaterOverride {
        endpoint: url,
        pubkey,
    }))
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<UpdateStatus, AppError> {
    let current = env!("CARGO_PKG_VERSION");
    // Uma checagem nova invalida o que a anterior achou ou baixou.
    pending.clear();

    let overridden = updater_override(
        cfg!(debug_assertions),
        std::env::var(ENDPOINT_ENV).ok(),
        std::env::var(PUBKEY_ENV).ok(),
    )?;
    let mut builder = app.updater_builder();
    if let Some(o) = overridden {
        builder = builder
            .endpoints(vec![o.endpoint])
            .map_err(map_updater_error)?
            .pubkey(o.pubkey);
    }
    let updater = builder.build().map_err(map_updater_error)?;

    let found = match updater.check().await {
        Ok(found) => found,
        Err(error) => return no_release_or_error(current, map_updater_error(error)),
    };
    let status = status_from(
        current,
        found.as_ref().map(|update| update.version.as_str()),
    )?;
    if let (true, Some(update)) = (status.update_available, found) {
        pending.set_found(update);
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCAL_ENDPOINT: &str = "http://127.0.0.1:47831/latest.json";
    const KEY: &str = "dGVzdC1rZXk=";

    #[test]
    fn a_higher_version_is_an_update_with_a_link_built_from_it() {
        let status = status_from("0.1.0", Some("0.2.0")).unwrap();
        assert!(status.update_available);
        assert_eq!(status.latest.as_deref(), Some("0.2.0"));
        assert_eq!(
            status.release_url.as_deref(),
            Some("https://github.com/Arven-Labs/anchor/releases/tag/v0.2.0")
        );
    }

    #[test]
    fn the_same_or_an_older_version_is_never_an_update() {
        for found in ["0.1.0", "0.0.9", "v0.1.0"] {
            let status = status_from("0.1.0", Some(found)).unwrap();
            assert!(!status.update_available, "{found}");
            assert_eq!(status.release_url, None, "{found}");
        }
    }

    #[test]
    fn nothing_newer_found_reports_the_current_version_as_latest() {
        let status = status_from("0.1.0", None).unwrap();
        assert!(!status.update_available);
        assert_eq!(status.latest.as_deref(), Some("0.1.0"));
        assert_eq!(status.release_url, None);
        assert_eq!(status.current, "0.1.0");
    }

    // O link nasce da versão; uma versão que não é MAJOR.MINOR.PATCH nunca chega a virar URL.
    #[test]
    fn a_found_version_that_is_not_plain_semver_is_an_error() {
        for version in [
            "0.2.0/../../evil",
            "1.0.0-beta",
            "0.2.0?x=1",
            "latest",
            "",
            "0.2",
        ] {
            assert!(
                matches!(
                    status_from("0.1.0", Some(version)),
                    Err(AppError::InvalidInput(_))
                ),
                "{version:?}"
            );
        }
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
        for tag in [
            "v1.0.0-beta",
            "latest",
            "",
            "v1.2",
            "v1.2.3.4",
            "v1.x.0",
            "v-1.0.0",
        ] {
            assert!(
                matches!(is_newer("0.1.0", tag), Err(AppError::InvalidInput(_))),
                "{tag:?}"
            );
        }
    }

    #[test]
    fn an_invalid_app_version_is_an_error() {
        assert!(matches!(
            is_newer("dev", "v0.2.0"),
            Err(AppError::InvalidInput(_))
        ));
    }

    #[test]
    fn no_release_published_yet_is_not_an_error() {
        let status = no_release_or_error("0.1.0", AppError::NoData("404".to_string())).unwrap();
        assert!(!status.update_available);
        assert_eq!(status.latest, None);
        assert_eq!(status.release_url, None);
        assert_eq!(status.current, "0.1.0");
    }

    #[test]
    fn a_network_failure_stays_an_error() {
        let down = AppError::ProviderUnavailable("updater: connection refused".to_string());
        assert!(matches!(
            no_release_or_error("0.1.0", down),
            Err(AppError::ProviderUnavailable(_))
        ));
    }

    #[test]
    fn a_missing_release_maps_to_no_data() {
        assert!(matches!(
            map_updater_error(tauri_plugin_updater::Error::ReleaseNotFound),
            AppError::NoData(_)
        ));
    }

    #[test]
    fn a_signature_problem_maps_to_a_rejection_not_to_an_outage() {
        let error = tauri_plugin_updater::Error::SignatureUtf8("bad".to_string());
        assert!(matches!(
            map_updater_error(error),
            AppError::UpdateRejected(_)
        ));
    }

    #[test]
    fn a_download_failure_maps_to_provider_unavailable() {
        let error = tauri_plugin_updater::Error::Network("reset".to_string());
        assert!(matches!(
            map_updater_error(error),
            AppError::ProviderUnavailable(_)
        ));
    }

    // Em release o ambiente é ignorado por completo, mesmo com as duas variáveis certas.
    #[test]
    fn release_builds_ignore_the_test_hooks() {
        let hook = updater_override(
            false,
            Some(LOCAL_ENDPOINT.to_string()),
            Some(KEY.to_string()),
        );
        assert!(hook.unwrap().is_none());
    }

    #[test]
    fn release_builds_ignore_even_a_half_configured_hook() {
        assert!(
            updater_override(false, Some(LOCAL_ENDPOINT.to_string()), None)
                .unwrap()
                .is_none()
        );
        assert!(updater_override(false, None, Some(KEY.to_string()))
            .unwrap()
            .is_none());
    }

    #[test]
    fn debug_without_the_variables_uses_the_real_endpoint() {
        assert!(updater_override(true, None, None).unwrap().is_none());
    }

    #[test]
    fn debug_with_both_variables_overrides_endpoint_and_key() {
        let hook = updater_override(
            true,
            Some(LOCAL_ENDPOINT.to_string()),
            Some(format!("  {KEY}\n")),
        )
        .unwrap()
        .unwrap();
        assert_eq!(hook.endpoint.as_str(), LOCAL_ENDPOINT);
        assert_eq!(hook.pubkey, KEY);
    }

    #[test]
    fn debug_with_only_one_variable_is_an_error() {
        assert!(matches!(
            updater_override(true, Some(LOCAL_ENDPOINT.to_string()), None),
            Err(AppError::InvalidInput(_))
        ));
        assert!(matches!(
            updater_override(true, None, Some(KEY.to_string())),
            Err(AppError::InvalidInput(_))
        ));
    }

    #[test]
    fn the_hook_only_accepts_a_local_http_endpoint() {
        for endpoint in [
            "https://evil.example/latest.json",
            "http://127.0.0.1.evil.example/latest.json",
            "ftp://127.0.0.1/latest.json",
            "file:///tmp/latest.json",
            "not a url",
            "",
        ] {
            assert!(
                matches!(
                    updater_override(true, Some(endpoint.to_string()), Some(KEY.to_string())),
                    Err(AppError::InvalidInput(_))
                ),
                "{endpoint:?}"
            );
        }
        for endpoint in [
            "http://localhost:4599/latest.json",
            "http://[::1]:4599/latest.json",
            "https://127.0.0.1/latest.json",
        ] {
            assert!(
                updater_override(true, Some(endpoint.to_string()), Some(KEY.to_string()))
                    .unwrap()
                    .is_some(),
                "{endpoint:?}"
            );
        }
    }

    #[test]
    fn the_hook_refuses_an_empty_key() {
        assert!(matches!(
            updater_override(
                true,
                Some(LOCAL_ENDPOINT.to_string()),
                Some("  \n".to_string())
            ),
            Err(AppError::InvalidInput(_))
        ));
    }
}
