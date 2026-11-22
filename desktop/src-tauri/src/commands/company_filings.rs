use chrono::{Duration, NaiveDate, Utc};
use serde::Serialize;

use crate::error::AppError;
use crate::finance_api::{client, skip_not_found};
use crate::finance_core::cvm_ipe::{self, CompanyFiling, FilingKind};

// Releases e apresentações de resultado da empresa, do IPE da CVM (frente 2 do ROADMAP, item
// "puxar o RI"). Só links para o PDF no portal da CVM: nada é baixado nem lido. O ticker vira
// código CVM pelo bolsai, como em `cvm_notice.rs`.
const RECENT_DAYS: i64 = 365;
const MAX_ROWS: usize = 12;

#[derive(Serialize, Debug, PartialEq)]
pub struct CompanyFilingView {
    pub delivered_on: String,
    pub kind: FilingKind,
    pub subject: String,
    pub pdf_url: String,
}

/// Entregas de `since` em diante (inclusive), da mais nova à mais antiga, no máximo `limit`.
fn recent_filings(filings: Vec<CompanyFiling>, since: NaiveDate, limit: usize) -> Vec<CompanyFilingView> {
    let mut recent: Vec<CompanyFiling> = filings.into_iter().filter(|f| f.data_entrega >= since).collect();
    recent.sort_by(|a, b| {
        b.data_entrega.cmp(&a.data_entrega).then_with(|| a.protocolo_entrega.cmp(&b.protocolo_entrega))
    });
    recent
        .into_iter()
        .take(limit)
        .map(|f| CompanyFilingView {
            delivered_on: f.data_entrega.to_string(),
            kind: f.kind,
            subject: f.subject,
            pdf_url: f.link_download,
        })
        .collect()
}

async fn filings_for(ticker: &str, since: NaiveDate) -> Result<Vec<CompanyFilingView>, AppError> {
    let Some(bolsai) = skip_not_found(client::fetch_stock_bolsai_fundamentals(ticker)).await? else {
        return Ok(vec![]);
    };
    let cvm_code: i32 = bolsai
        .cvm_code
        .trim()
        .parse()
        .map_err(|_| AppError::NoData(format!("invalid CVM code '{}' for {ticker}", bolsai.cvm_code)))?;
    Ok(recent_filings(cvm_ipe::company_filings(cvm_code).await?, since, MAX_ROWS))
}

#[tauri::command]
pub async fn list_company_filings(ticker: String) -> Result<Vec<CompanyFilingView>, AppError> {
    let since = Utc::now().date_naive() - Duration::days(RECENT_DAYS);
    filings_for(ticker.trim(), since).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filing(date: &str, link: &str, kind: FilingKind) -> CompanyFiling {
        CompanyFiling {
            reference_date: "2026-02-10".to_string(),
            version: 1,
            protocolo_entrega: link.to_string(),
            data_entrega: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            link_download: link.to_string(),
            kind,
            subject: format!("assunto {link}"),
        }
    }

    fn since() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
    }

    #[test]
    fn keeps_only_recent_filings_newest_first() {
        let filings = vec![
            filing("2026-06-30", "old", FilingKind::Release),
            filing("2026-07-01", "edge", FilingKind::Release),
            filing("2026-11-05", "new", FilingKind::Presentation),
        ];
        let views = recent_filings(filings, since(), 10);
        let urls: Vec<&str> = views.iter().map(|v| v.pdf_url.as_str()).collect();
        assert_eq!(urls, ["new", "edge"]);
        assert_eq!(views[0].delivered_on, "2026-11-05");
        assert_eq!(views[0].kind, FilingKind::Presentation);
        assert_eq!(views[0].subject, "assunto new");
    }

    #[test]
    fn the_limit_keeps_the_newest() {
        let filings = vec![
            filing("2026-08-01", "a", FilingKind::Release),
            filing("2026-09-01", "b", FilingKind::Release),
            filing("2026-10-01", "c", FilingKind::Release),
        ];
        let urls: Vec<String> = recent_filings(filings, since(), 2).into_iter().map(|v| v.pdf_url).collect();
        assert_eq!(urls, ["c", "b"]);
    }

    #[test]
    fn the_limit_is_per_kind_so_material_facts_do_not_push_the_releases_out() {
        let mut filings = vec![
            filing("2026-08-01", "rel-1", FilingKind::Release),
            filing("2026-08-02", "rel-2", FilingKind::Release),
            filing("2026-08-03", "pres-1", FilingKind::Presentation),
        ];
        // Oito fatos relevantes, todos mais novos que os releases.
        for day in 1..=8 {
            filings.push(filing(&format!("2026-10-{day:02}"), &format!("fact-{day}"), FilingKind::MaterialFact));
        }
        let urls: Vec<String> = recent_filings(filings, since(), 3).into_iter().map(|v| v.pdf_url).collect();
        // 3 de cada tipo, do mais novo ao mais antigo no conjunto.
        assert_eq!(urls, ["fact-8", "fact-7", "fact-6", "pres-1", "rel-2", "rel-1"]);
    }

    #[test]
    fn a_kind_over_the_limit_keeps_its_newest() {
        let filings: Vec<CompanyFiling> = (1..=4)
            .map(|d| filing(&format!("2026-09-{d:02}"), &format!("rel-{d}"), FilingKind::Release))
            .collect();
        let urls: Vec<String> = recent_filings(filings, since(), 2).into_iter().map(|v| v.pdf_url).collect();
        assert_eq!(urls, ["rel-4", "rel-3"]);
    }

    #[test]
    fn the_window_applies_to_material_facts_too() {
        let filings = vec![
            filing("2026-06-30", "old-fact", FilingKind::MaterialFact),
            filing("2026-07-01", "edge-fact", FilingKind::MaterialFact),
        ];
        let urls: Vec<String> = recent_filings(filings, since(), 10).into_iter().map(|v| v.pdf_url).collect();
        assert_eq!(urls, ["edge-fact"]);
    }

    #[test]
    fn nothing_in_the_window_is_an_empty_list() {
        assert!(recent_filings(vec![filing("2025-01-01", "x", FilingKind::Release)], since(), 10).is_empty());
    }

    #[test]
    fn same_day_filings_keep_a_stable_order() {
        let filings = vec![
            filing("2026-11-05", "b", FilingKind::Release),
            filing("2026-11-05", "a", FilingKind::Presentation),
        ];
        let urls: Vec<String> = recent_filings(filings, since(), 10).into_iter().map(|v| v.pdf_url).collect();
        assert_eq!(urls, ["a", "b"]);
    }

    #[tokio::test]
    #[ignore = "network: CVM IPE + bolsai key"]
    async fn live_filings_for_bbas3() {
        let since = Utc::now().date_naive() - Duration::days(365);
        let views = filings_for("BBAS3", since).await.expect("filings");
        println!("BBAS3: {} filing(s), newest {:?}", views.len(), views.first());
    }
}
