// Fase 33 — ingestão diária. Com o app aberto, uma tarefa em background busca
// uma vez por dia o que o usuário tem: câmbio das moedas em uso contra a moeda
// base de cada Workspace, cotação dos ativos com lançamento ou favoritados e os
// benchmarks. Cada rodada vira uma linha em `ingestion_runs`. A coleta manual
// continua igual; as duas dividem o mesmo trinco (`CollectorGuard`).

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use tauri::Manager;

use crate::commands::collector::{collect_asset, CollectorGuard};
use crate::commands::currency::{base_currency, workspace_asset_currencies};
use crate::commands::profitability::ASSET_CLASSES_WITH_AUTO_QUOTE;
use crate::entity::{
    asset_favorites, assets, bank_account, ingestion_runs, loan, stock_price_history, transactions,
    watchlist_items, workspace,
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
        targets.fx_pairs.extend(workspace_fx_pairs(db, ws.id).await?);
    }
    Ok(targets)
}

/// `(moeda, base)` de cada moeda em uso no Workspace (ativos com lançamento,
/// contas, empréstimos) diferente da base e presente no catálogo de câmbio.
pub async fn workspace_fx_pairs(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<BTreeSet<(String, String)>, AppError> {
    let base = base_currency(db, workspace_id).await?;
    let mut currencies = workspace_asset_currencies(db, workspace_id).await?;
    currencies.extend(
        bank_account::Entity::find()
            .filter(bank_account::Column::WorkspaceId.eq(workspace_id))
            .all(db)
            .await?
            .into_iter()
            .map(|a| a.currency),
    );
    currencies.extend(
        loan::Entity::find()
            .filter(loan::Column::WorkspaceId.eq(workspace_id))
            .all(db)
            .await?
            .into_iter()
            .map(|l| l.currency),
    );
    Ok(currencies
        .into_iter()
        .filter(|currency| *currency != base && is_supported_currency(currency) && is_supported_currency(&base))
        .map(|currency| (currency, base.clone()))
        .collect())
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

/// A decisão de cada conferência do laço: já é hora de uma nova rodada?
pub(crate) async fn is_due_now(db: &DatabaseConnection, now: DateTime<Utc>) -> Result<bool, AppError> {
    Ok(is_due(last_finished_at(db).await?, now))
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
            if matches!(is_due_now(&db, Utc::now()).await, Ok(true)) {
                let lock = handle.state::<AtomicBool>();
                // Ocupado (coleta manual) ou falha de rede: volta na próxima conferência.
                let _ = run_ingestion(&db, lock.inner(), trigger).await;
            }
            trigger = "scheduled";
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// Dias sem fechamento novo a partir dos quais o câmbio é considerado velho
/// (cobre fim de semana e feriado prolongado).
pub const STALE_AFTER_DAYS: i64 = 5;

#[derive(Debug, PartialEq, Serialize)]
pub struct FxFreshness {
    pub pair: String,
    /// `None` quando o par nunca foi coletado.
    pub last_date: Option<String>,
    pub days_old: Option<i64>,
}

/// Dos pares, os que nunca foram coletados ou cujo último fechamento tem mais de
/// `STALE_AFTER_DAYS` dias. `latest` mapeia código do par (`USDBRL`) à data mais recente.
pub fn stale_pairs(
    pairs: &BTreeSet<(String, String)>,
    latest: &HashMap<String, NaiveDate>,
    today: NaiveDate,
) -> Vec<FxFreshness> {
    pairs
        .iter()
        .filter_map(|(currency, base)| {
            let code = format!("{currency}{base}");
            let pair = format!("{currency}/{base}");
            match latest.get(&code) {
                None => Some(FxFreshness { pair, last_date: None, days_old: None }),
                Some(date) => {
                    let days_old = (today - *date).num_days();
                    (days_old > STALE_AFTER_DAYS).then(|| FxFreshness {
                        pair,
                        last_date: Some(date.to_string()),
                        days_old: Some(days_old),
                    })
                }
            }
        })
        .collect()
}

/// Pares de câmbio do Workspace com dado velho ou ausente, pra UI avisar.
pub async fn fx_staleness_in(
    db: &DatabaseConnection,
    workspace_id: i32,
    today: NaiveDate,
) -> Result<Vec<FxFreshness>, AppError> {
    let pairs = workspace_fx_pairs(db, workspace_id).await?;
    let codes: Vec<String> = pairs.iter().map(|(currency, base)| format!("{currency}{base}")).collect();
    let mut latest: HashMap<String, NaiveDate> = HashMap::new();
    if !codes.is_empty() {
        for row in stock_price_history::Entity::find()
            .filter(stock_price_history::Column::Ticker.is_in(codes))
            .all(db)
            .await?
        {
            if let Ok(date) = NaiveDate::parse_from_str(&row.price_date, "%Y-%m-%d") {
                let entry = latest.entry(row.ticker).or_insert(date);
                if date > *entry {
                    *entry = date;
                }
            }
        }
    }
    Ok(stale_pairs(&pairs, &latest, today))
}

#[tauri::command]
pub async fn get_fx_staleness(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<FxFreshness>, AppError> {
    fx_staleness_in(db.inner(), workspace_id, Utc::now().date_naive()).await
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
    fn pairs_are_stale_when_missing_or_older_than_the_limit() {
        let day = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let pairs: BTreeSet<(String, String)> = [("EUR", "BRL"), ("GBP", "BRL"), ("USD", "BRL")]
            .into_iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let latest: HashMap<String, NaiveDate> = [
            ("USDBRL".to_string(), day("2026-10-13")), // 5 dias: ainda vale
            ("EURBRL".to_string(), day("2026-10-12")), // 6 dias: velho
        ]
        .into_iter()
        .collect();

        let stale = stale_pairs(&pairs, &latest, day("2026-10-18"));
        assert_eq!(
            stale,
            vec![
                FxFreshness { pair: "EUR/BRL".into(), last_date: Some("2026-10-12".into()), days_old: Some(6) },
                FxFreshness { pair: "GBP/BRL".into(), last_date: None, days_old: None },
            ]
        );
    }

    #[test]
    fn status_reflects_the_counts() {
        assert_eq!(final_status(5, 0), STATUS_OK);
        assert_eq!(final_status(0, 0), STATUS_OK);
        assert_eq!(final_status(3, 2), STATUS_PARTIAL);
        assert_eq!(final_status(0, 4), STATUS_ERROR);
    }
}
