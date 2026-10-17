// Fase 13.1 — agregados pra tela "Resumo" da Carteira (KPI cards + donut de
// alocação). Reaproveita `transaction::compute_positions` (já precifica cada
// posição via `domain::position_pricing`) em vez de duplicar a reconstrução
// de posição — este comando só soma/agrupa o que `compute_positions` já
// devolve, mais os proventos recebidos direto de `transactions`.

use std::collections::HashMap;

use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::commands::currency::{positions_in_base, sum_in_base};
use crate::entity::transactions;
use crate::error::AppError;

const DIVIDEND: &str = "provento";

#[derive(Serialize)]
pub struct ClassAllocation {
    pub asset_class: String,
    pub market_value: f64,
    pub pct: f64,
}

#[derive(Serialize)]
pub struct PortfolioSummary {
    pub total_market_value: f64,
    pub total_cost_basis: f64,
    pub unrealized_pl: f64,
    pub unrealized_pl_pct: Option<f64>,
    pub dividends_received_12m: f64,
    pub allocation_by_class: Vec<ClassAllocation>,
    // Contagem de posições com quantidade > 0 sem nenhum preço/avaliação
    // encontrada (`price_source == "none"`) — sinaliza na UI que os
    // totais acima estão incompletos, sem impedir o cálculo do resto.
    pub positions_missing_price: i32,
    // Fase 19 — moeda dos totais acima e quantas posições ficaram de fora por
    // falta de cotação do par pra ela.
    pub base_currency: String,
    pub positions_missing_fx: i32,
}

#[tauri::command]
pub async fn get_portfolio_summary(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<PortfolioSummary, AppError> {
    let db = db.inner();
    let (positions, base_currency) = positions_in_base(db, portfolio_id).await?;

    let mut total_market_value = 0.0;
    let mut total_cost_basis = 0.0;
    let mut positions_missing_price = 0;
    let mut positions_missing_fx = 0;
    let mut by_class: HashMap<String, f64> = HashMap::new();

    for position in &positions {
        if position.fx_missing {
            if position.quantity > 0.0 {
                positions_missing_fx += 1;
            }
        } else if let Some(market_value) = position.market_value {
            total_market_value += market_value;
            *by_class.entry(position.asset_class.clone()).or_insert(0.0) += market_value;
        } else if position.quantity > 0.0 {
            positions_missing_price += 1;
        }
        if let Some(avg_price) = position.average_buy_price {
            total_cost_basis += avg_price * position.quantity;
        }
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

    let since = (Utc::now() - Duration::days(365)).format("%Y-%m-%d").to_string();
    let dividend_txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.eq(portfolio_id))
        .filter(transactions::Column::TransactionType.eq(DIVIDEND))
        .filter(transactions::Column::TransactionDate.gte(since))
        .all(db)
        .await?;
    let dividends_received_12m = sum_in_base(db, &dividend_txs, &base_currency).await?;

    Ok(PortfolioSummary {
        base_currency,
        positions_missing_fx,
        total_market_value,
        total_cost_basis,
        unrealized_pl,
        unrealized_pl_pct,
        dividends_received_12m,
        allocation_by_class,
        positions_missing_price,
    })
}
