// Fase 26 — Meta Selic diária gravada no `anchor.db` (`selic_daily`). O BCB é
// lento e oscila; a série inteira (4 janelas de 9 anos) só é baixada na
// primeira vez, depois busca-se só o trecho novo e, com a fonte fora do ar,
// serve-se o que está gravado como `stale`. Substitui o cache do Lume.

use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::entity::selic_daily;
use crate::error::AppError;
use crate::finance_core::bcb_sgs;

/// Mesmo TTL padrão do Lume (`cache_ttl_seconds`).
const TTL: Duration = Duration::from_secs(3600);
/// Linhas por `INSERT`: 5 colunas × 500 fica bem abaixo do limite de variáveis do SQLite.
const INSERT_CHUNK: usize = 500;

#[derive(Debug, PartialEq)]
pub enum Refresh {
    /// Gravado há menos que o TTL: não consulta a fonte.
    Fresh,
    /// Busca a partir de `since` (inclusive); `None` = série inteira.
    Fetch { since: Option<NaiveDate> },
}

pub fn plan_refresh(
    latest_date: Option<NaiveDate>,
    last_fetched: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Refresh {
    match (latest_date, last_fetched) {
        (Some(_), Some(fetched)) if (now - fetched).to_std().map(|age| age < TTL).unwrap_or(true) => {
            Refresh::Fresh
        }
        (latest, _) => Refresh::Fetch { since: latest },
    }
}

pub struct SelicSeries {
    pub points: Vec<(NaiveDate, f64)>,
    pub cached: bool,
    pub stale: bool,
    pub fetched_at: DateTime<Utc>,
}

struct Stored {
    points: Vec<(NaiveDate, f64)>,
    last_fetched: Option<DateTime<Utc>>,
}

async fn read_stored(db: &DatabaseConnection, slug: &str) -> Result<Stored, AppError> {
    let rows = selic_daily::Entity::find()
        .filter(selic_daily::Column::SeriesCode.eq(slug))
        .order_by_asc(selic_daily::Column::ReferenceDate)
        .all(db)
        .await?;
    let mut last_fetched: Option<DateTime<Utc>> = None;
    let mut points = Vec::with_capacity(rows.len());
    for row in rows {
        if let Ok(date) = NaiveDate::parse_from_str(&row.reference_date, "%Y-%m-%d") {
            points.push((date, row.value_pct));
        }
        if let Ok(fetched) = DateTime::parse_from_rfc3339(&row.fetched_at) {
            let fetched = fetched.with_timezone(&Utc);
            last_fetched = Some(last_fetched.map_or(fetched, |prev| prev.max(fetched)));
        }
    }
    Ok(Stored { points, last_fetched })
}

async fn upsert(
    db: &DatabaseConnection,
    slug: &str,
    source: &str,
    points: &[(NaiveDate, f64)],
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let fetched_at = now.to_rfc3339();
    for chunk in points.chunks(INSERT_CHUNK) {
        let models = chunk.iter().map(|(date, value_pct)| selic_daily::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            series_code: Set(slug.to_string()),
            reference_date: Set(date.format("%Y-%m-%d").to_string()),
            value_pct: Set(*value_pct),
            source: Set(source.to_string()),
            fetched_at: Set(fetched_at.clone()),
        });
        selic_daily::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    selic_daily::Column::SeriesCode,
                    selic_daily::Column::ReferenceDate,
                ])
                .update_columns([
                    selic_daily::Column::ValuePct,
                    selic_daily::Column::Source,
                    selic_daily::Column::FetchedAt,
                ])
                .to_owned(),
            )
            .exec(db)
            .await?;
    }
    Ok(())
}

/// Só o que já está gravado, sem ir ao BCB (para tarefas em segundo plano que não devem esperar a rede).
pub async fn stored(db: &DatabaseConnection, slug: &str) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    Ok(read_stored(db, slug).await?.points)
}

pub async fn load(db: &DatabaseConnection, slug: &str) -> Result<SelicSeries, AppError> {
    let stored = read_stored(db, slug).await?;
    let now = Utc::now();
    let latest = stored.points.last().map(|(date, _)| *date);

    let since = match plan_refresh(latest, stored.last_fetched, now) {
        Refresh::Fresh => {
            return Ok(SelicSeries {
                points: stored.points,
                cached: true,
                stale: false,
                fetched_at: stored.last_fetched.unwrap_or(now),
            });
        }
        Refresh::Fetch { since } => since,
    };

    match bcb_sgs::daily_series_since(slug, since).await {
        Ok(fetched) => {
            upsert(db, slug, fetched.source, &fetched.value, now).await?;
            Ok(SelicSeries {
                points: read_stored(db, slug).await?.points,
                cached: false,
                stale: false,
                fetched_at: now,
            })
        }
        // Fonte fora do ar com série gravada: serve o que tem, marcado `stale`.
        Err(AppError::ProviderUnavailable(msg)) if !stored.points.is_empty() => {
            eprintln!("Selic: serving stored series after provider failure: {msg}");
            Ok(SelicSeries {
                points: stored.points,
                cached: true,
                stale: true,
                fetched_at: stored.last_fetched.unwrap_or(now),
            })
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn empty_store_fetches_everything() {
        assert_eq!(plan_refresh(None, None, Utc::now()), Refresh::Fetch { since: None });
    }

    #[test]
    fn recent_fetch_is_fresh() {
        let now = Utc::now();
        let plan = plan_refresh(Some(d(2026, 9, 15)), Some(now - ChronoDuration::minutes(10)), now);
        assert_eq!(plan, Refresh::Fresh);
    }

    #[test]
    fn old_fetch_asks_only_for_the_tail() {
        let now = Utc::now();
        let plan = plan_refresh(Some(d(2026, 9, 15)), Some(now - ChronoDuration::hours(2)), now);
        assert_eq!(plan, Refresh::Fetch { since: Some(d(2026, 9, 15)) });
    }

    #[tokio::test]
    async fn upsert_is_idempotent_and_updates_the_overlap() {
        let db = crate::test_support::memory_db().await;
        let now = Utc::now();
        upsert(&db, "meta", "bcb_sgs", &[(d(2026, 9, 12), 14.0), (d(2026, 9, 15), 14.0)], now)
            .await
            .unwrap();
        // Refetch do trecho com sobreposição: 15/09 revisado e 16/09 novo.
        upsert(&db, "meta", "bcb_sgs", &[(d(2026, 9, 15), 14.25), (d(2026, 9, 16), 14.25)], now)
            .await
            .unwrap();

        let stored = read_stored(&db, "meta").await.unwrap();
        assert_eq!(
            stored.points,
            vec![(d(2026, 9, 12), 14.0), (d(2026, 9, 15), 14.25), (d(2026, 9, 16), 14.25)]
        );
        assert!(stored.last_fetched.is_some());
    }

    #[tokio::test]
    async fn upsert_handles_a_full_history_in_chunks() {
        let db = crate::test_support::memory_db().await;
        let start = d(1994, 1, 1);
        let points: Vec<_> =
            (0..(INSERT_CHUNK as i64 * 3 + 7)).map(|i| (start + ChronoDuration::days(i), 10.0)).collect();
        upsert(&db, "meta", "bcb_sgs", &points, Utc::now()).await.unwrap();
        assert_eq!(read_stored(&db, "meta").await.unwrap().points.len(), points.len());
    }

    #[tokio::test]
    async fn other_series_do_not_mix() {
        let db = crate::test_support::memory_db().await;
        upsert(&db, "meta", "bcb_sgs", &[(d(2026, 9, 12), 14.0)], Utc::now()).await.unwrap();
        assert!(read_stored(&db, "other").await.unwrap().points.is_empty());
    }

    // Ao vivo (rede): `cargo test --lib -- --ignored finance_api::selic_store`.
    #[tokio::test]
    #[ignore]
    async fn live_load_downloads_once_then_serves_from_the_database() {
        let db = crate::test_support::memory_db().await;
        let first = load(&db, "meta").await.unwrap();
        assert!(!first.cached && !first.stale);
        assert!(first.points.len() > 5000, "{} points", first.points.len());
        let (_, current) = *first.points.last().unwrap();
        assert!(current > 2.0 && current < 40.0, "Selic {current}% looks wrong");

        let second = load(&db, "meta").await.unwrap();
        assert!(second.cached && !second.stale);
        assert_eq!(second.points.len(), first.points.len());
    }
}
