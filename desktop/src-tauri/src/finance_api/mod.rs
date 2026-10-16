// Fase 14.2 — infraestrutura de acesso à Finance API (easybusiness), que vai
// substituir o `data-collector/` local por completo até o fim da Fase 14.
// `sidecar` cuida do ciclo de vida do processo (release) / endpoint fixo
// (dev); `client` é o HTTP client tipado consumido por cima dele. Nenhum
// `#[tauri::command]` chama isso ainda — a Fase 14.4 é quem porta a lógica
// de fetch+write do coletor Python pra cima deste client.
pub mod benchmark;
pub mod client;
pub mod crypto;
pub mod fii;
pub mod metals;
pub mod reit;
pub mod selic_store;
pub mod sidecar;
pub mod stocks;
pub mod us_stock;

pub use sidecar::FinanceApiHandle;

use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};

use crate::error::AppError;

/// Vários endpoints da Finance API voltam 404 pra "esse ticker/empresa não
/// tem esse dado" (ex.: `bolsai-fundamentals` pra um FII, `dividends-avg`
/// pra um ticker sem histórico) — mesmo contrato que
/// `data-collector/sources/finance_api_client.py` já tinha (`
/// FinanceApiNotFoundError` capturado dentro do loop por-ticker, deixando
/// qualquer outro erro propagar). Usado nos módulos por classe de ativo pra
/// não repetir esse `match` em cada chamada.
pub async fn skip_not_found<T, F>(fut: F) -> Result<Option<T>, AppError>
where
    F: std::future::Future<Output = Result<T, AppError>>,
{
    match fut.await {
        Ok(value) => Ok(Some(value)),
        Err(AppError::FinanceApiNotFound(_) | AppError::NoData(_)) => Ok(None),
        Err(err) => Err(err),
    }
}

/// `insert_many(...).on_conflict(...).do_nothing()` erra com
/// `DbErr::RecordNotInserted` quando toda linha do lote já existia (ou é um
/// único item que já existia) — não é falha de verdade, é o mesmo "0 linha
/// nova" que `conn.total_changes` media do lado Python
/// (`INSERT OR IGNORE`). Normalizado aqui pra não vazar como erro de
/// comando.
pub async fn insert_ignoring_conflicts<E>(
    db: &DatabaseConnection,
    models: Vec<E::ActiveModel>,
    conflict: OnConflict,
) -> Result<(), AppError>
where
    E: EntityTrait,
{
    if models.is_empty() {
        return Ok(());
    }
    match E::insert_many(models).on_conflict(conflict).exec(db).await {
        Ok(_) | Err(DbErr::RecordNotInserted) => Ok(()),
        Err(err) => Err(err.into()),
    }
}
