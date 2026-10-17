// Fase 19 — moeda base por Workspace e conversão de exibição. `compute_positions`
// continua devolvendo tudo na moeda do ativo (patrimônio, rentabilidade e
// impostos dependem disso); quem exibe (posições, resumo, Home) converte aqui
// com o último fechamento do par em `stock_price_history`.

use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Serialize;

use crate::commands::transaction::{compute_positions, PositionView};
use crate::domain::currency_conversion::{average_cost_in_base, FxHistory, FxRates};
use crate::entity::{assets, portfolio, stock_price_history, transactions, workspace_settings};
use crate::error::AppError;
use crate::finance_api;
use crate::finance_core::fx::CURRENCIES;

pub const DEFAULT_BASE_CURRENCY: &str = "BRL";
const BUY: &str = "compra";
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

// Códigos de par que podem servir de rota entre `currencies` e `base`: direto,
// inverso e as duas pernas via USD.
fn candidate_pair_codes(base: &str, currencies: &HashSet<String>) -> HashSet<String> {
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
    codes
}

/// Último fechamento de cada par que pode servir de rota entre `currencies` e
/// `base`.
pub async fn load_rates(
    db: &DatabaseConnection,
    base: &str,
    currencies: &HashSet<String>,
) -> Result<FxRates, AppError> {
    let codes = candidate_pair_codes(base, currencies);
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

/// Série inteira dos pares candidatos, lida de uma vez (a rentabilidade converte
/// dezenas de fluxos e fins de mês, cada um na sua data).
pub async fn load_fx_history(
    db: &DatabaseConnection,
    base: &str,
    currencies: &HashSet<String>,
) -> Result<FxHistory, AppError> {
    let codes = candidate_pair_codes(base, currencies);
    let mut pairs: HashMap<String, Vec<(NaiveDate, f64)>> = HashMap::new();
    if !codes.is_empty() {
        for row in stock_price_history::Entity::find()
            .filter(stock_price_history::Column::Ticker.is_in(codes))
            .all(db)
            .await?
        {
            if let Ok(date) = NaiveDate::parse_from_str(&row.price_date, "%Y-%m-%d") {
                pairs.entry(row.ticker).or_default().push((date, row.close_price));
            }
        }
    }
    Ok(FxHistory::new(pairs))
}

/// Posição na moeda base. Preço e valor de mercado entram pela taxa mais
/// recente; o custo, pela média das compras já convertidas na taxa de cada data
/// (`average_cost_in_base`, ou a taxa mais recente se o histórico não cobre), e
/// o P/L é refeito entre os dois — inclui a variação cambial. Sem taxa: valores
/// em `None`, moeda nativa preservada e `fx_missing` ligado, pra UI avisar e os
/// totais pularem.
pub fn convert_position(
    mut position: PositionView,
    rates: &FxRates,
    average_cost_in_base: Option<f64>,
) -> PositionView {
    let native = position.native_currency.clone();
    let Some(rate) = rates.rate(&native) else {
        position.fx_missing = true;
        position.current_price = None;
        position.market_value = None;
        position.unrealized_pl = None;
        position.unrealized_pl_pct = None;
        position.average_buy_price = None;
        return position;
    };
    position.currency = rates.base().to_string();
    let native_average = position.average_buy_price;
    let native_market_value = position.market_value;
    let average = average_cost_in_base.or(position.average_buy_price.map(|v| v * rate));
    if position.price_source == "avg_buy_price" {
        // Sem cotação, o "preço atual" é o próprio custo: vale o custo na base,
        // senão o câmbio apareceria como lucro de um ativo que ninguém precificou.
        position.current_price = average;
        position.market_value = average.map(|avg| avg * position.quantity);
    } else {
        position.current_price = position.current_price.map(|v| v * rate);
        position.market_value = position.market_value.map(|v| v * rate);
    }
    position.average_buy_price = average;
    let cost_basis = average.map(|avg| avg * position.quantity);
    (position.unrealized_pl, position.unrealized_pl_pct) = match (position.market_value, cost_basis) {
        (Some(market), Some(cost)) => {
            let pl = market - cost;
            (Some(pl), (cost != 0.0).then_some(pl / cost.abs()))
        }
        _ => (None, None),
    };
    // P/L = (mercado − custo) na moeda do ativo × taxa da compra  [preço]
    //     + mercado na moeda do ativo × (taxa de hoje − taxa da compra)  [câmbio]
    // e as duas parcelas somam exatamente o `unrealized_pl` de cima. Só existe
    // quando a moeda do ativo difere da base.
    if native != rates.base() {
        if let (Some(native_avg), Some(native_mv), Some(avg)) = (native_average, native_market_value, average) {
            if native_avg > 0.0 {
                let purchase_rate = avg / native_avg;
                if position.price_source == "avg_buy_price" {
                    // Sem cotação o valor é o próprio custo: nada de preço nem de câmbio.
                    position.unrealized_pl_price = Some(0.0);
                    position.unrealized_pl_fx = Some(0.0);
                } else {
                    position.unrealized_pl_price =
                        Some((native_mv - native_avg * position.quantity) * purchase_rate);
                    position.unrealized_pl_fx = Some(native_mv * (rate - purchase_rate));
                }
            }
        }
    }
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

    // Custo na taxa da data de cada compra; só vale a pena quando há posição em
    // moeda diferente da base (com a mesma moeda a taxa é 1 em qualquer data).
    let mut average_costs: HashMap<i32, f64> = HashMap::new();
    if currencies.iter().any(|c| *c != base) {
        let history = load_fx_history(db, &base, &currencies).await?;
        let mut lots: HashMap<i32, Vec<(NaiveDate, f64, f64)>> = HashMap::new();
        for tx in transactions::Entity::find()
            .filter(transactions::Column::PortfolioId.eq(portfolio_id))
            .filter(transactions::Column::TransactionType.eq(BUY))
            .all(db)
            .await?
        {
            let (Some(asset_id), Ok(date)) =
                (tx.asset_id, NaiveDate::parse_from_str(&tx.transaction_date, "%Y-%m-%d"))
            else {
                continue;
            };
            lots.entry(asset_id).or_default().push((date, tx.total_value, tx.quantity.unwrap_or(0.0)));
        }
        for position in &positions {
            if let Some(asset_lots) = lots.get(&position.asset_id) {
                if let Some(avg) =
                    average_cost_in_base(asset_lots, &position.native_currency, &history, &base)
                {
                    average_costs.insert(position.asset_id, avg);
                }
            }
        }
    }

    Ok((
        positions
            .into_iter()
            .map(|p| {
                let avg = average_costs.get(&p.asset_id).copied();
                convert_position(p, &rates, avg)
            })
            .collect(),
        base,
    ))
}

/// Soma lançamentos de caixa de ativo (proventos) na moeda base, pela moeda do
/// ativo da transação e pela taxa da data de cada um. Lançamento sem ativo é
/// tratado como BRL; sem rota de câmbio, fica de fora da soma.
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
    let history = load_fx_history(db, base, &currencies).await?;
    Ok(txs
        .iter()
        .filter_map(|t| {
            let date = NaiveDate::parse_from_str(&t.transaction_date, "%Y-%m-%d").ok()?;
            history.rate_on(&currency_for(t), base, date).map(|rate| t.total_value * rate)
        })
        .sum())
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

/// Código ISO do catálogo de câmbio, normalizado em maiúsculas.
pub fn validate_currency(code: &str) -> Result<String, AppError> {
    let code = code.trim().to_uppercase();
    if !CURRENCIES.iter().any(|(known, _)| *known == code) {
        return Err(AppError::InvalidInput(format!("unsupported currency '{code}'")));
    }
    Ok(code)
}

/// Base do workspace e as taxas dela pras `currencies` pedidas.
pub async fn rates_for(
    db: &DatabaseConnection,
    workspace_id: i32,
    currencies: &HashSet<String>,
) -> Result<FxRates, AppError> {
    let base = base_currency(db, workspace_id).await?;
    load_rates(db, &base, currencies).await
}

/// Valida e grava a moeda base (upsert da linha única do workspace).
pub async fn save_base_currency(
    db: &DatabaseConnection,
    workspace_id: i32,
    currency: &str,
) -> Result<String, AppError> {
    let currency = validate_currency(currency)?;
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
