//! Persistência do módulo (tabelas `wallet_viewer_*`, sem chave estrangeira para fora).

use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Serialize;

use crate::domain::bitcoin_xpub::XpubKind;
use crate::domain::wallet_address::parse_address;
use crate::entity::{wallet_viewer_address, wallet_viewer_settings};
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WalletAddress {
    pub id: i32,
    /// Endereço canônico (checksum EIP-55).
    pub address: String,
    pub label: Option<String>,
    pub created_at: String,
}

impl From<wallet_viewer_address::Model> for WalletAddress {
    fn from(model: wallet_viewer_address::Model) -> Self {
        Self { id: model.id, address: model.address, label: model.label, created_at: model.created_at }
    }
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// O módulo vem **desligado**: sem linha na tabela de configuração, o valor é `false`.
pub async fn is_enabled(db: &DatabaseConnection) -> Result<bool, AppError> {
    let row = wallet_viewer_settings::Entity::find().one(db).await?;
    Ok(row.is_some_and(|row| row.enabled))
}

pub async fn set_enabled(db: &DatabaseConnection, enabled: bool) -> Result<(), AppError> {
    wallet_viewer_settings::Entity::delete_many().exec(db).await?;
    wallet_viewer_settings::ActiveModel {
        enabled: Set(enabled),
        updated_at: Set(now()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// Porta de entrada dos comandos que escrevem ou usam a rede: com o módulo desligado, recusam.
pub async fn require_enabled(db: &DatabaseConnection) -> Result<(), AppError> {
    if is_enabled(db).await? {
        Ok(())
    } else {
        Err(AppError::InvalidInput("the wallet viewer is disabled; enable it in Settings".to_string()))
    }
}

pub async fn add_address(
    db: &DatabaseConnection,
    address: &str,
    label: Option<&str>,
) -> Result<WalletAddress, AppError> {
    require_enabled(db).await?;
    let address = parse_address(address)?;
    let duplicate = wallet_viewer_address::Entity::find()
        .filter(wallet_viewer_address::Column::Address.eq(address.as_str()))
        .one(db)
        .await?;
    if duplicate.is_some() {
        return Err(AppError::InvalidInput("this address was already added".to_string()));
    }
    let label = label.map(str::trim).filter(|label| !label.is_empty()).map(str::to_string);
    let saved = wallet_viewer_address::ActiveModel {
        address: Set(address),
        label: Set(label),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(saved.into())
}

/// Leitura local: continua permitida com o módulo desligado (desligar não apaga os dados).
pub async fn list_addresses(db: &DatabaseConnection) -> Result<Vec<WalletAddress>, AppError> {
    let rows = wallet_viewer_address::Entity::find().order_by_asc(wallet_viewer_address::Column::Id).all(db).await?;
    Ok(rows.into_iter().map(WalletAddress::from).collect())
}

pub async fn remove_address(db: &DatabaseConnection, id: i32) -> Result<(), AppError> {
    let result = wallet_viewer_address::Entity::delete_by_id(id).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound(format!("wallet address {id}")));
    }
    Ok(())
}

/// Chave pública estendida de Bitcoin cadastrada (Fase 36.4a). Só a chave pública: nunca privada.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WalletXpub {
    pub id: i32,
    /// A xpub/zpub como validada (sem espaços nas pontas).
    pub xpub: String,
    pub kind: XpubKind,
    pub label: Option<String>,
    pub created_at: String,
}

/// Valida a chave (`parse_xpub`), recusa duplicada e grava. Com o módulo desligado recusa antes.
pub async fn add_xpub(_db: &DatabaseConnection, _xpub: &str, _label: Option<&str>) -> Result<WalletXpub, AppError> {
    todo!("Fase 36.4a")
}

/// Leitura local: continua permitida com o módulo desligado.
pub async fn list_xpubs(_db: &DatabaseConnection) -> Result<Vec<WalletXpub>, AppError> {
    todo!("Fase 36.4a")
}

pub async fn remove_xpub(_db: &DatabaseConnection, _id: i32) -> Result<(), AppError> {
    todo!("Fase 36.4a")
}
