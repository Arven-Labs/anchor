// Fase 33 — ingestão diária. Com o app aberto, uma tarefa em background busca
// uma vez por dia o que o usuário tem: câmbio das moedas em uso contra a moeda
// base de cada Workspace, cotação dos ativos com lançamento ou favoritados e os
// benchmarks. Cada rodada vira uma linha em `ingestion_runs`. A coleta manual
// continua igual; as duas dividem o mesmo trinco (`CollectorGuard`).

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use tauri::Manager;

use crate::commands::collector::{collect_asset, CollectorGuard};
use crate::commands::currency::{base_currency, workspace_asset_currencies};
use crate::commands::profitability::ASSET_CLASSES_WITH_AUTO_QUOTE;
use crate::entity::{
    asset_favorites, assets, bank_account, ingestion_runs, loan, transactions, watchlist_items, workspace,
};
use crate::error::AppError;
use crate::finance_api::{benchmark, fx as finance_api_fx};
use crate::finance_core::fx::CURRENCIES;

/// Intervalo entre as conferências de "já passou um dia?".
const CHECK_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// Idade mínima da última rodada concluída pra uma nova ser devida.
const MIN_AGE_HOURS: i64 = 24;
/// Pausa entre itens, pra não rajadar as fontes (o Yahoo limita por IP).
const PAUSE_BETWEEN_ITEMS: Duration = Duration::from_millis(500);
/// Quantas mensagens de erro entram no resumo da rodada.
const MAX_ERRORS_KEPT: usize = 5;
const DISABLE_ENV: &str = "ANCHOR_DISABLE_INGESTION";

pub const STATUS_RUNNING: &str = "running";
pub const STATUS_OK: &str = "ok";
pub const STATUS_PARTIAL: &str = "partial";
pub const STATUS_ERROR: &str = "error";

/// O que uma rodada vai buscar.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Targets {
    /// `(ticker, asset_class)` de ativo com cotação automática.
    pub assets: BTreeSet<(String, String)>,
    /// `(moeda, base)` de cada par a atualizar.
    pub fx_pairs: BTreeSet<(String, String)>,
}

/// Uma rodada é devida se nunca houve uma concluída ou a última tem 24 h ou mais.
pub fn is_due(last_finished: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    match last_finished {
        None => true,
        Some(at) => now - at >= chrono::Duration::hours(MIN_AGE_HOURS),
    }
}

fn is_supported_currency(code: &str) -> bool {
    CURRENCIES.iter().any(|(c, _)| *c == code)
}

/// `status` final a partir das contagens da rodada.
pub fn final_status(ok: i32, failed: i32) -> &'static str {
    match (ok, failed) {
        (_, 0) => STATUS_OK,
        (0, _) => STATUS_ERROR,
        _ => STATUS_PARTIAL,
    }
}

/// Ativos e pares de câmbio do que o usuário realmente tem.
pub async fn collection_targets(db: &DatabaseConnection) -> Result<Targets, AppError> {
    let mut targets = Targets::default();

    // Ativos: com lançamento, favoritados ou numa watchlist. Só classes com cotação automática.
    let mut asset_ids: BTreeSet<i32> = BTreeSet::new();
    asset_ids.extend(
        transactions::Entity::find()
            .select_only()
            .column(transactions::Column::AssetId)
            .distinct()
            .into_tuple::<Option<i32>>()
            .all(db)
            .await?
            .into_iter()
            .flatten(),
    );
    asset_ids.extend(asset_favorites::Entity::find().all(db).await?.into_iter().map(|f| f.asset_id));
    asset_ids.extend(watchlist_items::Entity::find().all(db).await?.into_iter().map(|w| w.asset_id));
    if !asset_ids.is_empty() {
        for asset in assets::Entity::find().filter(assets::Column::Id.is_in(asset_ids)).all(db).await? {
            if ASSET_CLASSES_WITH_AUTO_QUOTE.contains(&asset.asset_class.as_str()) {
                targets.assets.insert((asset.ticker, asset.asset_class));
            }
        }
    }

    // Câmbio: moedas de ativos, contas e empréstimos de cada Workspace contra a base dele.
    for ws in workspace::Entity::find().all(db).await? {
        let base = base_currency(db, ws.id).await?;
        let mut currencies = workspace_asset_currencies(db, ws.id).await?;
        currencies.extend(
            bank_account::Entity::find()
                .filter(bank_account::Column::WorkspaceId.eq(ws.id))
                .all(db)
                .await?
                .into_iter()
                .map(|a| a.currency),
        );
        currencies.extend(
            loan::Entity::find()
                .filter(loan::Column::WorkspaceId.eq(ws.id))
                .all(db)
                .await?
                .into_iter()
                .map(|l| l.currency),
        );
        for currency in currencies {
            if currency != base && is_supported_currency(&currency) && is_supported_currency(&base) {
                targets.fx_pairs.insert((currency, base.clone()));
            }
        }
    }
    Ok(targets)
}

/// Fim da última rodada que terminou (`ok` ou `partial`), pra decidir a próxima.
pub(crate) async fn last_finished_at(db: &DatabaseConnection) -> Result<Option<DateTime<Utc>>, AppError> {
    Ok(ingestion_runs::Entity::find()
        .filter(ingestion_runs::Column::Status.is_in([STATUS_OK, STATUS_PARTIAL]))
        .order_by_desc(ingestion_runs::Column::Id)
        .one(db)
        .await?
        .and_then(|run| run.finished_at)
        .and_then(|at| DateTime::parse_from_rfc3339(&at).ok())
        .map(|at| at.with_timezone(&Utc)))
}

/// Uma rodada completa. Falha de um item não interrompe as outras; `CollectorBusy`
/// (coleta manual em andamento) volta sem gravar nada, pra quem chamou tentar depois.
pub async fn run_ingestion(
    db: &DatabaseConnection,
    lock: &AtomicBool,
    trigger: &str,
) -> Result<ingestion_runs::Model, AppError> {
    let _guard = CollectorGuard::acquire(lock)?;
    let run = ingestion_runs::ActiveModel {
        trigger_kind: Set(trigger.to_string()),
        started_at: Set(Utc::now().to_rfc3339()),
        status: Set(STATUS_RUNNING.to_string()),
        items_ok: Set(0),
        items_failed: Set(0),
        ..Default::default()
    }
    .insert(db)
    .await?;

    let mut ok = 0;
    let mut failed = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut record = |label: String, result: Result<(), AppError>| match result {
        Ok(()) => ok += 1,
        Err(err) => {
            failed += 1;
            if errors.len() < MAX_ERRORS_KEPT {
                errors.push(format!("{label}: {err}"));
            }
        }
    };

    match collection_targets(db).await {
        Ok(targets) => {
            for (currency, base) in &targets.fx_pairs {
                record(format!("{currency}/{base}"), finance_api_fx::collect_pair(db, currency, base).await.map(drop));
                tokio::time::sleep(PAUSE_BETWEEN_ITEMS).await;
            }
            for (ticker, class) in &targets.assets {
                record(ticker.clone(), collect_asset(db, ticker, Some(class)).await.map(drop));
                tokio::time::sleep(PAUSE_BETWEEN_ITEMS).await;
            }
        }
        Err(err) => record("targets".to_string(), Err(err)),
    }
    record("benchmarks".to_string(), benchmark::collect_benchmarks(db).await.map(drop));

    let mut finished: ingestion_runs::ActiveModel = run.into();
    finished.finished_at = Set(Some(Utc::now().to_rfc3339()));
    finished.status = Set(final_status(ok, failed).to_string());
    finished.items_ok = Set(ok);
    finished.items_failed = Set(failed);
    finished.error = Set(if errors.is_empty() { None } else { Some(errors.join("\n")) });
    Ok(finished.update(db).await?)
}

/// Confere a cada 30 min se a última rodada tem 24 h ou mais; a primeira
/// conferência é na abertura do app. Desligável por `ANCHOR_DISABLE_INGESTION`
/// (o E2E usa, pra a coleta de fundo não disputar o trinco com os testes).
pub fn spawn_daily_ingestion(handle: tauri::AppHandle, db: DatabaseConnection) {
    if std::env::var_os(DISABLE_ENV).is_some() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut trigger = "startup";
        loop {
            if matches!(last_finished_at(&db).await.map(|at| is_due(at, Utc::now())), Ok(true)) {
                let lock = handle.state::<AtomicBool>();
                // Ocupado (coleta manual) ou falha de rede: volta na próxima conferência.
                let _ = run_ingestion(&db, lock.inner(), trigger).await;
            }
            trigger = "scheduled";
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

#[tauri::command]
pub async fn list_ingestion_runs(
    db: tauri::State<'_, DatabaseConnection>,
    limit: Option<u64>,
) -> Result<Vec<ingestion_runs::Model>, AppError> {
    Ok(ingestion_runs::Entity::find()
        .order_by_desc(ingestion_runs::Column::Id)
        .limit(limit.unwrap_or(10).min(100))
        .all(db.inner())
        .await?)
}

#[tauri::command]
pub async fn run_ingestion_now(
    lock: tauri::State<'_, AtomicBool>,
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<ingestion_runs::Model, AppError> {
    run_ingestion(db.inner(), lock.inner(), "manual").await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn due_when_never_ran_or_a_day_old() {
        let now = at("2026-10-18T12:00:00Z");
        assert!(is_due(None, now));
        assert!(is_due(Some(at("2026-10-17T12:00:00Z")), now));
        assert!(is_due(Some(at("2026-10-10T00:00:00Z")), now));
        assert!(!is_due(Some(at("2026-10-17T12:00:01Z")), now));
        assert!(!is_due(Some(at("2026-10-18T11:59:00Z")), now));
    }

    #[test]
    fn status_reflects_the_counts() {
        assert_eq!(final_status(5, 0), STATUS_OK);
        assert_eq!(final_status(0, 0), STATUS_OK);
        assert_eq!(final_status(3, 2), STATUS_PARTIAL);
        assert_eq!(final_status(0, 4), STATUS_ERROR);
    }
}
