// Fase 12 — Patrimônio Líquido/Alavancagem cruzando Portfolio + Finanças
// Gerais. Sem migration nova, tudo derivado on-the-fly (mesmo padrão de
// `BankAccountView.balance`/`PositionView.market_value`/
// `LiabilityView.saldo_devedor_atual`).

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::commands::bank_account::compute_total_cash;
use crate::commands::liability::list_liabilities_view;
use crate::commands::loan::list_loans_view;
use crate::commands::transaction::compute_positions;
use crate::domain::net_worth::{compute_leverage, sum_receivable_in_net_worth};
use crate::entity::portfolio;
use crate::error::AppError;

#[derive(Serialize)]
pub struct NetWorthSummary {
    pub total_market_value: f64,
    pub total_cash: f64,
    pub total_debt: f64,
    /// Saldo a receber dos empréstimos concedidos marcados pra entrar no
    /// patrimônio (Fase 29). Os que ficaram de fora não aparecem aqui.
    pub total_loans_receivable: f64,
    pub net_worth: f64,
    pub leverage_ratio: Option<f64>,
}

#[tauri::command]
pub async fn get_net_worth_summary(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<NetWorthSummary, AppError> {
    compute_net_worth(db.inner(), workspace_id).await
}

pub async fn compute_net_worth(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<NetWorthSummary, AppError> {
    let portfolios = portfolio::Entity::find()
        .filter(portfolio::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?;

    let mut total_market_value = 0.0;
    for p in &portfolios {
        let positions = compute_positions(db, p.id).await?;
        total_market_value += positions.iter().filter_map(|pos| pos.market_value).sum::<f64>();
    }

    let total_cash = compute_total_cash(db, workspace_id).await?;

    let liabilities = list_liabilities_view(db, workspace_id).await?;
    let total_debt: f64 = liabilities.iter().map(|l| l.saldo_devedor_atual).sum();

    let loans = list_loans_view(db, workspace_id).await?;
    let total_loans_receivable = sum_receivable_in_net_worth(
        &loans
            .iter()
            .map(|l| (l.include_in_net_worth, l.saldo_a_receber))
            .collect::<Vec<_>>(),
    );

    let net_worth = total_market_value + total_cash + total_loans_receivable - total_debt;
    let leverage_ratio = compute_leverage(total_debt, net_worth);

    Ok(NetWorthSummary {
        total_market_value,
        total_cash,
        total_debt,
        total_loans_receivable,
        net_worth,
        leverage_ratio,
    })
}
