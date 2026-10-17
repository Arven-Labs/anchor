// Fase 19 — moeda base por Workspace e conversão de exibição. `compute_positions`
// continua devolvendo tudo na moeda do ativo (patrimônio, rentabilidade e
// impostos dependem disso); quem exibe (posições, resumo, Home) converte aqui
// com o último fechamento do par em `stock_price_history`.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Serialize;

use crate::commands::transaction::{compute_positions, PositionView};
use crate::domain::currency_conversion::FxRates;
use crate::entity::{assets, portfolio, stock_price_history, transactions, workspace_settings};
use crate::error::AppError;
use crate::finance_api;
use crate::finance_core::fx::CURRENCIES;

pub const DEFAULT_BASE_CURRENCY: &str = "BRL";
const PIVOT: &str = "USD";

pub async fn base_currency(db: &DatabaseConnection, workspace_id: i32) -> Result<String, AppError> {
    Ok(workspace_settings::Entity::find()
        .filter(workspace_settings::Column::WorkspaceId.eq(workspace_id))
        .one(db)
        .await?
        .map(|row| row.base_currency)
        .unwrap_or_else(|| DEFAULT_BASE_CURRENCY.to_string()))
}

pub async fn workspace_of_portfolio(db: &DatabaseConnection, portfolio_id: i32) -> Result<i32, AppError> {
    portfolio::Entity::find_by_id(portfolio_id)
        .one(db)
        .await?
        .map(|p| p.workspace_id)
        .ok_or_else(|| AppError::NotFound(format!("portfolio {portfolio_id}")))
}

/// Último fechamento de cada par que pode servir de rota entre `currencies` e
/// `base` (direto, inverso e as duas pernas via USD).
pub async fn load_rates(
    db: &DatabaseConnection,
    base: &str,
    currencies: &HashSet<String>,
) -> Result<FxRates, AppError> {
    let mut codes: HashSet<String> = HashSet::new();
    for currency in currencies.iter().filter(|c| c.as_str() != base) {
        for (a, b) in [(currency.as_str(), base), (base, currency.as_str()), (currency.as_str(), PIVOT), (PIVOT, currency.as_str())] {
            if a != b {
                codes.insert(format!("{a}{b}"));
            }
        }
    }
    if !currencies.is_empty() {
        codes.insert(format!("{PIVOT}{base}"));
        codes.insert(format!("{base}{PIVOT}"));
    }

    let mut latest: HashMap<String, (String, f64)> = HashMap::new();
    if !codes.is_empty() {
        for row in stock_price_history::Entity::find()
            .filter(stock_price_history::Column::Ticker.is_in(codes))
            .all(db)
            .await?
        {
            let entry = latest.entry(row.ticker.clone()).or_insert((row.price_date.clone(), row.close_price));
            if row.price_date > entry.0 {
                *entry = (row.price_date, row.close_price);
            }
        }
    }
    Ok(FxRates::new(base, latest.into_iter().map(|(code, (_, price))| (code, price)).collect()))
}

/// Posição na moeda base. Sem taxa: valores zerados em `None`, moeda nativa
/// preservada e `fx_missing` ligado, pra UI avisar e os totais pularem.
pub fn convert_position(mut position: PositionView, rates: &FxRates) -> PositionView {
    let native = position.native_currency.clone();
    let Some(rate) = rates.rate(&native) else {
        position.fx_missing = true;
        position.current_price = None;
        position.market_value = None;
        position.unrealized_pl = None;
        position.average_buy_price = None;
        return position;
    };
    position.currency = rates.base().to_string();
    position.current_price = position.current_price.map(|v| v * rate);
    position.market_value = position.market_value.map(|v| v * rate);
    position.unrealized_pl = position.unrealized_pl.map(|v| v * rate);
    position.average_buy_price = position.average_buy_price.map(|v| v * rate);
    position
}

/// Posições da carteira já na moeda base do Workspace dela; devolve também a
/// base, que os resumos usam pra rotular os totais.
pub async fn positions_in_base(
    db: &DatabaseConnection,
    portfolio_id: i32,
) -> Result<(Vec<PositionView>, String), AppError> {
    let base = base_currency(db, workspace_of_portfolio(db, portfolio_id).await?).await?;
    let positions = compute_positions(db, portfolio_id).await?;
    let currencies: HashSet<String> = positions.iter().map(|p| p.native_currency.clone()).collect();
    let rates = load_rates(db, &base, &currencies).await?;
    Ok((positions.into_iter().map(|p| convert_position(p, &rates)).collect(), base))
}

/// Soma lançamentos de caixa de ativo (proventos) na moeda base, pela moeda do
/// ativo da transação. Lançamento sem ativo é tratado como BRL; sem taxa, fica
/// de fora da soma.
pub async fn sum_in_base(
    db: &DatabaseConnection,
    txs: &[transactions::Model],
    base: &str,
) -> Result<f64, AppError> {
    let asset_ids: Vec<i32> = txs.iter().filter_map(|t| t.asset_id).collect();
    let currency_of: HashMap<i32, String> = if asset_ids.is_empty() {
        HashMap::new()
    } else {
        assets::Entity::find()
            .filter(assets::Column::Id.is_in(asset_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|a| (a.id, a.currency))
            .collect()
    };
    let currency_for = |t: &transactions::Model| {
        t.asset_id
            .and_then(|id| currency_of.get(&id).cloned())
            .unwrap_or_else(|| DEFAULT_BASE_CURRENCY.to_string())
    };
    let currencies: HashSet<String> = txs.iter().map(currency_for).collect();
    let rates = load_rates(db, base, &currencies).await?;
    Ok(txs.iter().filter_map(|t| rates.convert(t.total_value, &currency_for(t))).sum())
}

#[derive(Serialize)]
pub struct WorkspaceSettingsView {
    pub base_currency: String,
}

#[derive(Serialize)]
pub struct SetBaseCurrencyResult {
    pub base_currency: String,
    /// Pares que faltavam e não foram coletados (sem rede, ativo fora do
    /// catálogo de câmbio); os ativos dessas moedas ficam sem valor até o
    /// usuário coletar o par na aba de câmbio.
    pub missing_pairs: Vec<String>,
}

#[tauri::command]
pub async fn get_workspace_settings(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<WorkspaceSettingsView, AppError> {
    Ok(WorkspaceSettingsView { base_currency: base_currency(db.inner(), workspace_id).await? })
}

/// Valida e grava a moeda base (upsert da linha única do workspace).
pub async fn save_base_currency(
    db: &DatabaseConnection,
    workspace_id: i32,
    currency: &str,
) -> Result<String, AppError> {
    let currency = currency.trim().to_uppercase();
    if !CURRENCIES.iter().any(|(code, _)| *code == currency) {
        return Err(AppError::InvalidInput(format!("unsupported base currency '{currency}'")));
    }
    let now = Utc::now().to_rfc3339();
    let existing = workspace_settings::Entity::find()
        .filter(workspace_settings::Column::WorkspaceId.eq(workspace_id))
        .one(db)
        .await?;
    match existing {
        Some(row) => {
            let mut active: workspace_settings::ActiveModel = row.into();
            active.base_currency = Set(currency.clone());
            active.updated_at = Set(now);
            active.update(db).await?;
        }
        None => {
            workspace_settings::ActiveModel {
                workspace_id: Set(workspace_id),
                base_currency: Set(currency.clone()),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }
    Ok(currency)
}

/// Moedas dos ativos com lançamento nas carteiras do workspace.
async fn workspace_asset_currencies(
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

#[tauri::command]
pub async fn set_base_currency(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
    currency: String,
) -> Result<SetBaseCurrencyResult, AppError> {
    let db = db.inner();
    let base = save_base_currency(db, workspace_id, &currency).await?;

    // Coleta o que faltar pra converter as moedas dos ativos; falha de rede
    // não desfaz a troca, só volta na lista pra UI avisar.
    let currencies = workspace_asset_currencies(db, workspace_id).await?;
    let rates = load_rates(db, &base, &currencies).await?;
    let mut missing_pairs = Vec::new();
    for currency in currencies.iter().filter(|c| **c != base && rates.rate(c).is_none()) {
        if finance_api::fx::collect_pair(db, currency, &base).await.is_err() {
            missing_pairs.push(format!("{currency}/{base}"));
        }
    }
    missing_pairs.sort();
    Ok(SetBaseCurrencyResult { base_currency: base, missing_pairs })
}
