// Fase 24 — fundos de investimento (classe `fundo`) e a estimativa do come-cotas. O usuário diz o tipo de cada fundo
// (longo prazo, curto prazo ou ações) e lança o valor da cota por data; a estimativa (`domain::tax::come_cotas`) usa a
// posição do Workspace e o último valor de cota. O desconto real do come-cotas não é registrado aqui (próxima fatia).

use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};

use crate::commands::tax::{load_rule_set, workspace_portfolio_ids};
use crate::commands::transaction::{cost_events, COME_COTAS};
use crate::domain::cost_basis;
use crate::domain::tax::come_cotas::{self, ComeCotasEstimate, FundHolding, FundTerm, QuotaValue};
use crate::domain::tax::fund_redemption::{self, FundEvent, FundEventKind, RedemptionEstimate};
use crate::entity::{assets, fund_detail, fund_quota_value, transactions};
use crate::error::AppError;

const FUND_CLASS: &str = "fundo";
const BUY: &str = "compra";
const SELL: &str = "venda";

/// Um desconto de come-cotas já registrado (transação `come_cotas`).
#[derive(Serialize)]
pub struct DeductionView {
    pub tx_id: i32,
    pub date: String,
    pub quotas: f64,
    pub quota_value: f64,
    /// O imposto em reais (cotas × valor da cota).
    pub tax: f64,
}

/// Um fundo com a posição do Workspace e os valores de cota lançados (do mais antigo ao mais novo).
#[derive(Serialize)]
pub struct FundView {
    pub asset_id: i32,
    pub ticker: String,
    pub name: String,
    /// `longo_prazo`, `curto_prazo` ou `acoes`; `None` até o usuário escolher.
    pub term: Option<String>,
    pub quantity: f64,
    pub average_cost: Option<f64>,
    pub quotas: Vec<fund_quota_value::Model>,
    /// Descontos de come-cotas registrados, do mais antigo ao mais novo.
    pub deductions: Vec<DeductionView>,
    /// IR e IOF estimados de cada resgate (venda), do mais antigo ao mais novo; vazio até o fundo ter tipo.
    pub redemptions: Vec<RedemptionEstimate>,
}

#[derive(Serialize)]
pub struct FundsOverview {
    pub funds: Vec<FundView>,
    /// Estimativa do próximo come-cotas, um item por fundo (com o motivo quando não há imposto a estimar).
    pub estimates: Vec<ComeCotasEstimate>,
}

#[derive(Deserialize)]
pub struct UpsertFundQuotaRequest {
    pub asset_id: i32,
    /// `YYYY-MM-DD`.
    pub quota_date: String,
    /// Valor de uma cota, em BRL.
    pub value: f64,
}

/// Os fatos do fundo que importam para o IR no resgate: aplicações (custo = valor + taxa), come-cotas e resgates. Data
/// inválida fica de fora (não derruba a tela).
fn fund_events(txs: &[&transactions::Model]) -> Vec<FundEvent> {
    txs.iter()
        .filter_map(|t| {
            let date = NaiveDate::parse_from_str(&t.transaction_date, "%Y-%m-%d").ok()?;
            let quantity = t.quantity.unwrap_or(0.0);
            let kind = match t.transaction_type.as_str() {
                BUY => FundEventKind::Buy { quantity, cost: t.total_value + t.fee.unwrap_or(0.0) },
                COME_COTAS => FundEventKind::ComeCotas { quantity, quota_value: t.unit_price.unwrap_or(0.0) },
                SELL => FundEventKind::Redeem { tx_id: t.id, quantity, quota_value: t.unit_price.unwrap_or(0.0) },
                _ => return None,
            };
            Some(FundEvent { date, id: t.id, kind })
        })
        .collect()
}

async fn fund_asset(db: &DatabaseConnection, asset_id: i32) -> Result<assets::Model, AppError> {
    let asset = assets::Entity::find_by_id(asset_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {asset_id}")))?;
    if asset.asset_class != FUND_CLASS {
        return Err(AppError::InvalidInput(format!("{} is not a fund", asset.ticker)));
    }
    Ok(asset)
}

#[tauri::command]
pub async fn get_funds_overview(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<FundsOverview, AppError> {
    funds_overview_in(db.inner(), workspace_id, Utc::now().date_naive()).await
}

pub async fn funds_overview_in(
    db: &DatabaseConnection,
    workspace_id: i32,
    today: NaiveDate,
) -> Result<FundsOverview, AppError> {
    let fund_assets = assets::Entity::find()
        .filter(assets::Column::AssetClass.eq(FUND_CLASS))
        .order_by_asc(assets::Column::Ticker)
        .all(db)
        .await?;
    if fund_assets.is_empty() {
        return Ok(FundsOverview { funds: Vec::new(), estimates: Vec::new() });
    }
    let ids: Vec<i32> = fund_assets.iter().map(|a| a.id).collect();

    let terms: HashMap<i32, String> = fund_detail::Entity::find()
        .filter(fund_detail::Column::AssetId.is_in(ids.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|d| (d.asset_id, d.term))
        .collect();
    let quota_rows = fund_quota_value::Entity::find()
        .filter(fund_quota_value::Column::AssetId.is_in(ids.clone()))
        .order_by_asc(fund_quota_value::Column::QuotaDate)
        .all(db)
        .await?;

    let portfolio_ids = workspace_portfolio_ids(db, workspace_id).await?;
    let txs = if portfolio_ids.is_empty() {
        Vec::new()
    } else {
        transactions::Entity::find()
            .filter(transactions::Column::PortfolioId.is_in(portfolio_ids))
            .filter(transactions::Column::AssetId.is_in(ids))
            .all(db)
            .await?
    };

    let rules = load_rule_set(db).await?;
    let mut funds = Vec::with_capacity(fund_assets.len());
    let mut holdings = Vec::with_capacity(fund_assets.len());
    for asset in &fund_assets {
        let asset_txs: Vec<&transactions::Model> = txs.iter().filter(|t| t.asset_id == Some(asset.id)).collect();
        let state = cost_basis::replay(&cost_events(&asset_txs)?, &cost_basis::native_rate)
            .expect("a taxa nativa sempre existe");
        let first_buy = asset_txs
            .iter()
            .filter(|t| t.transaction_type == BUY)
            .filter_map(|t| NaiveDate::parse_from_str(&t.transaction_date, "%Y-%m-%d").ok())
            .min();
        let quotas: Vec<fund_quota_value::Model> =
            quota_rows.iter().filter(|q| q.asset_id == asset.id).cloned().collect();
        let term = terms.get(&asset.id).cloned();

        let mut deductions: Vec<DeductionView> = asset_txs
            .iter()
            .filter(|t| t.transaction_type == COME_COTAS)
            .map(|t| {
                let quotas = t.quantity.unwrap_or(0.0);
                DeductionView {
                    tx_id: t.id,
                    date: t.transaction_date.clone(),
                    quotas,
                    quota_value: t.unit_price.unwrap_or(0.0),
                    tax: t.total_value,
                }
            })
            .collect();
        deductions.sort_by(|a, b| (&a.date, a.tx_id).cmp(&(&b.date, b.tx_id)));

        // IR e IOF estimados de cada resgate (venda): precisa do tipo do fundo.
        let redemptions = match term.as_deref().and_then(FundTerm::parse) {
            Some(fund_term) => fund_redemption::estimate_redemptions(&fund_events(&asset_txs), fund_term, &rules)?,
            None => Vec::new(),
        };

        holdings.push(FundHolding {
            asset_id: asset.id,
            ticker: asset.ticker.clone(),
            name: asset.name.clone(),
            term: term.as_deref().and_then(FundTerm::parse),
            quantity: state.quantity,
            average_cost: state.average_price().unwrap_or(0.0),
            first_buy,
            quotas: quotas
                .iter()
                .filter_map(|q| {
                    NaiveDate::parse_from_str(&q.quota_date, "%Y-%m-%d")
                        .ok()
                        .map(|date| QuotaValue { date, value: q.value })
                })
                .collect(),
            deductions: deductions
                .iter()
                .filter_map(|d| {
                    NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
                        .ok()
                        .map(|date| QuotaValue { date, value: d.quota_value })
                })
                .collect(),
        });
        funds.push(FundView {
            asset_id: asset.id,
            ticker: asset.ticker.clone(),
            name: asset.name.clone(),
            term,
            quantity: state.quantity,
            average_cost: state.average_price(),
            quotas,
            deductions,
            redemptions,
        });
    }

    let estimates = come_cotas::estimate(&holdings, &rules, today)?;
    Ok(FundsOverview { funds, estimates })
}

#[tauri::command]
pub async fn set_fund_term(
    db: tauri::State<'_, DatabaseConnection>,
    asset_id: i32,
    term: String,
) -> Result<(), AppError> {
    set_fund_term_in(db.inner(), asset_id, &term).await
}

pub async fn set_fund_term_in(db: &DatabaseConnection, asset_id: i32, term: &str) -> Result<(), AppError> {
    if FundTerm::parse(term).is_none() {
        return Err(AppError::InvalidInput("term must be longo_prazo, curto_prazo or acoes".to_string()));
    }
    fund_asset(db, asset_id).await?;
    match fund_detail::Entity::find().filter(fund_detail::Column::AssetId.eq(asset_id)).one(db).await? {
        Some(row) => {
            let mut active: fund_detail::ActiveModel = row.into();
            active.term = Set(term.to_string());
            active.update(db).await?;
        }
        None => {
            fund_detail::ActiveModel {
                asset_id: Set(asset_id),
                term: Set(term.to_string()),
                created_at: Set(Utc::now().to_rfc3339()),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn upsert_fund_quota(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpsertFundQuotaRequest,
) -> Result<fund_quota_value::Model, AppError> {
    upsert_fund_quota_in(db.inner(), request, Utc::now().date_naive()).await
}

pub async fn upsert_fund_quota_in(
    db: &DatabaseConnection,
    request: UpsertFundQuotaRequest,
    today: NaiveDate,
) -> Result<fund_quota_value::Model, AppError> {
    let date = NaiveDate::parse_from_str(&request.quota_date, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidInput("quota date must be YYYY-MM-DD".to_string()))?;
    if date > today {
        return Err(AppError::InvalidInput("quota date cannot be in the future".to_string()));
    }
    if !request.value.is_finite() || request.value <= 0.0 {
        return Err(AppError::InvalidInput("quota value must be above zero".to_string()));
    }
    fund_asset(db, request.asset_id).await?;

    let existing = fund_quota_value::Entity::find()
        .filter(fund_quota_value::Column::AssetId.eq(request.asset_id))
        .filter(fund_quota_value::Column::QuotaDate.eq(request.quota_date.clone()))
        .one(db)
        .await?;
    match existing {
        Some(row) => {
            let mut active: fund_quota_value::ActiveModel = row.into();
            active.value = Set(request.value);
            Ok(active.update(db).await?)
        }
        None => Ok(fund_quota_value::ActiveModel {
            asset_id: Set(request.asset_id),
            quota_date: Set(request.quota_date),
            value: Set(request.value),
            created_at: Set(Utc::now().to_rfc3339()),
            ..Default::default()
        }
        .insert(db)
        .await?),
    }
}

#[tauri::command]
pub async fn delete_fund_quota(
    db: tauri::State<'_, DatabaseConnection>,
    quota_id: i32,
) -> Result<(), AppError> {
    fund_quota_value::Entity::delete_by_id(quota_id).exec(db.inner()).await?;
    Ok(())
}
