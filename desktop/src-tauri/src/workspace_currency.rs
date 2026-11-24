// Moeda base do Workspace e moedas dos ativos dele: leituras de banco que a carteira, as
// finanças pessoais e a coleta de dados (`ingestion.rs`) precisam. Moram na raiz, e não em
// `commands/currency.rs`, para a camada de dados não depender de um comando da carteira
// (ADR 0007, passo 2). A conversão de moedas continua em `commands/currency.rs`.

use std::collections::HashSet;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::entity::{assets, portfolio, transactions, workspace_settings};
use crate::error::AppError;

pub const DEFAULT_BASE_CURRENCY: &str = "BRL";

pub async fn base_currency(db: &DatabaseConnection, workspace_id: i32) -> Result<String, AppError> {
    Ok(workspace_settings::Entity::find()
        .filter(workspace_settings::Column::WorkspaceId.eq(workspace_id))
        .one(db)
        .await?
        .map(|row| row.base_currency)
        .unwrap_or_else(|| DEFAULT_BASE_CURRENCY.to_string()))
}

/// Moedas dos ativos com lançamento nas carteiras do workspace.
pub(crate) async fn workspace_asset_currencies(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<HashSet<String>, AppError> {
    let portfolio_ids: Vec<i32> = portfolio::Entity::find()
        .filter(portfolio::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.id)
        .collect();
    if portfolio_ids.is_empty() {
        return Ok(HashSet::new());
    }
    let asset_ids: Vec<i32> = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.is_in(portfolio_ids))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|t| t.asset_id)
        .collect();
    if asset_ids.is_empty() {
        return Ok(HashSet::new());
    }
    Ok(assets::Entity::find()
        .filter(assets::Column::Id.is_in(asset_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|a| a.currency)
        .collect())
}
