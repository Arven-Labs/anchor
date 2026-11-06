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
