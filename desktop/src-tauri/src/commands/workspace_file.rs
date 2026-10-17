// Fase 34 — camada Tauri fina do arquivo portátil de um Workspace
// (`crate::workspace_file`): o front escolhe o caminho pelo diálogo nativo.
use sea_orm::DatabaseConnection;

use crate::error::AppError;
use crate::workspace_file::{self, WorkspaceFile, WorkspaceImportSummary};

#[derive(serde::Serialize)]
pub struct WorkspaceExportSummary {
    pub workspace_name: String,
    pub table_count: usize,
    pub row_count: usize,
}

#[tauri::command]
pub async fn export_workspace_file(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
    dest_path: String,
) -> Result<WorkspaceExportSummary, AppError> {
    let file = workspace_file::export_workspace(db.inner(), workspace_id).await?;
    let summary = WorkspaceExportSummary {
        workspace_name: file.workspace.get("name").and_then(|n| n.as_str()).unwrap_or_default().to_string(),
        table_count: file.tables.len(),
        row_count: file.tables.values().map(Vec::len).sum::<usize>() + file.assets.len(),
    };
    std::fs::write(&dest_path, serde_json::to_vec_pretty(&file)?)?;
    Ok(summary)
}

#[tauri::command]
pub async fn import_workspace_file(
    db: tauri::State<'_, DatabaseConnection>,
    source_path: String,
) -> Result<WorkspaceImportSummary, AppError> {
    let bytes = std::fs::read(&source_path)?;
    let file: WorkspaceFile = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::InvalidInput(format!("arquivo de Workspace ilegível: {e}")))?;
    workspace_file::import_workspace(db.inner(), file).await
}
