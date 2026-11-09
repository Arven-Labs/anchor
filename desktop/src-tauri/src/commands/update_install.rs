// P9 — instalar a atualização que `check_for_update` achou. Dois comandos, de
// propósito: `download_update` baixa e **verifica a assinatura** (a chave pública
// está no `tauri.conf.json`) antes de qualquer byte ser aplicado; `install_update`
// só aplica o que já foi baixado e verificado, e reinicia. Separar permite provar
// a verificação sem substituir o executável de quem testa. Nada aqui roda sozinho:
// quem decide baixar e instalar é o usuário, por botão.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_updater::Update;

use super::update_check::map_updater_error;
use crate::error::AppError;

#[derive(Default)]
enum Stage {
    #[default]
    Empty,
    /// Achada pela checagem, ainda não baixada.
    Found(Update),
    /// Baixada e com a assinatura verificada, pronta para instalar.
    Downloaded(Update, Vec<u8>),
}

#[derive(Default)]
pub struct PendingUpdate(Mutex<Stage>);

impl PendingUpdate {
    // Um `Mutex` envenenado só significa que outra thread entrou em pânico segurando o
    // trinco; o estado continua válido (um enum), então segue em frente.
    fn lock(&self) -> std::sync::MutexGuard<'_, Stage> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn clear(&self) {
        *self.lock() = Stage::Empty;
    }

    pub fn set_found(&self, update: Update) {
        *self.lock() = Stage::Found(update);
    }

    fn found(&self) -> Option<Update> {
        match &*self.lock() {
            Stage::Found(update) => Some(update.clone()),
            _ => None,
        }
    }

    fn set_downloaded(&self, update: Update, bytes: Vec<u8>) {
        *self.lock() = Stage::Downloaded(update, bytes);
    }

    fn take_downloaded(&self) -> Option<(Update, Vec<u8>)> {
        let mut stage = self.lock();
        match std::mem::take(&mut *stage) {
            Stage::Downloaded(update, bytes) => Some((update, bytes)),
            other => {
                *stage = other;
                None
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DownloadedUpdate {
    pub version: String,
}

#[tauri::command]
pub async fn download_update(
    pending: State<'_, PendingUpdate>,
) -> Result<DownloadedUpdate, AppError> {
    let update = pending.found().ok_or_else(|| {
        AppError::InvalidInput("no update to download; check for updates first".to_string())
    })?;
    // `download` confere a assinatura contra a chave pública da config; se não bater,
    // devolve erro e nada fica guardado.
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(map_updater_error)?;
    let version = update.version.clone();
    pending.set_downloaded(update, bytes);
    Ok(DownloadedUpdate { version })
}

#[tauri::command]
pub fn install_update(app: AppHandle, pending: State<'_, PendingUpdate>) -> Result<(), AppError> {
    let (update, bytes) = pending.take_downloaded().ok_or_else(|| {
        AppError::InvalidInput("no downloaded update; download it first".to_string())
    })?;
    update.install(bytes).map_err(map_updater_error)?;
    app.restart()
}
