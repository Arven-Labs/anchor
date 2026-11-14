// Visualizador de wallets (ADR 0002): comandos finos sobre `wallet_viewer::store` e `wallet_viewer::rpc`.
// Só leitura: nenhum comando aqui assina nada nem lida com chave privada.
use alloy_primitives::utils::format_ether;
use sea_orm::DatabaseConnection;
use serde::Serialize;

use crate::domain::bitcoin_xpub::{format_btc, parse_xpub};
use crate::error::AppError;
use crate::wallet_viewer::esplora::{self, EsploraSource};
use crate::wallet_viewer::ledger;
use crate::wallet_viewer::rpc::{self, Network};
use crate::wallet_viewer::trezor;
use crate::wallet_viewer::store::{self, WalletAddress, WalletXpub};

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

/// Lê o endereço Ethereum de uma conta da Ledger plugada (Fase 36.2). Não grava nada:
/// a tela mostra o endereço e o usuário confirma com `wallet_viewer_add_address`.
#[tauri::command]
pub async fn wallet_viewer_read_ledger_address(
    db: tauri::State<'_, DatabaseConnection>,
    account: u32,
) -> Result<String, AppError> {
    ledger::read_address_if_enabled(db.inner(), account, ledger::read_address_from_device).await
}

/// Lê o endereço Ethereum de uma conta do Trezor plugado (Fase 36.3). PIN e passphrase, se
/// pedidos, são digitados no aparelho. Não grava nada, como o da Ledger.
#[tauri::command]
pub async fn wallet_viewer_read_trezor_address(
    db: tauri::State<'_, DatabaseConnection>,
    account: u32,
) -> Result<String, AppError> {
    trezor::read_address_if_enabled(db.inner(), account, trezor::READ_LIMIT, trezor::read_address_from_device).await
}

#[tauri::command]
pub async fn wallet_viewer_list_xpubs(db: tauri::State<'_, DatabaseConnection>) -> Result<Vec<WalletXpub>, AppError> {
    store::list_xpubs(db.inner()).await
}

#[tauri::command]
pub async fn wallet_viewer_add_xpub(
    db: tauri::State<'_, DatabaseConnection>,
    xpub: String,
    label: Option<String>,
) -> Result<WalletXpub, AppError> {
    store::add_xpub(db.inner(), &xpub, label.as_deref()).await
}

#[tauri::command]
pub async fn wallet_viewer_remove_xpub(db: tauri::State<'_, DatabaseConnection>, id: i32) -> Result<(), AppError> {
    store::remove_xpub(db.inner(), id).await
}

/// Saldo de uma chave estendida de Bitcoin. Falha de uma chave não derruba as outras: vem em `error`.
#[derive(Serialize)]
pub struct XpubBalance {
    #[serde(flatten)]
    pub wallet: WalletXpub,
    /// BTC confirmados, texto decimal exato com 8 casas.
    pub confirmed_btc: Option<String>,
    /// Variação ainda no mempool, em BTC (pode ser negativa).
    pub pending_btc: Option<String>,
    pub used_addresses: Option<u32>,
    pub scanned_addresses: Option<u32>,
    /// `true` se a varredura bateu no teto sem fechar o gap: o saldo pode estar subestimado.
    pub truncated: bool,
    pub error: Option<String>,
}

/// Varre cada chave no Esplora, endereço a endereço (a chave nunca sai da máquina). Só roda ao
/// clicar em "Refresh": cada consulta revela um endereço derivado ao provedor.
#[tauri::command]
pub async fn wallet_viewer_get_bitcoin_balances(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<XpubBalance>, AppError> {
    let db = db.inner();
    store::require_enabled(db).await?;
    let mut result = Vec::new();
    for wallet in store::list_xpubs(db).await? {
        let scan = match parse_xpub(&wallet.xpub) {
            Ok(parsed) => esplora::scan_balance(&parsed, &EsploraSource).await,
            Err(error) => Err(error),
        };
        result.push(match scan {
            Ok(balance) => XpubBalance {
                confirmed_btc: Some(format_btc(i128::from(balance.confirmed_sat))),
                pending_btc: Some(format_btc(i128::from(balance.pending_sat))),
                used_addresses: Some(balance.used_addresses),
                scanned_addresses: Some(balance.scanned_addresses),
                truncated: balance.truncated,
                error: None,
                wallet,
            },
            Err(error) => XpubBalance {
                confirmed_btc: None,
                pending_btc: None,
                used_addresses: None,
                scanned_addresses: None,
                truncated: false,
                error: Some(error.to_string()),
                wallet,
            },
        });
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
