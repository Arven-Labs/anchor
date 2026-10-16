// Fase 15 — StorageProvider seletivo (spec trazida na Sessão 95, ver PHASE.md). O core do app só
// fala com esta trait, nunca direto com disco/rede/Web3 — a implementação ativa é resolvida em
// runtime a partir de `storage_settings`. Só `LocalFSProvider` existe de verdade nesta fatia;
// `self_hosted`/`managed_cloud`/`decentralized_vault` ficam reservados no enum (e na UI, como
// "em breve") até terem uma implementação real (a última depende da Fase 8/Vault Web3, ainda não
// implementada). Todas as operações são síncronas (mesmo estilo de `std::fs::*` que
// `commands/property.rs`/`commands/thesis.rs` já usavam antes desta fatia) — não há motivo pra
// puxar `async-trait` enquanto o único provider real é o disco local.
mod local_fs;

use std::path::PathBuf;

use sea_orm::{DatabaseConnection, EntityTrait};
use tauri::Manager;

use crate::entity::storage_settings;
use crate::error::AppError;

pub use local_fs::LocalFSProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageProviderKind {
    Local,
    SelfHosted,
    ManagedCloud,
    DecentralizedVault,
}

impl StorageProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageProviderKind::Local => "local",
            StorageProviderKind::SelfHosted => "self_hosted",
            StorageProviderKind::ManagedCloud => "managed_cloud",
            StorageProviderKind::DecentralizedVault => "decentralized_vault",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, AppError> {
        match raw {
            "local" => Ok(StorageProviderKind::Local),
            "self_hosted" => Ok(StorageProviderKind::SelfHosted),
            "managed_cloud" => Ok(StorageProviderKind::ManagedCloud),
            "decentralized_vault" => Ok(StorageProviderKind::DecentralizedVault),
            other => Err(AppError::InvalidInput(format!(
                "unknown storage provider '{other}'"
            ))),
        }
    }
}

/// Onde anexos/documentos financeiros são lidos/escritos. Caminhos são relativos (mesmo formato
/// que `stored_relative_path` já usava, ex. `"asset_attachments/5/xxx.pdf"`) — cada provider
/// decide o que isso significa de verdade (pasta local, upload remoto, blob cifrado no Vault).
pub trait StorageProvider: Send + Sync {
    fn write(&self, relative_path: &str, content: &[u8]) -> Result<(), AppError>;
    fn read(&self, relative_path: &str) -> Result<Vec<u8>, AppError>;
    fn list(&self, prefix: &str) -> Result<Vec<String>, AppError>;
    fn delete(&self, relative_path: &str) -> Result<(), AppError>;
    fn delete_prefix(&self, prefix: &str) -> Result<(), AppError>;

    /// Caminho absoluto no disco local, quando o provider expõe um (só `LocalFSProvider` por
    /// ora) — usado pelo frontend via `convertFileSrc` pra montar preview de anexo.
    fn local_path(&self, _relative_path: &str) -> Option<PathBuf> {
        None
    }

    fn export_all(&self) -> Result<Vec<(String, Vec<u8>)>, AppError>;
    fn import_all(&self, files: Vec<(String, Vec<u8>)>) -> Result<(), AppError>;
}

/// Resolve o provider configurado em `storage_settings` (default `local` se a tabela estiver
/// vazia: linha única, ausente = valor padrão).
pub async fn resolve_active_provider(
    app: &tauri::AppHandle,
    db: &DatabaseConnection,
) -> Result<Box<dyn StorageProvider>, AppError> {
    let row = storage_settings::Entity::find().one(db).await?;
    let kind = match row {
        Some(r) => StorageProviderKind::parse(&r.provider)?,
        None => StorageProviderKind::Local,
    };

    match kind {
        StorageProviderKind::Local => {
            let base_dir = app.path().app_data_dir().map_err(|e| {
                AppError::InvalidInput(format!("could not resolve app data dir: {e}"))
            })?;
            Ok(Box::new(LocalFSProvider::new(base_dir)))
        }
        other => Err(AppError::InvalidInput(format!(
            "storage provider '{}' is not implemented yet",
            other.as_str()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_all_known_providers() {
        assert_eq!(StorageProviderKind::parse("local").unwrap(), StorageProviderKind::Local);
        assert_eq!(
            StorageProviderKind::parse("self_hosted").unwrap(),
            StorageProviderKind::SelfHosted
        );
        assert_eq!(
            StorageProviderKind::parse("managed_cloud").unwrap(),
            StorageProviderKind::ManagedCloud
        );
        assert_eq!(
            StorageProviderKind::parse("decentralized_vault").unwrap(),
            StorageProviderKind::DecentralizedVault
        );
    }

    #[test]
    fn parse_rejects_unknown_provider() {
        let err = StorageProviderKind::parse("dropbox").unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(msg) if msg.contains("dropbox")));
    }

    #[test]
    fn as_str_round_trips_through_parse() {
        for kind in [
            StorageProviderKind::Local,
            StorageProviderKind::SelfHosted,
            StorageProviderKind::ManagedCloud,
            StorageProviderKind::DecentralizedVault,
        ] {
            assert_eq!(StorageProviderKind::parse(kind.as_str()).unwrap(), kind);
        }
    }
}
