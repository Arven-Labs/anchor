// Home do Workspace, fatia 1 (layout fixo) — agregados cruzando todos os
// portfolios do workspace, mesmo precedente de `commands::net_worth::get_net_worth_summary`
// (que já soma `compute_positions` de cada `portfolio::Model` do workspace).
// Sem migration nova, tudo derivado on-the-fly.

use std::collections::HashMap;

use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::commands::portfolio_summary::ClassAllocation;
use crate::commands::profitability::{compute_profitability, MonthlyReturn};
use crate::commands::currency::{base_currency, positions_in_base, sum_in_base};
use crate::entity::{portfolio, transactions};
use crate::error::AppError;

const DIVIDEND: &str = "provento";

#[derive(Serialize)]
pub struct PortfolioBreakdown {
    pub portfolio_id: i32,
    pub name: String,
    pub market_value: f64,
}

#[derive(Serialize)]
pub struct WorkspaceSummary {
    pub total_market_value: f64,
    pub total_cost_basis: f64,
    pub unrealized_pl: f64,
    pub unrealized_pl_pct: Option<f64>,
    pub dividends_received_12m: f64,
    pub allocation_by_class: Vec<ClassAllocation>,
    pub positions_missing_price: i32,
    // Fase 19 — mesma semântica de `PortfolioSummary`.
    pub base_currency: String,
    pub positions_missing_fx: i32,
    pub portfolios: Vec<PortfolioBreakdown>,
}

#[tauri::command]
pub async fn get_workspace_summary(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<WorkspaceSummary, AppError> {
    let db = db.inner();

    let portfolios = portfolio::Entity::find()
        .filter(portfolio::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;

    let mut total_market_value = 0.0;
    let mut total_cost_basis = 0.0;
    let mut positions_missing_price = 0;
    let mut positions_missing_fx = 0;
    let mut by_class: HashMap<String, f64> = HashMap::new();
    let mut breakdown = Vec::with_capacity(portfolios.len());
    let base_currency = base_currency(db, workspace_id).await?;

    for p in &portfolios {
        let (positions, _) = positions_in_base(db, p.id).await?;
        let mut portfolio_market_value = 0.0;

        for position in &positions {
            if position.fx_missing {
                if position.quantity > 0.0 {
                    positions_missing_fx += 1;
                }
            } else if let Some(market_value) = position.market_value {
                total_market_value += market_value;
                portfolio_market_value += market_value;
                *by_class.entry(position.asset_class.clone()).or_insert(0.0) += market_value;
            } else if position.quantity > 0.0 {
                positions_missing_price += 1;
            }
            if let Some(avg_price) = position.average_buy_price {
                total_cost_basis += avg_price * position.quantity;
            }
        }

        breakdown.push(PortfolioBreakdown {
            portfolio_id: p.id,
            name: p.name.clone(),
            market_value: portfolio_market_value,
        });
    }

    let unrealized_pl = total_market_value - total_cost_basis;
    let unrealized_pl_pct = (total_cost_basis != 0.0).then_some(unrealized_pl / total_cost_basis);

    let mut allocation_by_class: Vec<ClassAllocation> = by_class
        .into_iter()
        .map(|(asset_class, market_value)| ClassAllocation {
            asset_class,
            market_value,
            pct: if total_market_value != 0.0 {
                market_value / total_market_value
            } else {
                0.0
            },
        })
        .collect();
    allocation_by_class.sort_by(|a, b| b.market_value.partial_cmp(&a.market_value).unwrap());

    let portfolio_ids: Vec<i32> = portfolios.iter().map(|p| p.id).collect();
    let since = (Utc::now() - Duration::days(365)).format("%Y-%m-%d").to_string();
    let dividend_txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.is_in(portfolio_ids))
        .filter(transactions::Column::TransactionType.eq(DIVIDEND))
        .filter(transactions::Column::TransactionDate.gte(since))
        .all(db)
        .await?;
    let dividends_received_12m = sum_in_base(db, &dividend_txs, &base_currency).await?;

    Ok(WorkspaceSummary {
        base_currency,
        positions_missing_fx,
        total_market_value,
        total_cost_basis,
        unrealized_pl,
        unrealized_pl_pct,
        dividends_received_12m,
        allocation_by_class,
        positions_missing_price,
        portfolios: breakdown,
    })
}

#[tauri::command]
pub async fn get_workspace_profitability(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<MonthlyReturn>, AppError> {
    let db = db.inner();
    let portfolio_ids: Vec<i32> = portfolio::Entity::find()
        .filter(portfolio::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.id)
        .collect();

    compute_profitability(db, &portfolio_ids, None).await
}
