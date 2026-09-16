// Fase 16 — Backup do Workspace. Camada Tauri fina em cima do engine genérico (`crate::backup`):
// resolve o `StorageProvider` ativo (mesmo `resolve_active_provider` da Fase 15) pros anexos,
// lê/escreve o arquivo `.json` escolhido pelo usuário via file picker do frontend.
use sea_orm::DatabaseConnection;

use crate::backup::{self, BackupSnapshot, BackupSummary};
use crate::error::AppError;
use crate::storage;

#[tauri::command]
pub async fn export_workspace_backup(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    dest_path: String,
) -> Result<BackupSummary, AppError> {
    let provider = storage::resolve_active_provider(&app, db.inner()).await?;
    let snapshot = backup::export_snapshot(db.inner(), provider.as_ref()).await?;
    let summary = backup::summarize(&snapshot);
    let json = serde_json::to_vec_pretty(&snapshot)?;
    std::fs::write(&dest_path, json)?;
    Ok(summary)
}

#[tauri::command]
pub async fn import_workspace_backup(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    source_path: String,
) -> Result<BackupSummary, AppError> {
    let provider = storage::resolve_active_provider(&app, db.inner()).await?;
    let bytes = std::fs::read(&source_path)?;
    let snapshot: BackupSnapshot = serde_json::from_slice(&bytes)?;
    backup::import_snapshot(db.inner(), provider.as_ref(), snapshot).await
}
