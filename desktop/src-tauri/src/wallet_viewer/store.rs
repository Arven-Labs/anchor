//! Persistência do módulo (tabelas `wallet_viewer_*`, sem chave estrangeira para fora).

use sea_orm::DatabaseConnection;
use serde::Serialize;

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WalletAddress {
    pub id: i32,
    /// Endereço canônico (checksum EIP-55).
    pub address: String,
    pub label: Option<String>,
    pub created_at: String,
}

/// O módulo vem **desligado**: sem linha na tabela de configuração, o valor é `false`.
pub async fn is_enabled(_db: &DatabaseConnection) -> Result<bool, AppError> {
    todo!("Etapa 2 e 4 do plano do visualizador de wallets")
}

pub async fn set_enabled(_db: &DatabaseConnection, _enabled: bool) -> Result<(), AppError> {
    todo!("Etapa 2 e 4 do plano do visualizador de wallets")
}

/// Porta de entrada dos comandos que escrevem ou usam a rede: com o módulo desligado, recusam.
pub async fn require_enabled(_db: &DatabaseConnection) -> Result<(), AppError> {
    todo!("Etapa 2 e 4 do plano do visualizador de wallets")
}

pub async fn add_address(
    _db: &DatabaseConnection,
    _address: &str,
    _label: Option<&str>,
) -> Result<WalletAddress, AppError> {
    todo!("Etapa 2 e 4 do plano do visualizador de wallets")
}

/// Leitura local: continua permitida com o módulo desligado (desligar não apaga os dados).
pub async fn list_addresses(_db: &DatabaseConnection) -> Result<Vec<WalletAddress>, AppError> {
    todo!("Etapa 2 e 4 do plano do visualizador de wallets")
}

pub async fn remove_address(_db: &DatabaseConnection, _id: i32) -> Result<(), AppError> {
    todo!("Etapa 2 e 4 do plano do visualizador de wallets")
}
