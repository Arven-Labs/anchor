// BCB SGS (Sistema Gerenciador de Séries Temporais) — porta de
// `lume/api/app/sources/bcb_sgs.py`. Pública, sem chave:
// `GET https://api.bcb.gov.br/dados/serie/bcdata.sgs.{code}/dados?formato=json`
// devolve `[{"data": "dd/mm/yyyy", "valor": "0.47"}, ...]`.
//
// Séries mensais (CDI 4391, IPCA 433) vêm sempre com `data` no dia 1 e `valor`
// já em % no mês. A Meta Selic (432) é diária e o BCB recusa (HTTP 406) uma
// busca diária sem janela: por isso pagina em janelas de 9 anos. O BCB oscila
// (mesma janela leva 0,4 s ou 30 s, ou devolve 502, ou uma página HTML com
// status 200), então cada janela tem timeout maior e até 3 tentativas.

use std::sync::OnceLock;
use std::time::Duration;

use chrono::{Datelike, NaiveDate, Utc};
use serde::Deserialize;

use crate::error::AppError;
use crate::finance_core::cache::{Cached, TtlCache};
use crate::finance_core::http;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "bcb_sgs";
const URL_TEMPLATE: &str = "https://api.bcb.gov.br/dados/serie/bcdata.sgs";

const DAILY_TIMEOUT: Duration = Duration::from_secs(45);
const DAILY_WINDOW_ATTEMPTS: u32 = 3;
const DAILY_RETRY_BACKOFF: Duration = Duration::from_secs(2);
/// Teto real do BCB é 10 anos; 9 deixa margem.
const DAILY_WINDOW_YEARS: i32 = 9;
/// Nenhuma série diária do BCB começa antes disso.
const DAILY_EARLIEST_YEAR: i32 = 1994;
/// Mesmo TTL padrão do Lume (`cache_ttl_seconds`).
const SERIES_TTL: Duration = Duration::from_secs(3600);

// ---------------------------------------------------------------------------
// Catálogo — deliberadamente pequeno (só séries confirmadas ao vivo)
// ---------------------------------------------------------------------------

fn monthly_bcb_code(slug: &str) -> Option<u32> {
    match slug {
        "cdi" => Some(4391),
        "ipca" => Some(433),
        _ => None,
    }
}

fn daily_bcb_code(slug: &str) -> Option<u32> {
    match slug {
        "meta" => Some(432),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Item {
    data: String,
    valor: String,
}

fn parse_items(body: &str) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let bad = |detail: String| AppError::ProviderUnavailable(format!("BCB SGS: {detail}"));
    let items: Vec<Item> = serde_json::from_str(body).map_err(|e| bad(e.to_string()))?;
    items
        .into_iter()
        .map(|item| {
            let date = NaiveDate::parse_from_str(&item.data, "%d/%m/%Y")
                .map_err(|e| bad(format!("invalid date '{}': {e}", item.data)))?;
            let value = item
                .valor
                .parse::<f64>()
                .map_err(|_| bad(format!("invalid value '{}'", item.valor)))?;
            Ok((date, value))
        })
        .collect()
}

/// `(mês de referência no dia 1, % no mês)`.
pub fn parse_monthly(body: &str) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    Ok(parse_items(body)?
        .into_iter()
        .map(|(date, value)| (date.with_day(1).expect("day 1 always exists"), value))
        .collect())
}

/// `(data real, valor)` — mantém o dia, pra série que publica 1 valor por dia útil.
pub fn parse_daily(body: &str) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    parse_items(body)
}

/// Janelas `[início, fim]` de 9 anos de 1994 até `today`, contíguas e sem sobreposição.
pub fn daily_windows(today: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut windows = Vec::new();
    let mut start = NaiveDate::from_ymd_opt(DAILY_EARLIEST_YEAR, 1, 1).expect("valid date");
    while start <= today {
        let next_start =
            NaiveDate::from_ymd_opt(start.year() + DAILY_WINDOW_YEARS, 1, 1).expect("valid date");
        let end = (next_start.pred_opt().expect("has predecessor")).min(today);
        windows.push((start, end));
        start = end.succ_opt().expect("has successor");
    }
    windows
}

// ---------------------------------------------------------------------------
// Fetch
// ---------------------------------------------------------------------------

async fn fetch_monthly(code: u32) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let url = format!("{URL_TEMPLATE}.{code}/dados?formato=json");
    parse_monthly(&http::get_text("BCB SGS", &url, http::DEFAULT_TIMEOUT).await?)
}

async fn fetch_daily_window(
    code: u32,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let url = format!(
        "{URL_TEMPLATE}.{code}/dados?formato=json&dataInicial={}&dataFinal={}",
        start.format("%d/%m/%Y"),
        end.format("%d/%m/%Y")
    );
    let mut last = None;
    for attempt in 0..DAILY_WINDOW_ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(DAILY_RETRY_BACKOFF).await;
        }
        // Corpo HTML com status 200 cai no parse e também é tentado de novo.
        match http::get_text("BCB SGS", &url, DAILY_TIMEOUT).await.and_then(|b| parse_daily(&b)) {
            Ok(points) => return Ok(points),
            // 404 não melhora com nova tentativa.
            Err(err @ AppError::NoData(_)) => return Err(err),
            Err(err) => last = Some(err),
        }
    }
    Err(last.expect("at least one attempt"))
}

async fn fetch_daily(code: u32) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let mut all = Vec::new();
    for (start, end) in daily_windows(Utc::now().date_naive()) {
        all.extend(fetch_daily_window(code, start, end).await?);
    }
    Ok(all)
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

/// Séries macro do Brasil. Hoje só o BCB responde; um provider pago/BYO-key
/// entraria como outra implementação na lista de `providers()`.
pub trait MacroSeriesProvider: DataProvider {
    fn monthly<'a>(&'a self, slug: &'a str) -> BoxFuture<'a, Result<Vec<(NaiveDate, f64)>, AppError>>;
    fn daily<'a>(&'a self, slug: &'a str) -> BoxFuture<'a, Result<Vec<(NaiveDate, f64)>, AppError>>;
}

pub struct BcbSgs;

impl DataProvider for BcbSgs {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl MacroSeriesProvider for BcbSgs {
    fn monthly<'a>(&'a self, slug: &'a str) -> BoxFuture<'a, Result<Vec<(NaiveDate, f64)>, AppError>> {
        Box::pin(async move {
            let code = monthly_bcb_code(slug)
                .ok_or_else(|| AppError::NoData(format!("unknown macro series '{slug}'")))?;
            fetch_monthly(code).await
        })
    }

    fn daily<'a>(&'a self, slug: &'a str) -> BoxFuture<'a, Result<Vec<(NaiveDate, f64)>, AppError>> {
        Box::pin(async move {
            let code = daily_bcb_code(slug)
                .ok_or_else(|| AppError::NoData(format!("unknown daily series '{slug}'")))?;
            fetch_daily(code).await
        })
    }
}

fn providers() -> Vec<Box<dyn MacroSeriesProvider>> {
    vec![Box::new(BcbSgs)]
}

/// Série mensal completa (`cdi`, `ipca`), do mais antigo ao mais recente.
pub async fn monthly_series(slug: &str) -> Result<Sourced<Vec<(NaiveDate, f64)>>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.monthly(slug))).collect();
    cascade(&format!("macro series '{slug}'"), attempts).await
}

static DAILY_CACHE: OnceLock<TtlCache<String, Sourced<Vec<(NaiveDate, f64)>>>> = OnceLock::new();

/// Série diária completa (`meta` = Meta Selic), em cache de 1 h na memória: a
/// Home e o backfill de benchmarks pedem a série inteira (~4 janelas) toda vez.
pub async fn daily_series(
    slug: &str,
) -> Result<Cached<Sourced<Vec<(NaiveDate, f64)>>>, AppError> {
    let cache = DAILY_CACHE.get_or_init(TtlCache::new);
    cache
        .get_or_refresh(slug.to_string(), SERIES_TTL, || async {
            let providers = providers();
            let attempts = providers.iter().map(|p| (p.id(), p.daily(slug))).collect();
            cascade(&format!("daily series '{slug}'"), attempts).await
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn monthly_parses_and_forces_day_one() {
        let body = r#"[{"data":"01/06/2003","valor":"1.14"},{"data":"15/07/2003","valor":"1.35"}]"#;
        assert_eq!(
            parse_monthly(body).unwrap(),
            vec![(d(2003, 6, 1), 1.14), (d(2003, 7, 1), 1.35)]
        );
    }

    #[test]
    fn daily_keeps_the_real_day() {
        let body = r#"[{"data":"12/09/2026","valor":"14.00"},{"data":"15/09/2026","valor":"14.00"}]"#;
        assert_eq!(
            parse_daily(body).unwrap(),
            vec![(d(2026, 9, 12), 14.0), (d(2026, 9, 15), 14.0)]
        );
    }

    #[test]
    fn malformed_payloads_are_provider_unavailable() {
        for body in [
            r#"[{"bad":"shape"}]"#,
            "<html>502 Bad Gateway</html>",
            r#"[{"data":"31/02/2020","valor":"1"}]"#,
            r#"[{"data":"01/01/2020","valor":"abc"}]"#,
        ] {
            assert!(
                matches!(parse_monthly(body), Err(AppError::ProviderUnavailable(_))),
                "{body}"
            );
        }
    }

    #[test]
    fn windows_are_contiguous_nine_year_blocks_until_today() {
        let today = d(2026, 10, 16);
        let windows = daily_windows(today);
        assert_eq!(windows[0], (d(1994, 1, 1), d(2002, 12, 31)));
        assert_eq!(windows[1], (d(2003, 1, 1), d(2011, 12, 31)));
        assert_eq!(*windows.last().unwrap(), (d(2021, 1, 1), today));
        for pair in windows.windows(2) {
            assert_eq!(pair[0].1.succ_opt().unwrap(), pair[1].0, "gap or overlap");
        }
        for (start, end) in &windows {
            assert!(end.year() - start.year() <= DAILY_WINDOW_YEARS);
        }
    }

    #[test]
    fn window_ending_exactly_on_a_block_boundary_is_not_duplicated() {
        let windows = daily_windows(d(2002, 12, 31));
        assert_eq!(windows, vec![(d(1994, 1, 1), d(2002, 12, 31))]);
    }

    #[test]
    fn catalog_knows_only_confirmed_series() {
        assert_eq!(monthly_bcb_code("cdi"), Some(4391));
        assert_eq!(monthly_bcb_code("ipca"), Some(433));
        assert_eq!(monthly_bcb_code("selic"), None);
        assert_eq!(daily_bcb_code("meta"), Some(432));
        assert_eq!(daily_bcb_code("cdi"), None);
    }

    #[tokio::test]
    async fn unknown_series_is_no_data() {
        let err = monthly_series("bitcoin").await.unwrap_err();
        assert!(matches!(err, AppError::NoData(_)), "{err}");
    }

    // Ao vivo (rede): `cargo test --lib -- --ignored finance_core::bcb_sgs`.
    #[tokio::test]
    #[ignore]
    async fn live_cdi_and_meta_selic() {
        let cdi = monthly_series("cdi").await.unwrap();
        assert_eq!(cdi.source, SOURCE);
        assert!(cdi.value.len() > 200, "{} points", cdi.value.len());
        assert!(cdi.value.iter().all(|(date, _)| date.day() == 1));

        let selic = daily_series("meta").await.unwrap();
        assert!(!selic.cached);
        let (_, current) = *selic.value.value.last().unwrap();
        assert!(current > 2.0 && current < 40.0, "Selic {current}% looks wrong");
        assert!(daily_series("meta").await.unwrap().cached);
    }
}
