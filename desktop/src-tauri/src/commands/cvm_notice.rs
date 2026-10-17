use std::collections::HashMap;

use chrono::{Duration, NaiveDate, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::domain::dividend_suggestion::{quantity_held_by, LedgerRow};
use crate::entity::{assets, portfolio, transactions};
use crate::error::AppError;
use crate::finance_api::{client, skip_not_found};
use crate::finance_core::cvm_ipe::DividendNotice;

// Alerta de aviso de provento da CVM ("Relatório Proventos", Sessão 110). O
// aviso só traz data de entrega e link do PDF (o valor ficava no PDF, lido por
// IA até a Fase 20), então isto é só um alerta: não cria sugestão nem grava
// nada. Só empresas com código CVM, que o bolsai resolve a partir do ticker.
const CVM_ASSET_CLASSES: [&str; 2] = ["acao_br", "bdr"];
const RECENT_DAYS: i64 = 90;

#[derive(Serialize, Debug, PartialEq)]
pub struct CvmNoticeView {
    pub delivered_on: String,
    pub pdf_url: String,
}

#[derive(Serialize, Debug)]
pub struct CvmAssetNotices {
    pub asset_id: i32,
    pub ticker: String,
    pub notices: Vec<CvmNoticeView>,
    /// Falha só deste ativo (sem chave bolsai, CVM fora do ar): os outros seguem.
    pub error: Option<String>,
}

/// Avisos entregues de `since` em diante, do mais novo ao mais antigo.
fn recent_notices(notices: Vec<DividendNotice>, since: NaiveDate) -> Vec<CvmNoticeView> {
    let mut recent: Vec<DividendNotice> = notices.into_iter().filter(|n| n.data_entrega >= since).collect();
    recent.sort_by(|a, b| b.data_entrega.cmp(&a.data_entrega));
    recent
        .into_iter()
        .map(|n| CvmNoticeView { delivered_on: n.data_entrega.to_string(), pdf_url: n.link_download })
        .collect()
}

async fn notices_for(ticker: &str, since: NaiveDate) -> Result<Vec<CvmNoticeView>, AppError> {
    let Some(bolsai) = skip_not_found(client::fetch_stock_bolsai_fundamentals(ticker)).await? else {
        return Ok(vec![]);
    };
    let cvm_code: i32 = bolsai
        .cvm_code
        .trim()
        .parse()
        .map_err(|_| AppError::NoData(format!("invalid CVM code '{}' for {ticker}", bolsai.cvm_code)))?;
    let response = client::fetch_company_dividend_notices(cvm_code).await?;
    Ok(recent_notices(response.data, since))
}

#[tauri::command]
pub async fn list_cvm_dividend_notices(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<Vec<CvmAssetNotices>, AppError> {
    portfolio::Entity::find_by_id(portfolio_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("portfolio {portfolio_id}")))?;

    let txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.eq(portfolio_id))
        .all(db.inner())
        .await?;
    let mut ledger_by_asset: HashMap<i32, Vec<LedgerRow>> = HashMap::new();
    for tx in &txs {
        if let Some(asset_id) = tx.asset_id {
            ledger_by_asset.entry(asset_id).or_default().push(LedgerRow {
                transaction_type: tx.transaction_type.clone(),
                quantity: tx.quantity.unwrap_or(0.0),
                transaction_date: tx.transaction_date.clone(),
            });
        }
    }
    if ledger_by_asset.is_empty() {
        return Ok(vec![]);
    }

    let today = Utc::now().date_naive();
    let held = assets::Entity::find()
        .filter(assets::Column::Id.is_in(ledger_by_asset.keys().copied().collect::<Vec<i32>>()))
        .filter(assets::Column::AssetClass.is_in(CVM_ASSET_CLASSES))
        .all(db.inner())
        .await?
        .into_iter()
        .filter(|a| quantity_held_by(&ledger_by_asset[&a.id], &today.to_string()) > 0.0);

    let since = today - Duration::days(RECENT_DAYS);
    let mut result = Vec::new();
    for asset in held {
        let (notices, error) = match notices_for(&asset.ticker, since).await {
            Ok(notices) => (notices, None),
            Err(err) => (vec![], Some(err.to_string())),
        };
        result.push(CvmAssetNotices { asset_id: asset.id, ticker: asset.ticker, notices, error });
    }
    result.sort_by(|a, b| a.ticker.cmp(&b.ticker));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(date: &str, link: &str) -> DividendNotice {
        DividendNotice {
            protocolo_entrega: link.to_string(),
            data_entrega: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            link_download: link.to_string(),
        }
    }

    #[test]
    fn keeps_only_recent_notices_newest_first() {
        let since = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let notices = vec![
            notice("2026-06-30", "old"),
            notice("2026-07-01", "edge"),
            notice("2026-09-09", "new"),
        ];
        let views = recent_notices(notices, since);
        let urls: Vec<&str> = views.iter().map(|v| v.pdf_url.as_str()).collect();
        assert_eq!(urls, ["new", "edge"]);
        assert_eq!(views[0].delivered_on, "2026-09-09");
    }

    #[test]
    fn no_notice_in_the_window_is_an_empty_list() {
        let since = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        assert!(recent_notices(vec![notice("2025-01-01", "x")], since).is_empty());
    }

    #[tokio::test]
    #[ignore = "network: CVM IPE + bolsai key"]
    async fn live_notices_for_bbas3() {
        let since = Utc::now().date_naive() - Duration::days(365);
        let views = notices_for("BBAS3", since).await.expect("notices");
        assert!(!views.is_empty());
    }
}
