// Estatísticas de índices da B3 (IFIX, SMLL, IDIV) — porta de
// `lume/api/app/sources/b3_index_stats.py`. API interna do site da B3,
// pública e sem chave:
// `GET https://sistemaswebb3-listados.b3.com.br/indexStatisticsProxy/IndexCall/GetPortfolioDay/{base64}`
// onde `{base64}` é o base64 de `{"language": "en-us", "index": "<COD>", "year": <ANO>}`.
// Devolve, por ano, uma grade fixa de 31 linhas (`day`) × 12 colunas
// (`rateValue1..12`, jan..dez); vazio/nulo em dia sem pregão ou que não existe
// no mês. O valor vem como `"3,314.09"` (vírgula de milhar, por causa do `en-us`).

use base64::Engine;
use chrono::{Datelike, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::error::AppError;
use crate::finance_core::http;
use crate::finance_core::provider::{cascade, BoxFuture, DataProvider, Sourced};

pub const SOURCE: &str = "b3_index_stats";
const URL: &str =
    "https://sistemaswebb3-listados.b3.com.br/indexStatisticsProxy/IndexCall/GetPortfolioDay";

/// Catálogo travado nos 3 índices já validados em produção: `(slug, código B3, ano-base)`.
/// Não acrescentar código/ano sem confirmar contra a API real.
const CATALOG: [(&str, &str, i32); 3] = [("ifix", "IFIX", 2010), ("smll", "SMLL", 2005), ("idiv", "IDIV", 2005)];

pub fn index_info(slug: &str) -> Option<(&'static str, i32)> {
    let slug = slug.to_lowercase();
    CATALOG.iter().find(|(s, _, _)| *s == slug).map(|&(_, code, start)| (code, start))
}

#[derive(Deserialize)]
struct Grid {
    #[serde(default)]
    results: Vec<Value>,
}

/// Fechamentos de um ano a partir da grade devolvida pela B3.
pub fn parse_year(body: &str, year: i32) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let bad = |detail: String| AppError::ProviderUnavailable(format!("B3 index stats: {detail}"));
    let grid: Grid = serde_json::from_str(body).map_err(|e| bad(e.to_string()))?;
    let mut points = Vec::new();
    for row in &grid.results {
        let day = row
            .get("day")
            .and_then(Value::as_u64)
            .ok_or_else(|| bad("row missing 'day'".into()))? as u32;
        for month in 1..=12u32 {
            let raw = match row.get(format!("rateValue{month}")) {
                Some(Value::String(s)) if !s.is_empty() => s.as_str(),
                _ => continue,
            };
            // Dia que não existe no mês (ex.: 31 de fevereiro): a grade é fixa.
            let Some(date) = NaiveDate::from_ymd_opt(year, month, day) else { continue };
            let value = raw
                .replace(',', "")
                .parse::<f64>()
                .map_err(|_| bad(format!("invalid value '{raw}'")))?;
            points.push((date, value));
        }
    }
    Ok(points)
}

fn url_for(code: &str, year: i32) -> String {
    // Mesmo texto do `json.dumps` do Python (separadores `, ` e `: `).
    let params = format!(r#"{{"language": "en-us", "index": "{code}", "year": {year}}}"#);
    format!("{URL}/{}", base64::engine::general_purpose::STANDARD.encode(params))
}

/// Histórico diário de um índice entre `start_year` e `end_year`, um pedido por
/// ano. Anos antes da base do índice voltam com grade vazia (inofensivo).
async fn fetch_history(code: &str, start_year: i32, end_year: i32) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let mut all = Vec::new();
    for year in start_year..=end_year {
        let body = http::get_text("B3 index stats", &url_for(code, year), http::DEFAULT_TIMEOUT).await?;
        all.extend(parse_year(&body, year)?);
    }
    Ok(all)
}

pub trait IndexHistoryProvider: DataProvider {
    fn history<'a>(&'a self, slug: &'a str) -> BoxFuture<'a, Result<Vec<(NaiveDate, f64)>, AppError>>;
}

pub struct B3IndexStats;

impl DataProvider for B3IndexStats {
    fn id(&self) -> &'static str {
        SOURCE
    }
}

impl IndexHistoryProvider for B3IndexStats {
    fn history<'a>(&'a self, slug: &'a str) -> BoxFuture<'a, Result<Vec<(NaiveDate, f64)>, AppError>> {
        Box::pin(async move {
            let (code, start_year) = index_info(slug)
                .ok_or_else(|| AppError::NoData(format!("unknown B3 index '{slug}'")))?;
            fetch_history(code, start_year, Utc::now().year()).await
        })
    }
}

fn providers() -> Vec<Box<dyn IndexHistoryProvider>> {
    vec![Box::new(B3IndexStats)]
}

/// Histórico diário completo de `ifix`, `smll` ou `idiv`.
pub async fn index_history(slug: &str) -> Result<Sourced<Vec<(NaiveDate, f64)>>, AppError> {
    let providers = providers();
    let attempts = providers.iter().map(|p| (p.id(), p.history(slug))).collect();
    cascade(&format!("B3 index '{slug}'"), attempts).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn grid_is_parsed_and_empty_cells_skipped() {
        let body = r#"{"results":[
            {"day":1,"rateValue1":"3,314.09"},
            {"day":2,"rateValue1":""},
            {"day":3,"rateValue1":null,"rateValue2":"4,000.50"},
            {"day":31,"rateValue2":"100.00","rateValue3":"7.25"}
        ]}"#;
        assert_eq!(
            parse_year(body, 2026).unwrap(),
            vec![(d(2026, 1, 1), 3314.09), (d(2026, 2, 3), 4000.5), (d(2026, 3, 31), 7.25)]
        ); // 31 de fevereiro não existe e é pulado
    }

    #[test]
    fn empty_or_missing_results_is_an_empty_year() {
        assert!(parse_year(r#"{"results":[]}"#, 2005).unwrap().is_empty());
        assert!(parse_year("{}", 2005).unwrap().is_empty());
    }

    #[test]
    fn malformed_payloads_are_provider_unavailable() {
        for body in [
            "<html>",
            r#"{"results":[{"rateValue1":"1.0"}]}"#,
            r#"{"results":[{"day":1,"rateValue1":"abc"}]}"#,
        ] {
            assert!(matches!(parse_year(body, 2026), Err(AppError::ProviderUnavailable(_))), "{body}");
        }
    }

    #[test]
    fn url_carries_the_base64_of_the_python_json_text() {
        let url = url_for("IFIX", 2026);
        let encoded = url.rsplit('/').next().unwrap();
        let decoded = base64::engine::general_purpose::STANDARD.decode(encoded).unwrap();
        assert_eq!(
            String::from_utf8(decoded).unwrap(),
            r#"{"language": "en-us", "index": "IFIX", "year": 2026}"#
        );
    }

    #[test]
    fn catalog_is_locked_to_the_three_validated_indexes() {
        assert_eq!(index_info("IFIX"), Some(("IFIX", 2010)));
        assert_eq!(index_info("smll"), Some(("SMLL", 2005)));
        assert_eq!(index_info("idiv"), Some(("IDIV", 2005)));
        assert_eq!(index_info("ibov"), None);
    }

    // Ao vivo (rede): `cargo test --lib -- --ignored finance_core::b3_index_stats`.
    #[tokio::test]
    #[ignore]
    async fn live_ifix_history() {
        let ifix = index_history("ifix").await.unwrap();
        assert_eq!(ifix.source, SOURCE);
        assert!(ifix.value.len() > 3000, "{} points", ifix.value.len());
        let (date, last) = *ifix.value.last().unwrap();
        println!("IFIX last {date} {last}");
        assert!(last > 1000.0 && last < 10000.0);
        assert!(matches!(index_history("ibov").await.unwrap_err(), AppError::NoData(_)));
    }
}
