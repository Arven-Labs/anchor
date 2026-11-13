// Visualizador de wallets (ADR 0002): comandos finos sobre `wallet_viewer::store` e `wallet_viewer::rpc`.
// Só leitura: nenhum comando aqui assina nada nem lida com chave privada.
use alloy_primitives::utils::format_ether;
use sea_orm::DatabaseConnection;
use serde::Serialize;

use crate::error::AppError;
use crate::wallet_viewer::rpc::{self, Network};
use crate::wallet_viewer::store::{self, WalletAddress};

#[derive(Serialize)]
pub struct WalletViewerSettingsView {
    pub enabled: bool,
}

/// Saldo de um endereço numa rede. Falha de uma rede não derruba as outras: vem em `error`.
#[derive(Serialize)]
pub struct NetworkBalance {
    pub network: &'static str,
    /// Saldo em wei, como texto decimal (um `U256` não cabe num número de JavaScript).
    pub wei: Option<String>,
    /// Saldo em ETH, texto decimal exato com 18 casas.
    pub eth: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct AddressBalances {
    #[serde(flatten)]
    pub wallet: WalletAddress,
    pub balances: Vec<NetworkBalance>,
}

#[tauri::command]
pub async fn wallet_viewer_get_settings(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<WalletViewerSettingsView, AppError> {
    Ok(WalletViewerSettingsView { enabled: store::is_enabled(db.inner()).await? })
}

#[tauri::command]
pub async fn wallet_viewer_set_enabled(
    db: tauri::State<'_, DatabaseConnection>,
    enabled: bool,
) -> Result<(), AppError> {
    store::set_enabled(db.inner(), enabled).await
}

#[tauri::command]
pub async fn wallet_viewer_list_addresses(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<WalletAddress>, AppError> {
    store::list_addresses(db.inner()).await
}

#[tauri::command]
pub async fn wallet_viewer_add_address(
    db: tauri::State<'_, DatabaseConnection>,
    address: String,
    label: Option<String>,
) -> Result<WalletAddress, AppError> {
    store::add_address(db.inner(), &address, label.as_deref()).await
}

#[tauri::command]
pub async fn wallet_viewer_remove_address(db: tauri::State<'_, DatabaseConnection>, id: i32) -> Result<(), AppError> {
    store::remove_address(db.inner(), id).await
}

#[tauri::command]
pub async fn wallet_viewer_get_balances(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<AddressBalances>, AppError> {
    let db = db.inner();
    store::require_enabled(db).await?;
    let mut result = Vec::new();
    for wallet in store::list_addresses(db).await? {
        let mut balances = Vec::new();
        for network in [&rpc::ETHEREUM, &rpc::BASE] {
            balances.push(read_balance(network, &wallet.address).await);
        }
        result.push(AddressBalances { wallet, balances });
    }
    Ok(result)
}

async fn read_balance(network: &Network, address: &str) -> NetworkBalance {
    match rpc::fetch_balance(network, address).await {
        Ok(wei) => NetworkBalance {
            network: network.name,
            wei: Some(wei.to_string()),
            eth: Some(format_ether(wei)),
            error: None,
        },
        Err(error) => NetworkBalance { network: network.name, wei: None, eth: None, error: Some(error.to_string()) },
    }
}
