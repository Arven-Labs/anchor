use std::path::Path;

use rand::rngs::OsRng;
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::Deserialize;

use crate::entity::{asset_attachments, asset_valuations};
use crate::error::AppError;

// Fase 10, item 8 — classe `imovel` (cadastro manual, sem fonte de dados
// externa). Histórico de avaliações + anexos (escritura, ITBI, IPTU) do
// ativo, mesmo molde de `commands/thesis.rs` (Fase 10.5): anexo vive atrás
// do `StorageProvider` ativo (Fase 15 — só metadados no banco, o disco local
// segue sendo o único provider implementado, ver `storage::resolve_active_provider`).
// `origin` de `asset_valuations` só grava `"manual"` por ora — o mecanismo de
// reajuste automático por % (rascunho da Sessão 30) ainda não foi decidido,
// ver PHASE.md item 8.

fn sanitize_file_name(name: &str) -> String {
    name.replace(['/', '\\'], "_")
}

fn guess_content_type(file_name: &str) -> Option<&'static str> {
    let extension = Path::new(file_name)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    Some(match extension.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "xls" => "application/vnd.ms-excel",
        "csv" => "text/csv",
        _ => return None,
    })
}

// --- Avaliações ---

#[tauri::command]
pub async fn list_asset_valuations(
    db: tauri::State<'_, DatabaseConnection>,
    asset_id: i32,
) -> Result<Vec<asset_valuations::Model>, AppError> {
    Ok(asset_valuations::Entity::find()
        .filter(asset_valuations::Column::AssetId.eq(asset_id))
        .order_by_asc(asset_valuations::Column::ValuationDate)
        .all(db.inner())
        .await?)
}

#[derive(Deserialize)]
pub struct AddAssetValuationRequest {
    pub asset_id: i32,
    pub valuation_date: String,
    pub value: f64,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn add_asset_valuation(
    db: tauri::State<'_, DatabaseConnection>,
    request: AddAssetValuationRequest,
) -> Result<asset_valuations::Model, AppError> {
    Ok(asset_valuations::ActiveModel {
        asset_id: Set(request.asset_id),
        valuation_date: Set(request.valuation_date),
        value: Set(request.value),
        origin: Set("manual".to_string()),
        notes: Set(request.notes),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?)
}

#[tauri::command]
pub async fn delete_asset_valuation(
    db: tauri::State<'_, DatabaseConnection>,
    valuation_id: i32,
) -> Result<(), AppError> {
    asset_valuations::Entity::delete_by_id(valuation_id)
        .exec(db.inner())
        .await?;
    Ok(())
}

// --- Anexos ---

#[tauri::command]
pub async fn list_asset_attachments(
    db: tauri::State<'_, DatabaseConnection>,
    asset_id: i32,
) -> Result<Vec<asset_attachments::Model>, AppError> {
    Ok(asset_attachments::Entity::find()
        .filter(asset_attachments::Column::AssetId.eq(asset_id))
        .all(db.inner())
        .await?)
}

#[derive(Deserialize)]
pub struct AddAssetAttachmentRequest {
    pub asset_id: i32,
    pub source_path: String,
    pub document_type: Option<String>,
}

#[tauri::command]
pub async fn add_asset_attachment(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    request: AddAssetAttachmentRequest,
) -> Result<asset_attachments::Model, AppError> {
    let source = Path::new(&request.source_path);
    let original_file_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            AppError::InvalidInput(format!("invalid file path '{}'", request.source_path))
        })?
        .to_string();

    let provider = crate::storage::resolve_active_provider(&app, db.inner()).await?;
    let content = std::fs::read(&request.source_path)?;
    let file_size_bytes = content.len() as i64;

    let stored_name = format!(
        "{}_{:08x}_{}",
        chrono::Utc::now().timestamp_millis(),
        OsRng.next_u32(),
        sanitize_file_name(&original_file_name)
    );
    let stored_relative_path = format!("asset_attachments/{}/{}", request.asset_id, stored_name);
    provider.write(&stored_relative_path, &content)?;

    Ok(asset_attachments::ActiveModel {
        asset_id: Set(request.asset_id),
        original_file_name: Set(original_file_name.clone()),
        stored_relative_path: Set(stored_relative_path),
        file_size_bytes: Set(file_size_bytes),
        content_type: Set(guess_content_type(&original_file_name).map(str::to_string)),
        document_type: Set(request.document_type),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?)
}

#[tauri::command]
pub async fn delete_asset_attachment(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    attachment_id: i32,
) -> Result<(), AppError> {
    let existing = asset_attachments::Entity::find_by_id(attachment_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset attachment {attachment_id}")))?;

    let provider = crate::storage::resolve_active_provider(&app, db.inner()).await?;
    provider.delete(&existing.stored_relative_path)?;

    asset_attachments::Entity::delete_by_id(attachment_id)
        .exec(db.inner())
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn get_asset_attachment_path(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    attachment_id: i32,
) -> Result<String, AppError> {
    let existing = asset_attachments::Entity::find_by_id(attachment_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset attachment {attachment_id}")))?;

    let provider = crate::storage::resolve_active_provider(&app, db.inner()).await?;
    let path = provider
        .local_path(&existing.stored_relative_path)
        .ok_or_else(|| {
            AppError::InvalidInput(
                "active storage provider does not expose a local file path".to_string(),
            )
        })?;

    Ok(path.to_string_lossy().into_owned())
}

// Prévia de anexo de planilha (CSV, xls, xlsx), lida aqui e não no webview. O frontend manda só o
// id do anexo: o caminho sai do banco, então o comando nunca abre um arquivo que não seja anexo.
#[tauri::command]
pub async fn read_asset_attachment_spreadsheet(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    attachment_id: i32,
) -> Result<Vec<Vec<String>>, AppError> {
    let existing = asset_attachments::Entity::find_by_id(attachment_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset attachment {attachment_id}")))?;
    let path = get_asset_attachment_path(app, db, attachment_id).await?;
    crate::commands::portfolio_io::read_spreadsheet_preview(&path, existing.content_type.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_file_name_strips_path_separators() {
        assert_eq!(sanitize_file_name("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(sanitize_file_name(r"a\b\c.pdf"), "a_b_c.pdf");
    }

    #[test]
    fn guess_content_type_matches_known_extensions() {
        assert_eq!(guess_content_type("escritura.PDF"), Some("application/pdf"));
        assert_eq!(guess_content_type("foto.jpeg"), Some("image/jpeg"));
    }

    #[test]
    fn guess_content_type_returns_none_for_unknown_or_missing_extension() {
        assert_eq!(guess_content_type("no_extension"), None);
        assert_eq!(guess_content_type("arquivo.docx"), None);
    }
}
