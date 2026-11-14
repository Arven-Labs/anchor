use std::path::Path;

use rand::rngs::OsRng;
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, Unchanged,
};
use serde::Deserialize;

use crate::entity::{theses, thesis_attachments};
use crate::error::AppError;

// Fase 10.5 — tese de investimento (vinculável a um ativo ou global/macro)
// com anexos. Anexo vive atrás do `StorageProvider` ativo (Fase 15 — só o
// disco local está implementado, ver `storage::resolve_active_provider`), não
// em bucket — decisão explícita da sessão, ver PHASE.md item 10.5. Só
// metadados (nome original, caminho relativo, tamanho, content-type) ficam
// no banco.

// Remove separadores de path do nome original antes de gravar em disco —
// evita que um nome de arquivo malicioso ("../../etc/passwd") escape do
// diretório de anexos.
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

// --- Teses ---

#[tauri::command]
pub async fn list_theses(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<theses::Model>, AppError> {
    Ok(theses::Entity::find()
        .filter(theses::Column::WorkspaceId.eq(workspace_id))
        .all(db.inner())
        .await?)
}

#[derive(Deserialize)]
pub struct CreateThesisRequest {
    pub workspace_id: i32,
    pub asset_id: Option<i32>,
    pub title: String,
    pub content_markdown: String,
}

#[tauri::command]
pub async fn create_thesis(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateThesisRequest,
) -> Result<theses::Model, AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let thesis = theses::ActiveModel {
        workspace_id: Set(request.workspace_id),
        asset_id: Set(request.asset_id),
        title: Set(request.title),
        content_markdown: Set(request.content_markdown),
        created_at: Set(now.clone()),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db.inner())
    .await?;

    Ok(thesis)
}

#[derive(Deserialize)]
pub struct UpdateThesisRequest {
    pub thesis_id: i32,
    pub asset_id: Option<i32>,
    pub title: String,
    pub content_markdown: String,
}

#[tauri::command]
pub async fn update_thesis(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateThesisRequest,
) -> Result<theses::Model, AppError> {
    let existing = theses::Entity::find_by_id(request.thesis_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("thesis {}", request.thesis_id)))?;

    let updated = theses::ActiveModel {
        id: Unchanged(existing.id),
        asset_id: Set(request.asset_id),
        title: Set(request.title),
        content_markdown: Set(request.content_markdown),
        updated_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .update(db.inner())
    .await?;

    Ok(updated)
}

#[tauri::command]
pub async fn delete_thesis(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    thesis_id: i32,
) -> Result<(), AppError> {
    let provider = crate::storage::resolve_active_provider(&app, db.inner()).await?;
    provider.delete_prefix(&format!("thesis_attachments/{thesis_id}"))?;

    theses::Entity::delete_by_id(thesis_id)
        .exec(db.inner())
        .await?;

    Ok(())
}

// --- Anexos ---

#[tauri::command]
pub async fn list_thesis_attachments(
    db: tauri::State<'_, DatabaseConnection>,
    thesis_id: i32,
) -> Result<Vec<thesis_attachments::Model>, AppError> {
    Ok(thesis_attachments::Entity::find()
        .filter(thesis_attachments::Column::ThesisId.eq(thesis_id))
        .all(db.inner())
        .await?)
}

#[derive(Deserialize)]
pub struct AddThesisAttachmentRequest {
    pub thesis_id: i32,
    pub source_path: String,
}

#[tauri::command]
pub async fn add_thesis_attachment(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    request: AddThesisAttachmentRequest,
) -> Result<thesis_attachments::Model, AppError> {
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
    let stored_relative_path = format!(
        "thesis_attachments/{}/{}",
        request.thesis_id, stored_name
    );
    provider.write(&stored_relative_path, &content)?;

    let attachment = thesis_attachments::ActiveModel {
        thesis_id: Set(request.thesis_id),
        original_file_name: Set(original_file_name.clone()),
        stored_relative_path: Set(stored_relative_path),
        file_size_bytes: Set(file_size_bytes),
        content_type: Set(guess_content_type(&original_file_name).map(str::to_string)),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?;

    Ok(attachment)
}

#[tauri::command]
pub async fn delete_thesis_attachment(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    attachment_id: i32,
) -> Result<(), AppError> {
    let existing = thesis_attachments::Entity::find_by_id(attachment_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("thesis attachment {attachment_id}")))?;

    let provider = crate::storage::resolve_active_provider(&app, db.inner()).await?;
    provider.delete(&existing.stored_relative_path)?;

    thesis_attachments::Entity::delete_by_id(attachment_id)
        .exec(db.inner())
        .await?;

    Ok(())
}

// Devolve o caminho absoluto do arquivo no disco — o frontend usa
// `convertFileSrc` em cima disso pra montar a URL de preview (imagem/PDF
// direto num `<img>`/`<iframe>`; planilha vai por `read_thesis_attachment_spreadsheet`,
// lida no backend).
#[tauri::command]
pub async fn get_thesis_attachment_path(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    attachment_id: i32,
) -> Result<String, AppError> {
    let existing = thesis_attachments::Entity::find_by_id(attachment_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("thesis attachment {attachment_id}")))?;

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
pub async fn read_thesis_attachment_spreadsheet(
    app: tauri::AppHandle,
    db: tauri::State<'_, DatabaseConnection>,
    attachment_id: i32,
) -> Result<Vec<Vec<String>>, AppError> {
    let existing = thesis_attachments::Entity::find_by_id(attachment_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("thesis attachment {attachment_id}")))?;
    let path = get_thesis_attachment_path(app, db, attachment_id).await?;
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
    fn sanitize_file_name_leaves_plain_names_untouched() {
        assert_eq!(sanitize_file_name("relatorio.pdf"), "relatorio.pdf");
    }

    #[test]
    fn guess_content_type_matches_known_extensions() {
        assert_eq!(guess_content_type("relatorio.PDF"), Some("application/pdf"));
        assert_eq!(guess_content_type("foto.jpeg"), Some("image/jpeg"));
        assert_eq!(guess_content_type("planilha.xlsx"), Some(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        ));
        assert_eq!(guess_content_type("dados.csv"), Some("text/csv"));
    }

    #[test]
    fn guess_content_type_returns_none_for_unknown_or_missing_extension() {
        assert_eq!(guess_content_type("no_extension"), None);
        assert_eq!(guess_content_type("arquivo.docx"), None);
    }
}
