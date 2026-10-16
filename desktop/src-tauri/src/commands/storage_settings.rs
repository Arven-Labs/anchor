// Fase 15 — só a escolha de `StorageProvider` (schema+UI), escopo explícito: `set_storage_settings` recusa qualquer valor
// diferente de `"local"`: as outras 3 opções existem no enum (`StorageProviderKind`) e na UI
// ("em breve") só pra não exigir migração de schema quando ganharem implementação real.
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::Serialize;

use crate::entity::storage_settings;
use crate::error::AppError;
use crate::storage::StorageProviderKind;

#[derive(Serialize)]
pub struct StorageSettingsView {
    pub provider: String,
}

#[tauri::command]
pub async fn get_storage_settings(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<StorageSettingsView, AppError> {
    let row = storage_settings::Entity::find().one(db.inner()).await?;

    Ok(StorageSettingsView {
        provider: row.map(|r| r.provider).unwrap_or_else(|| "local".to_string()),
    })
}

#[tauri::command]
pub async fn set_storage_settings(
    db: tauri::State<'_, DatabaseConnection>,
    provider: String,
) -> Result<(), AppError> {
    let kind = StorageProviderKind::parse(&provider)?;
    if kind != StorageProviderKind::Local {
        return Err(AppError::InvalidInput(format!(
            "storage provider '{provider}' is not implemented yet"
        )));
    }

    let db = db.inner();
    storage_settings::Entity::delete_many().exec(db).await?;
    storage_settings::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        provider: Set(provider),
        updated_at: Set(chrono::Utc::now().to_rfc3339()),
    }
    .insert(db)
    .await?;

    Ok(())
}
