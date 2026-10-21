// PTAX do Banco Central gravada no `anchor.db` (`ptax_daily`), para o IR de exterior. A PTAX de um dia passado não muda,
// então não há TTL: busca-se só o trecho que falta (o começo, se os fatos são mais antigos que o gravado, e o fim, se
// são mais novos) e a leitura serve sempre o que está gravado. O BCB oscila (ver `bcb_sgs`): depois de uma falha a
// rede fica de molho por `COOLDOWN_SECS`, para abrir a aba Taxes não esperar vários timeouts seguidos.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};

use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::domain::ptax::PtaxSeries;
use crate::entity::ptax_daily;
use crate::error::AppError;
use crate::finance_core::bcb_ptax::{self, PtaxQuote};

/// Linhas por `INSERT`: 7 colunas × 500 fica bem abaixo do limite de variáveis do SQLite.
const INSERT_CHUNK: usize = 500;
/// Depois de uma falha do BCB, não tenta de novo por 10 minutos.
const COOLDOWN_SECS: i64 = 600;
/// Se o gravado começa até 7 dias depois do primeiro fato, não há buraco: o primeiro dia útil pode ser alguns dias depois.
const LEADING_SLACK_DAYS: i64 = 7;
/// Se o gravado termina até 4 dias antes do último fato, não há buraco (fim de semana com feriado emendado).
const TRAILING_SLACK_DAYS: i64 = 4;

static LAST_FAILURE: AtomicI64 = AtomicI64::new(0);

/// Trechos `[início, fim]` a buscar para cobrir `since..=until`, dado o que está gravado (`min` e `max` das datas).
pub fn plan_refresh(
    min: Option<NaiveDate>,
    max: Option<NaiveDate>,
    since: NaiveDate,
    until: NaiveDate,
) -> Vec<(NaiveDate, NaiveDate)> {
    let (Some(min), Some(max)) = (min, max) else {
        return vec![(since, until)];
    };
    let mut windows = Vec::new();
    if min > since + ChronoDuration::days(LEADING_SLACK_DAYS) {
        windows.push((since, min.pred_opt().expect("has predecessor")));
    }
    if until > max + ChronoDuration::days(TRAILING_SLACK_DAYS) {
        windows.push((max.succ_opt().expect("has successor"), until));
    }
    windows
}

/// Dentro da janela de espera depois de uma falha? `last_failure` em segundos unix, 0 = nunca falhou.
fn cooling_down(last_failure: i64, now: i64) -> bool {
    last_failure > 0 && now - last_failure < COOLDOWN_SECS
}

async fn bounds(db: &DatabaseConnection, currency: &str) -> Result<(Option<NaiveDate>, Option<NaiveDate>), AppError> {
    let rows = ptax_daily::Entity::find()
        .filter(ptax_daily::Column::Currency.eq(currency))
        .order_by_asc(ptax_daily::Column::ReferenceDate)
        .all(db)
        .await?;
    let date = |row: &ptax_daily::Model| NaiveDate::parse_from_str(&row.reference_date, "%Y-%m-%d").ok();
    Ok((rows.first().and_then(date), rows.last().and_then(date)))
}

async fn upsert(
    db: &DatabaseConnection,
    currency: &str,
    quotes: &BTreeMap<NaiveDate, PtaxQuote>,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let fetched_at = now.to_rfc3339();
    let rows: Vec<_> = quotes.iter().collect();
    for chunk in rows.chunks(INSERT_CHUNK) {
        let models = chunk.iter().map(|(date, quote)| ptax_daily::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            currency: Set(currency.to_string()),
            reference_date: Set(date.format("%Y-%m-%d").to_string()),
            buy: Set(quote.buy),
            sell: Set(quote.sell),
            source: Set(bcb_ptax::SOURCE.to_string()),
            fetched_at: Set(fetched_at.clone()),
        });
        ptax_daily::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([ptax_daily::Column::Currency, ptax_daily::Column::ReferenceDate])
                    .update_columns([
                        ptax_daily::Column::Buy,
                        ptax_daily::Column::Sell,
                        ptax_daily::Column::Source,
                        ptax_daily::Column::FetchedAt,
                    ])
                    .to_owned(),
            )
            .exec(db)
            .await?;
    }
    Ok(())
}

/// Só o que já está gravado, sem ir ao BCB.
pub async fn stored(db: &DatabaseConnection, currencies: &HashSet<String>) -> Result<PtaxSeries, AppError> {
    if currencies.is_empty() {
        return Ok(PtaxSeries::default());
    }
    let rows = ptax_daily::Entity::find()
        .filter(ptax_daily::Column::Currency.is_in(currencies.iter().cloned().collect::<Vec<_>>()))
        .all(db)
        .await?;
    let mut by_currency: HashMap<String, BTreeMap<NaiveDate, PtaxQuote>> = HashMap::new();
    for row in rows {
        if let Ok(date) = NaiveDate::parse_from_str(&row.reference_date, "%Y-%m-%d") {
            by_currency.entry(row.currency).or_default().insert(date, PtaxQuote { buy: row.buy, sell: row.sell });
        }
    }
    Ok(PtaxSeries::new(by_currency))
}

/// Completa o gravado de `currency` para cobrir `since..=until`, buscando só os trechos que faltam. Moeda que a PTAX
/// não cota ou BCB em espera (falha recente) não vai à rede; falha do BCB entra em espera e devolve o erro (quem chama
/// registra e segue com o que está gravado).
pub async fn refresh(
    db: &DatabaseConnection,
    currency: &str,
    since: NaiveDate,
    until: NaiveDate,
) -> Result<(), AppError> {
    if !bcb_ptax::is_supported(currency) {
        return Ok(());
    }
    let until = until.min(Utc::now().date_naive());
    if since > until {
        return Ok(());
    }
    let (min, max) = bounds(db, currency).await?;
    let windows = plan_refresh(min, max, since, until);
    if windows.is_empty() {
        return Ok(());
    }
    let now = Utc::now();
    if cooling_down(LAST_FAILURE.load(Ordering::Relaxed), now.timestamp()) {
        return Err(AppError::ProviderUnavailable("BCB PTAX: waiting after a recent failure".to_string()));
    }
    for (start, end) in windows {
        match bcb_ptax::fetch_range(currency, start, end).await {
            Ok(quotes) => upsert(db, currency, &quotes, now).await?,
            Err(err) => {
                LAST_FAILURE.store(now.timestamp(), Ordering::Relaxed);
                return Err(err);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn nothing_stored_fetches_the_whole_range() {
        assert_eq!(plan_refresh(None, None, d(2024, 3, 1), d(2026, 10, 1)), vec![(d(2024, 3, 1), d(2026, 10, 1))]);
    }

    #[test]
    fn a_covered_range_fetches_nothing() {
        let plan = plan_refresh(Some(d(2024, 3, 4)), Some(d(2026, 9, 30)), d(2024, 3, 1), d(2026, 10, 1));
        assert!(plan.is_empty(), "{plan:?}");
    }

    #[test]
    fn older_facts_fetch_the_head_that_is_missing() {
        let plan = plan_refresh(Some(d(2025, 1, 2)), Some(d(2026, 9, 30)), d(2024, 3, 1), d(2026, 10, 1));
        assert_eq!(plan, vec![(d(2024, 3, 1), d(2025, 1, 1))]);
    }

    #[test]
    fn newer_facts_fetch_only_the_tail() {
        let plan = plan_refresh(Some(d(2024, 3, 4)), Some(d(2026, 6, 30)), d(2024, 3, 1), d(2026, 10, 1));
        assert_eq!(plan, vec![(d(2026, 7, 1), d(2026, 10, 1))]);
    }

    #[test]
    fn a_gap_on_both_ends_fetches_both() {
        let plan = plan_refresh(Some(d(2025, 1, 2)), Some(d(2025, 6, 30)), d(2024, 3, 1), d(2026, 10, 1));
        assert_eq!(plan.len(), 2);
    }

    #[test]
    fn a_long_weekend_at_the_end_is_not_a_gap() {
        // gravado até a sexta, último fato na segunda de feriado: 3 dias, dentro da folga
        let plan = plan_refresh(Some(d(2024, 3, 4)), Some(d(2026, 9, 25)), d(2024, 3, 1), d(2026, 9, 28));
        assert!(plan.is_empty());
    }

    #[test]
    fn the_network_rests_for_ten_minutes_after_a_failure() {
        assert!(!cooling_down(0, 1_000_000), "nunca falhou");
        assert!(cooling_down(1_000_000, 1_000_300));
        assert!(!cooling_down(1_000_000, 1_000_600));
    }

    #[tokio::test]
    async fn upsert_is_idempotent_and_updates_the_overlap() {
        let db = crate::test_support::memory_db().await;
        let now = Utc::now();
        let q = |buy, sell| PtaxQuote { buy, sell };
        upsert(&db, "USD", &BTreeMap::from([(d(2025, 1, 2), q(6.2, 6.21)), (d(2025, 1, 3), q(6.1, 6.11))]), now)
            .await
            .unwrap();
        // Refetch com sobreposição: 03/01 revisado e 06/01 novo.
        upsert(&db, "USD", &BTreeMap::from([(d(2025, 1, 3), q(6.15, 6.16)), (d(2025, 1, 6), q(6.0, 6.01))]), now)
            .await
            .unwrap();

        let series = stored(&db, &HashSet::from(["USD".to_string()])).await.unwrap();
        use crate::domain::ptax::RateKind::Sell;
        assert_eq!(series.rate("USD", d(2025, 1, 2), Sell), Some(6.21));
        assert_eq!(series.rate("USD", d(2025, 1, 3), Sell), Some(6.16));
        assert_eq!(series.rate("USD", d(2025, 1, 6), Sell), Some(6.01));
        assert_eq!(bounds(&db, "USD").await.unwrap(), (Some(d(2025, 1, 2)), Some(d(2025, 1, 6))));
    }

    #[tokio::test]
    async fn only_the_asked_currencies_are_read() {
        let db = crate::test_support::memory_db().await;
        let quotes = BTreeMap::from([(d(2025, 1, 2), PtaxQuote { buy: 6.3, sell: 6.4 })]);
        upsert(&db, "EUR", &quotes, Utc::now()).await.unwrap();

        let none = stored(&db, &HashSet::from(["USD".to_string()])).await.unwrap();
        use crate::domain::ptax::RateKind::Sell;
        assert_eq!(none.rate("EUR", d(2025, 1, 2), Sell), None);
        assert_eq!(stored(&db, &HashSet::new()).await.unwrap().rate("EUR", d(2025, 1, 2), Sell), None);
    }
}
