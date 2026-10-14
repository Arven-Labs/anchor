// Fase 14.3 — só armazenamento + leitura da escolha Local/Remote da Finance API (schema+UI,
// decisão explícita de escopo). Não consultado por `finance_api::sidecar::init` ainda — isso
// fica pra uma fatia futura, decisão registrada no PHASE.md.
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::Serialize;

use crate::entity::finance_api_settings;
use crate::error::AppError;

// Serviço do keyring do app (morava em `commands::api_key` até a Fase 20 remover as chaves de IA;
// este é o único consumidor que sobrou).
const KEYRING_SERVICE: &str = "anchor";

// Username fixo — só uma chave remota possível por vez.
const REMOTE_KEY_USERNAME: &str = "finance_api_remote";

#[derive(Serialize)]
pub struct FinanceApiSettingsView {
    pub mode: String,
    pub remote_url: Option<String>,
    pub has_remote_key: bool,
}

fn remote_key_entry() -> Result<keyring::Entry, AppError> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, REMOTE_KEY_USERNAME)?)
}

fn has_remote_key() -> Result<bool, AppError> {
    match remote_key_entry()?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(err) => Err(err.into()),
    }
}

#[tauri::command]
pub async fn get_finance_api_settings(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<FinanceApiSettingsView, AppError> {
    let row = finance_api_settings::Entity::find().one(db.inner()).await?;

    Ok(FinanceApiSettingsView {
        mode: row.as_ref().map(|r| r.mode.clone()).unwrap_or_else(|| "local".to_string()),
        remote_url: row.and_then(|r| r.remote_url),
        has_remote_key: has_remote_key()?,
    })
}

#[tauri::command]
pub async fn set_finance_api_settings(
    db: tauri::State<'_, DatabaseConnection>,
    mode: String,
    remote_url: Option<String>,
    remote_key: Option<String>,
) -> Result<(), AppError> {
    if mode != "local" && mode != "remote" {
        return Err(AppError::InvalidInput(format!(
            "unknown Finance API mode '{mode}' (expected 'local' or 'remote')"
        )));
    }

    let remote_url = remote_url.filter(|url| !url.trim().is_empty());
    if mode == "remote" && remote_url.is_none() {
        return Err(AppError::InvalidInput(
            "remote_url is required when mode is 'remote'".to_string(),
        ));
    }

    let db = db.inner();
    finance_api_settings::Entity::delete_many().exec(db).await?;
    finance_api_settings::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        mode: Set(mode),
        remote_url: Set(remote_url),
        updated_at: Set(chrono::Utc::now().to_rfc3339()),
    }
    .insert(db)
    .await?;

    // Em branco mantém a chave já salva — evita forçar redigitar o segredo toda vez que só a
    // URL muda.
    if let Some(key) = remote_key.filter(|k| !k.trim().is_empty()) {
        remote_key_entry()?.set_password(&key)?;
    }

    Ok(())
}
