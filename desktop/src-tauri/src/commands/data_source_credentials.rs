// Fase 26 — credenciais das fontes de dados (Settings → Data sources). Substitui a
// escolha Local/Remote da Finance API do Lume, que sempre foi gravada e nunca
// lida (a tabela `finance_api_settings` foi apagada). Os valores ficam no keyring do SO
// (`finance_core::api_keys`); a tela nunca recebe um segredo de volta, só de
// onde ele está vindo. O e-mail de contato da SEC não é segredo e volta para a
// tela poder mostrá-lo.

use serde::Serialize;

use crate::error::AppError;
use crate::finance_core::api_keys::{self, ApiKeySpec, KeySource};

#[derive(Serialize)]
pub struct CredentialView {
    pub id: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub secret: bool,
    pub env_var: &'static str,
    pub source: KeySource,
    /// Só para credenciais que não são segredo.
    pub value: Option<String>,
}

fn view(spec: &'static ApiKeySpec) -> CredentialView {
    CredentialView {
        id: spec.id,
        label: spec.label,
        help: spec.help,
        secret: spec.secret,
        env_var: spec.env_var,
        source: api_keys::status(spec),
        value: api_keys::visible_value(spec),
    }
}

fn spec_for(id: &str) -> Result<&'static ApiKeySpec, AppError> {
    api_keys::find(id).ok_or_else(|| AppError::InvalidInput(format!("unknown data source credential '{id}'")))
}

#[tauri::command]
pub async fn list_data_source_credentials() -> Result<Vec<CredentialView>, AppError> {
    Ok(api_keys::ALL.iter().copied().map(view).collect())
}

#[tauri::command]
pub async fn set_data_source_credential(id: String, value: String) -> Result<CredentialView, AppError> {
    let spec = spec_for(&id)?;
    api_keys::store(spec, &value)?;
    Ok(view(spec))
}

#[tauri::command]
pub async fn clear_data_source_credential(id: String) -> Result<CredentialView, AppError> {
    let spec = spec_for(&id)?;
    api_keys::clear(spec)?;
    Ok(view(spec))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_id_is_invalid_input() {
        assert!(matches!(spec_for("nope"), Err(AppError::InvalidInput(_))));
        assert_eq!(spec_for("bolsai").unwrap().id, "bolsai");
    }

    #[tokio::test]
    async fn the_list_has_one_view_per_credential_and_never_leaks_a_secret() {
        let views = list_data_source_credentials().await.unwrap();
        assert_eq!(views.len(), api_keys::ALL.len());
        let bolsai = views.iter().find(|v| v.id == "bolsai").unwrap();
        assert!(bolsai.secret && bolsai.value.is_none());
        let sec = views.iter().find(|v| v.id == "sec_edgar_email").unwrap();
        assert!(!sec.secret);
    }

    #[tokio::test]
    async fn invalid_values_are_rejected_before_touching_the_keyring() {
        let err = set_data_source_credential("sec_edgar_email".into(), "not-an-email".into()).await;
        assert!(matches!(err, Err(AppError::InvalidInput(_))));
        let err = set_data_source_credential("bolsai".into(), "has space".into()).await;
        assert!(matches!(err, Err(AppError::InvalidInput(_))));
        let err = set_data_source_credential("nope".into(), "x".into()).await;
        assert!(matches!(err, Err(AppError::InvalidInput(_))));
    }
}
