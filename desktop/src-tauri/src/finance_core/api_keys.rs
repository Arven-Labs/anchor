// Credenciais de providers que exigem conta ou identificação (BYO-key). Ordem
// de busca: cofre do SO (keyring, serviço `anchor`) e depois variável de
// ambiente. Nunca em arquivo de configuração nem no banco. Sem a credencial o
// provider responde `ProviderUnavailable` com uma mensagem que diz o que
// configurar; a tela Settings → Data sources grava e remove no keyring.

use serde::Serialize;

use crate::error::AppError;

const KEYRING_SERVICE: &str = "anchor";

pub struct ApiKeySpec {
    /// Identificador estável usado pela tela e pelos comandos.
    pub id: &'static str,
    /// Nome curto do provider, nas mensagens de erro.
    pub provider: &'static str,
    /// Rótulo e ajuda mostrados na tela de credenciais.
    pub label: &'static str,
    pub help: &'static str,
    /// `true` pra segredo (a tela nunca o mostra de volta); `false` pra dado
    /// que pode ser exibido, como o e-mail de contato.
    pub secret: bool,
    pub keyring_user: &'static str,
    pub env_var: &'static str,
}

pub const BOLSAI: ApiKeySpec = ApiKeySpec {
    id: "bolsai",
    provider: "bolsai",
    label: "bolsai API key",
    help: "Free key from usebolsai.com. Needed for Brazilian stock fundamentals and for finding a real-estate fund's (FII) CNPJ from its ticker.",
    secret: true,
    keyring_user: "bolsai_api_key",
    env_var: "BOLSAI_API_KEY",
};

pub const ALPHA_VANTAGE: ApiKeySpec = ApiKeySpec {
    id: "alpha_vantage",
    provider: "Alpha Vantage",
    label: "Alpha Vantage API key",
    help: "Free key from alphavantage.co. Optional: a second source for stock quotes and price history when Yahoo Finance is down (the free plan allows about 25 calls a day).",
    secret: true,
    keyring_user: "alpha_vantage_api_key",
    env_var: "ALPHA_VANTAGE_API_KEY",
};

/// A SEC não pede chave, mas exige um e-mail de contato no `User-Agent`. Tem que
/// ser de quem opera o app, então não há valor embutido no código.
pub const SEC_EDGAR_EMAIL: ApiKeySpec = ApiKeySpec {
    id: "sec_edgar_email",
    provider: "SEC EDGAR",
    label: "SEC EDGAR contact email",
    help: "The SEC asks every client to identify itself with a contact email. Needed for US stock and REIT fundamentals.",
    secret: false,
    keyring_user: "sec_edgar_contact_email",
    env_var: "SEC_EDGAR_CONTACT_EMAIL",
};

/// Todas as credenciais configuráveis, na ordem em que a tela as mostra.
pub const ALL: [&ApiKeySpec; 3] = [&BOLSAI, &SEC_EDGAR_EMAIL, &ALPHA_VANTAGE];

pub fn find(id: &str) -> Option<&'static ApiKeySpec> {
    ALL.iter().copied().find(|spec| spec.id == id)
}

/// De onde a credencial está vindo agora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeySource {
    Keyring,
    Env,
    None,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Escolhe a credencial: o cofre tem prioridade sobre o ambiente. Separado de
/// `resolve` pra ser testável sem keyring.
pub fn pick(keyring: Option<String>, env: Option<String>) -> Option<String> {
    non_empty(keyring).or_else(|| non_empty(env))
}

/// Mesma regra de `pick`, dizendo qual fonte venceu.
pub fn source_of(keyring: Option<&str>, env: Option<&str>) -> KeySource {
    if non_empty(keyring.map(str::to_string)).is_some() {
        KeySource::Keyring
    } else if non_empty(env.map(str::to_string)).is_some() {
        KeySource::Env
    } else {
        KeySource::None
    }
}

fn entry(spec: &ApiKeySpec) -> Result<keyring::Entry, AppError> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, spec.keyring_user)?)
}

/// Sem backend de keyring (ex.: container sem secret service) cai pro ambiente.
fn from_keyring(spec: &ApiKeySpec) -> Option<String> {
    entry(spec).ok().and_then(|entry| entry.get_password().ok())
}

pub fn status(spec: &ApiKeySpec) -> KeySource {
    source_of(from_keyring(spec).as_deref(), std::env::var(spec.env_var).ok().as_deref())
}

/// Valor atual, só para credenciais que não são segredo (a tela mostra o e-mail).
pub fn visible_value(spec: &ApiKeySpec) -> Option<String> {
    if spec.secret {
        return None;
    }
    pick(from_keyring(spec), std::env::var(spec.env_var).ok())
}

pub fn resolve(spec: &ApiKeySpec) -> Result<String, AppError> {
    pick(from_keyring(spec), std::env::var(spec.env_var).ok()).ok_or_else(|| {
        AppError::ProviderUnavailable(format!(
            "{}: no {} configured (add it in Settings → Data sources, or set {})",
            spec.provider, spec.label, spec.env_var
        ))
    })
}

/// Valida e normaliza o que o usuário digitou. Segredo: não vazio e sem
/// espaço/quebra de linha (colar com `\n` é o erro típico). E-mail: um `@`,
/// partes não vazias, domínio com ponto e nenhum espaço.
pub fn validate_value(spec: &ApiKeySpec, raw: &str) -> Result<String, AppError> {
    let value = raw.trim();
    let invalid = |why: &str| AppError::InvalidInput(format!("{}: {why}", spec.label));
    if value.is_empty() {
        return Err(invalid("value is empty"));
    }
    if value.chars().any(char::is_whitespace) || value.chars().any(char::is_control) {
        return Err(invalid("must not contain spaces or line breaks"));
    }
    if !spec.secret {
        let mut parts = value.split('@');
        let (user, domain, rest) = (parts.next(), parts.next(), parts.next());
        let well_formed = matches!((user, domain, rest), (Some(u), Some(d), None)
            if !u.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.'));
        if !well_formed {
            return Err(invalid("not a valid email address"));
        }
    }
    Ok(value.to_string())
}

/// Grava no keyring do SO. Falha com `Keyring` se o sistema não tem cofre.
pub fn store(spec: &ApiKeySpec, raw: &str) -> Result<(), AppError> {
    let value = validate_value(spec, raw)?;
    entry(spec)?.set_password(&value)?;
    Ok(())
}

/// Remove do keyring (não mexe na variável de ambiente). Sem nada gravado não é erro.
pub fn clear(spec: &ApiKeySpec) -> Result<(), AppError> {
    match entry(spec)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_wins_over_env() {
        assert_eq!(pick(Some("a".into()), Some("b".into())), Some("a".into()));
        assert_eq!(source_of(Some("a"), Some("b")), KeySource::Keyring);
    }

    #[test]
    fn blank_values_fall_through() {
        assert_eq!(pick(Some("  ".into()), Some(" b ".into())), Some("b".into()));
        assert_eq!(pick(None, Some("".into())), None);
        assert_eq!(source_of(Some("  "), Some(" b ")), KeySource::Env);
        assert_eq!(source_of(None, Some("")), KeySource::None);
    }

    #[test]
    fn ids_are_unique_and_findable() {
        for spec in ALL {
            assert_eq!(find(spec.id).map(|s| s.id), Some(spec.id));
        }
        let ids: std::collections::HashSet<_> = ALL.iter().map(|s| s.id).collect();
        let users: std::collections::HashSet<_> = ALL.iter().map(|s| s.keyring_user).collect();
        let vars: std::collections::HashSet<_> = ALL.iter().map(|s| s.env_var).collect();
        assert_eq!(ids.len(), ALL.len());
        assert_eq!(users.len(), ALL.len());
        assert_eq!(vars.len(), ALL.len());
        assert!(find("nope").is_none());
    }

    #[test]
    fn the_sec_email_is_shown_back_but_the_api_key_is_not() {
        assert!(!SEC_EDGAR_EMAIL.secret);
        assert!(BOLSAI.secret);
        assert_eq!(visible_value(&BOLSAI), None);
    }

    #[test]
    fn secrets_are_trimmed_and_must_not_hold_spaces_or_line_breaks() {
        assert_eq!(validate_value(&BOLSAI, "  abc123  \n").unwrap(), "abc123");
        for bad in ["", "   ", "ab cd", "ab\ncd", "ab\tcd"] {
            assert!(matches!(validate_value(&BOLSAI, bad), Err(AppError::InvalidInput(_))), "{bad:?}");
        }
    }

    #[test]
    fn the_contact_email_needs_a_plausible_address() {
        assert_eq!(validate_value(&SEC_EDGAR_EMAIL, " me@example.com ").unwrap(), "me@example.com");
        for bad in ["me", "me@", "@example.com", "me@example", "me@@example.com", "a@b@c.com", "me@.com", "me@example.", "me @example.com"] {
            assert!(matches!(validate_value(&SEC_EDGAR_EMAIL, bad), Err(AppError::InvalidInput(_))), "{bad:?}");
        }
    }
}
