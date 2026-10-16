// Coletores de dados de mercado: buscam nas fontes públicas (via `client`, que
// chama `finance_core`) e gravam no `anchor.db`, uma classe de ativo por módulo.
// Foi a camada que falava com o sidecar do Lume (Fases 14 e 17); na Fase 26 o
// sidecar foi aposentado e `client` passou a usar as fontes nativas.
pub mod benchmark;
pub mod client;
pub mod crypto;
pub mod fii;
pub mod metals;
pub mod reit;
pub mod selic_store;
pub mod stocks;
pub mod us_stock;

use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};

use crate::error::AppError;

/// Muitas fontes respondem "esse ticker/empresa não tem esse dado" (ex.:
/// fundamentos da bolsai pra um FII, média de dividendos de um ticker sem
/// histórico), que chega como `AppError::NoData`. O coletor pula o item e deixa
/// qualquer outro erro propagar. Usado nos módulos por classe de ativo pra não
/// repetir esse `match` em cada chamada.
pub async fn skip_not_found<T, F>(fut: F) -> Result<Option<T>, AppError>
where
    F: std::future::Future<Output = Result<T, AppError>>,
{
    match fut.await {
        Ok(value) => Ok(Some(value)),
        Err(AppError::NoData(_)) => Ok(None),
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
