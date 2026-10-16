// Chaves de providers que exigem conta (BYO-key). Ordem de busca: cofre do SO
// (keyring, serviço `anchor`) e depois variável de ambiente. Nunca em arquivo
// de configuração nem no banco. Sem chave o provider responde
// `ProviderUnavailable` com uma mensagem que diz o que configurar.

use crate::error::AppError;

const KEYRING_SERVICE: &str = "anchor";

pub struct ApiKeySpec {
    /// Nome mostrado nas mensagens de erro.
    pub provider: &'static str,
    pub keyring_user: &'static str,
    pub env_var: &'static str,
}

pub const BOLSAI: ApiKeySpec =
    ApiKeySpec { provider: "bolsai", keyring_user: "bolsai_api_key", env_var: "BOLSAI_API_KEY" };

fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Escolhe a chave: o cofre tem prioridade sobre o ambiente. Separado de
/// `resolve` pra ser testável sem keyring.
pub fn pick(keyring: Option<String>, env: Option<String>) -> Option<String> {
    non_empty(keyring).or_else(|| non_empty(env))
}

pub fn resolve(spec: &ApiKeySpec) -> Result<String, AppError> {
    // Sem backend de keyring (ex.: container sem secret service) cai pro ambiente.
    let from_keyring = keyring::Entry::new(KEYRING_SERVICE, spec.keyring_user)
        .ok()
        .and_then(|entry| entry.get_password().ok());
    pick(from_keyring, std::env::var(spec.env_var).ok()).ok_or_else(|| {
        AppError::ProviderUnavailable(format!(
            "{}: no API key configured (set {} or save it in the OS keyring as '{}')",
            spec.provider, spec.env_var, spec.keyring_user
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_wins_over_env() {
        assert_eq!(pick(Some("a".into()), Some("b".into())), Some("a".into()));
    }

    #[test]
    fn blank_values_fall_through() {
        assert_eq!(pick(Some("  ".into()), Some(" b ".into())), Some("b".into()));
        assert_eq!(pick(None, Some("".into())), None);
    }
}
