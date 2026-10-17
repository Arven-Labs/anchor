use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};

use crate::commands::currency::{
    base_currency, load_fx_history, positions_in_base, workspace_of_portfolio,
};
use crate::domain::position_pricing::{self, PricingResult};
use crate::domain::proventos::{PAYMENT_TYPE_DIVIDENDO, PAYMENT_TYPE_JSCP};
use crate::domain::transaction_ledger::{self, LedgerEntry};
use crate::entity::{asset_valuations, assets, custodia, stock_price_history, transactions};
use crate::error::AppError;

const BUY: &str = "compra";
const SELL: &str = "venda";
const DEPOSIT: &str = "aporte";
const WITHDRAWAL: &str = "retirada";
const DIVIDEND: &str = "provento";
const TRANSFER: &str = "transferencia";
const TRANSACTION_TYPES: [&str; 6] = [BUY, SELL, DEPOSIT, WITHDRAWAL, DIVIDEND, TRANSFER];

#[derive(Deserialize)]
pub struct CreateTransactionRequest {
    pub portfolio_id: i32,
    pub asset_id: Option<i32>,
    pub custodia_id: Option<i32>,
    pub transfer_to_custodia_id: Option<i32>,
    pub transaction_type: String,
    pub payment_type: Option<String>,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub total_value: f64,
    pub fee: Option<f64>,
    pub transaction_date: String,
    pub notes: Option<String>,
    pub fi_emissor: Option<String>,
    pub fi_indexador: Option<String>,
    pub fi_taxa_percentual: Option<f64>,
    pub fi_data_vencimento: Option<String>,
    pub fi_liquidez: Option<String>,
}

// Cross-field validation gating (não uma conta pura, mesmo espírito de
// `create_alert_rule`'s guard por `target_type`) — cada tipo de lançamento
// tem um conjunto diferente de campos obrigatórios (ver PHASE.md item 9 e
// a modelagem da Sessão 33 pra `transactions`).
fn validate(request: &CreateTransactionRequest) -> Result<(), AppError> {
    if !TRANSACTION_TYPES.contains(&request.transaction_type.as_str()) {
        return Err(AppError::InvalidGuard(format!(
            "transaction_type '{}' inválido (esperado um de {TRANSACTION_TYPES:?})",
            request.transaction_type
        )));
    }

    match request.transaction_type.as_str() {
        BUY | SELL => {
            if request.asset_id.is_none() {
                return Err(AppError::InvalidGuard(format!(
                    "asset_id é obrigatório para '{}'",
                    request.transaction_type
                )));
            }
            if request.quantity.is_none() || request.unit_price.is_none() {
                return Err(AppError::InvalidGuard(format!(
                    "quantity e unit_price são obrigatórios para '{}'",
                    request.transaction_type
                )));
            }
        }
        DEPOSIT | WITHDRAWAL => {
            if request.asset_id.is_some() {
                return Err(AppError::InvalidGuard(format!(
                    "'{}' é fluxo de caixa puro, não referencia um ativo",
                    request.transaction_type
                )));
            }
        }
        DIVIDEND => {
            if request.asset_id.is_none() {
                return Err(AppError::InvalidGuard(
                    "asset_id é obrigatório para 'provento' (qual ativo pagou)".to_string(),
                ));
            }
            if let Some(payment_type) = &request.payment_type {
                if payment_type != PAYMENT_TYPE_DIVIDENDO && payment_type != PAYMENT_TYPE_JSCP {
                    return Err(AppError::InvalidGuard(format!(
                        "payment_type '{payment_type}' inválido (esperado '{PAYMENT_TYPE_DIVIDENDO}' ou '{PAYMENT_TYPE_JSCP}')"
                    )));
                }
            }
        }
        TRANSFER => {
            if request.asset_id.is_none() || request.quantity.is_none() {
                return Err(AppError::InvalidGuard(
                    "asset_id e quantity são obrigatórios para 'transferencia'".to_string(),
                ));
            }
            match (request.custodia_id, request.transfer_to_custodia_id) {
                (Some(from), Some(to)) if from == to => {
                    return Err(AppError::InvalidGuard(
                        "origem e destino da transferência não podem ser a mesma custódia"
                            .to_string(),
                    ));
                }
                (Some(_), Some(_)) => {}
                _ => {
                    return Err(AppError::InvalidGuard(
                        "transferência exige custodia_id (origem) e transfer_to_custodia_id (destino)"
                            .to_string(),
                    ));
                }
            }
        }
        _ => unreachable!("já validado pelo contains acima"),
    }

    Ok(())
}

#[tauri::command]
pub async fn create_transaction(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateTransactionRequest,
) -> Result<transactions::Model, AppError> {
    validate(&request)?;

    if let Some(asset_id) = request.asset_id {
        assets::Entity::find_by_id(asset_id)
            .one(db.inner())
            .await?
            .ok_or_else(|| AppError::NotFound(format!("asset {asset_id}")))?;
    }

    let tx = transactions::ActiveModel {
        portfolio_id: Set(request.portfolio_id),
        asset_id: Set(request.asset_id),
        custodia_id: Set(request.custodia_id),
        transfer_to_custodia_id: Set(request.transfer_to_custodia_id),
        transaction_type: Set(request.transaction_type),
        payment_type: Set(request.payment_type),
        quantity: Set(request.quantity),
        unit_price: Set(request.unit_price),
        total_value: Set(request.total_value),
        fee: Set(request.fee),
        transaction_date: Set(request.transaction_date),
        notes: Set(request.notes),
        fi_emissor: Set(request.fi_emissor),
        fi_indexador: Set(request.fi_indexador),
        fi_taxa_percentual: Set(request.fi_taxa_percentual),
        fi_data_vencimento: Set(request.fi_data_vencimento),
        fi_liquidez: Set(request.fi_liquidez),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?;

    Ok(tx)
}

#[tauri::command]
pub async fn delete_transaction(
    db: tauri::State<'_, DatabaseConnection>,
    transaction_id: i32,
) -> Result<(), AppError> {
    transactions::Entity::delete_by_id(transaction_id)
        .exec(db.inner())
        .await?;

    Ok(())
}

fn custodia_label(custodias: &HashMap<i32, custodia::Model>, id: Option<i32>) -> Option<String> {
    id.and_then(|id| custodias.get(&id))
        .map(|c| format!("{} ({})", c.instituicao, c.titular))
}

#[derive(Serialize)]
pub struct TransactionView {
    pub id: i32,
    pub asset_id: Option<i32>,
    pub ticker: Option<String>,
    pub asset_class: Option<String>,
    pub custodia_id: Option<i32>,
    pub custodia_label: Option<String>,
    pub transfer_to_custodia_id: Option<i32>,
    pub transfer_to_custodia_label: Option<String>,
    // Fase 19 — moeda do `total_value` (a do ativo; aporte/retirada, sem ativo,
    // valem na moeda base) e o valor na moeda base pela taxa da data do
    // lançamento (`None` sem rota de câmbio).
    pub currency: String,
    pub total_value_in_base: Option<f64>,
    pub base_currency: String,
    pub transaction_type: String,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub total_value: f64,
    pub fee: Option<f64>,
    pub transaction_date: String,
    pub notes: Option<String>,
    pub fi_emissor: Option<String>,
    pub fi_indexador: Option<String>,
    pub fi_taxa_percentual: Option<f64>,
    pub fi_data_vencimento: Option<String>,
    pub fi_liquidez: Option<String>,
    pub created_at: String,
    // Fase 13.2 — quantidade líquida do ativo imediatamente após este
    // lançamento (`domain::transaction_ledger::running_quantities`). `None`
    // pra lançamentos sem `asset_id` (aporte/retirada).
    pub running_quantity: Option<f64>,
}

#[tauri::command]
pub async fn list_transactions(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<Vec<TransactionView>, AppError> {
    list_transactions_in(db.inner(), portfolio_id).await
}

pub async fn list_transactions_in(
    db: &DatabaseConnection,
    portfolio_id: i32,
) -> Result<Vec<TransactionView>, AppError> {
    let txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.eq(portfolio_id))
        .order_by_desc(transactions::Column::TransactionDate)
        .all(db)
        .await?;

    let asset_ids: Vec<i32> = txs.iter().filter_map(|t| t.asset_id).collect();
    let assets_map: HashMap<i32, assets::Model> = if asset_ids.is_empty() {
        HashMap::new()
    } else {
        assets::Entity::find()
            .filter(assets::Column::Id.is_in(asset_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|a| (a.id, a))
            .collect()
    };

    let custodia_ids: Vec<i32> = txs
        .iter()
        .filter_map(|t| t.custodia_id)
        .chain(txs.iter().filter_map(|t| t.transfer_to_custodia_id))
        .collect();
    let custodias_map: HashMap<i32, custodia::Model> = if custodia_ids.is_empty() {
        HashMap::new()
    } else {
        custodia::Entity::find()
            .filter(custodia::Column::Id.is_in(custodia_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|c| (c.id, c))
            .collect()
    };

    let base = base_currency(db, workspace_of_portfolio(db, portfolio_id).await?).await?;
    let currencies: HashSet<String> =
        assets_map.values().map(|a| a.currency.clone()).chain([base.clone()]).collect();
    let history = load_fx_history(db, &base, &currencies).await?;

    let ledger_entries: Vec<LedgerEntry> = txs
        .iter()
        .filter_map(|tx| {
            tx.asset_id.map(|asset_id| LedgerEntry {
                id: tx.id,
                asset_id,
                transaction_type: tx.transaction_type.clone(),
                quantity: tx.quantity.unwrap_or(0.0),
                transaction_date: tx.transaction_date.clone(),
            })
        })
        .collect();
    let running_quantities = transaction_ledger::running_quantities(&ledger_entries);

    let views = txs
        .into_iter()
        .map(|tx| {
            let currency = tx
                .asset_id
                .and_then(|id| assets_map.get(&id))
                .map_or_else(|| base.clone(), |a| a.currency.clone());
            let total_value_in_base = NaiveDate::parse_from_str(&tx.transaction_date, "%Y-%m-%d")
                .ok()
                .and_then(|date| history.rate_on(&currency, &base, date))
                .map(|rate| tx.total_value * rate);
            (tx, currency, total_value_in_base)
        })
        .map(|(tx, currency, total_value_in_base)| TransactionView {
            currency,
            total_value_in_base,
            base_currency: base.clone(),
            ticker: tx.asset_id.and_then(|id| assets_map.get(&id)).map(|a| a.ticker.clone()),
            asset_class: tx.asset_id.and_then(|id| assets_map.get(&id)).map(|a| a.asset_class.clone()),
            custodia_label: custodia_label(&custodias_map, tx.custodia_id),
            transfer_to_custodia_label: custodia_label(&custodias_map, tx.transfer_to_custodia_id),
            running_quantity: running_quantities.get(&tx.id).copied(),
            id: tx.id,
            asset_id: tx.asset_id,
            custodia_id: tx.custodia_id,
            transfer_to_custodia_id: tx.transfer_to_custodia_id,
            transaction_type: tx.transaction_type,
            quantity: tx.quantity,
            unit_price: tx.unit_price,
            total_value: tx.total_value,
            fee: tx.fee,
            transaction_date: tx.transaction_date,
            notes: tx.notes,
            fi_emissor: tx.fi_emissor,
            fi_indexador: tx.fi_indexador,
            fi_taxa_percentual: tx.fi_taxa_percentual,
            fi_data_vencimento: tx.fi_data_vencimento,
            fi_liquidez: tx.fi_liquidez,
            created_at: tx.created_at,
        })
        .collect();

    Ok(views)
}

#[derive(Serialize)]
pub struct CustodiaBreakdown {
    pub custodia_id: Option<i32>,
    pub custodia_label: Option<String>,
    pub quantity: f64,
}

// MVP deliberadamente simples (decisão implícita da Sessão 33, escopo desta
// fatia): preço médio é a média das compras, sem redução proporcional do
// custo por venda (FIFO/custo médio "de verdade" fica pra quando a Fase
// 10.3 — rentabilidade histórica — for desenhada). `provento` não move
// quantidade (é caixa recebido, não ativo). `transferencia` move quantidade
// entre custódias sem alterar o total líquido do ativo no portfolio.
#[derive(Serialize)]
pub struct PositionView {
    pub asset_id: i32,
    pub ticker: String,
    pub name: String,
    pub asset_class: String,
    // Moeda em que os valores abaixo estão. `compute_positions` devolve a do
    // ativo; `commands::currency::positions_in_base` converte pra moeda base
    // do Workspace (Fase 19) e passa a ser ela.
    pub currency: String,
    pub native_currency: String,
    // Fase 19 — sem cotação do par pra moeda base: valores vêm `None` e a
    // posição fica fora dos totais.
    pub fx_missing: bool,
    // Fase 19 — só com moeda do ativo ≠ base: quanto do `unrealized_pl` veio da
    // variação do preço e quanto da variação do câmbio desde a compra.
    pub unrealized_pl_price: Option<f64>,
    pub unrealized_pl_fx: Option<f64>,
    pub quantity: f64,
    pub average_buy_price: Option<f64>,
    pub by_custodia: Vec<CustodiaBreakdown>,
    // Fase 13.1 — preço/valor de mercado calculado por
    // `domain::position_pricing::price_position`, ver ali as regras por
    // classe de ativo (cotação automática / reavaliação manual / fallback
    // pro preço médio de compra).
    pub current_price: Option<f64>,
    pub market_value: Option<f64>,
    pub unrealized_pl: Option<f64>,
    pub unrealized_pl_pct: Option<f64>,
    pub price_source: String,
}

#[tauri::command]
pub async fn get_portfolio_positions(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<Vec<PositionView>, AppError> {
    Ok(positions_in_base(db.inner(), portfolio_id).await?.0)
}

// Extraída de `get_portfolio_positions` (comando Tauri) pra ser reaproveitada
// por `commands::portfolio_summary::get_portfolio_summary` (Fase 13.1) sem
// duplicar a reconstrução de posição.
pub async fn compute_positions(
    db: &DatabaseConnection,
    portfolio_id: i32,
) -> Result<Vec<PositionView>, AppError> {
    let txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.eq(portfolio_id))
        .all(db)
        .await?;

    let mut by_asset: HashMap<i32, Vec<&transactions::Model>> = HashMap::new();
    for tx in &txs {
        if let Some(asset_id) = tx.asset_id {
            by_asset.entry(asset_id).or_default().push(tx);
        }
    }

    if by_asset.is_empty() {
        return Ok(vec![]);
    }

    let asset_ids: Vec<i32> = by_asset.keys().copied().collect();
    let assets_map: HashMap<i32, assets::Model> = assets::Entity::find()
        .filter(assets::Column::Id.is_in(asset_ids.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();

    let custodia_ids: Vec<i32> = txs
        .iter()
        .filter_map(|t| t.custodia_id)
        .chain(txs.iter().filter_map(|t| t.transfer_to_custodia_id))
        .collect();
    let custodias_map: HashMap<i32, custodia::Model> = if custodia_ids.is_empty() {
        HashMap::new()
    } else {
        custodia::Entity::find()
            .filter(custodia::Column::Id.is_in(custodia_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|c| (c.id, c))
            .collect()
    };

    // Fase 13.1 — preço mais recente por ticker (`MAX(price_date)`, mesma
    // tabela que `commands/profitability.rs::closest_price` já usa) e última
    // reavaliação manual por ativo (`asset_valuations`), pra alimentar
    // `domain::position_pricing::price_position` linha a linha abaixo.
    let tickers: Vec<String> = assets_map.values().map(|a| a.ticker.clone()).collect();
    let latest_ticker_price: HashMap<String, f64> = if tickers.is_empty() {
        HashMap::new()
    } else {
        let mut by_ticker: HashMap<String, (String, f64)> = HashMap::new();
        for row in stock_price_history::Entity::find()
            .filter(stock_price_history::Column::Ticker.is_in(tickers))
            .all(db)
            .await?
        {
            let entry = by_ticker
                .entry(row.ticker.clone())
                .or_insert((row.price_date.clone(), row.close_price));
            if row.price_date > entry.0 {
                *entry = (row.price_date, row.close_price);
            }
        }
        by_ticker.into_iter().map(|(k, (_, price))| (k, price)).collect()
    };

    let latest_manual_valuation: HashMap<i32, f64> = {
        let mut by_asset_id: HashMap<i32, (String, f64)> = HashMap::new();
        for row in asset_valuations::Entity::find()
            .filter(asset_valuations::Column::AssetId.is_in(asset_ids))
            .all(db)
            .await?
        {
            let entry = by_asset_id
                .entry(row.asset_id)
                .or_insert((row.valuation_date.clone(), row.value));
            if row.valuation_date > entry.0 {
                *entry = (row.valuation_date, row.value);
            }
        }
        by_asset_id.into_iter().map(|(k, (_, value))| (k, value)).collect()
    };

    let mut positions = Vec::new();
    for (asset_id, asset_txs) in by_asset {
        let Some(asset) = assets_map.get(&asset_id) else {
            continue;
        };

        let mut net_qty = 0.0;
        let mut buy_qty_sum = 0.0;
        let mut buy_value_sum = 0.0;
        let mut custodia_qty: HashMap<Option<i32>, f64> = HashMap::new();

        for tx in asset_txs {
            match tx.transaction_type.as_str() {
                BUY => {
                    let qty = tx.quantity.unwrap_or(0.0);
                    net_qty += qty;
                    buy_qty_sum += qty;
                    buy_value_sum += tx.total_value;
                    *custodia_qty.entry(tx.custodia_id).or_insert(0.0) += qty;
                }
                SELL => {
                    let qty = tx.quantity.unwrap_or(0.0);
                    net_qty -= qty;
                    *custodia_qty.entry(tx.custodia_id).or_insert(0.0) -= qty;
                }
                TRANSFER => {
                    let qty = tx.quantity.unwrap_or(0.0);
                    *custodia_qty.entry(tx.custodia_id).or_insert(0.0) -= qty;
                    *custodia_qty.entry(tx.transfer_to_custodia_id).or_insert(0.0) += qty;
                }
                _ => {}
            }
        }

        let average_buy_price = (buy_qty_sum > 0.0).then_some(buy_value_sum / buy_qty_sum);

        let by_custodia = custodia_qty
            .into_iter()
            .map(|(cid, quantity)| CustodiaBreakdown {
                custodia_id: cid,
                custodia_label: custodia_label(&custodias_map, cid),
                quantity,
            })
            .collect();

        let PricingResult {
            current_price,
            market_value,
            unrealized_pl,
            unrealized_pl_pct,
            price_source,
        } = position_pricing::price_position(
            &asset.asset_class,
            net_qty,
            average_buy_price,
            latest_ticker_price.get(&asset.ticker).copied(),
            latest_manual_valuation.get(&asset_id).copied(),
        );

        positions.push(PositionView {
            asset_id,
            ticker: asset.ticker.clone(),
            name: asset.name.clone(),
            asset_class: asset.asset_class.clone(),
            currency: asset.currency.clone(),
            native_currency: asset.currency.clone(),
            fx_missing: false,
            unrealized_pl_price: None,
            unrealized_pl_fx: None,
            quantity: net_qty,
            average_buy_price,
            by_custodia,
            current_price,
            market_value,
            unrealized_pl,
            unrealized_pl_pct,
            price_source,
        });
    }

    positions.sort_by(|a, b| a.ticker.cmp(&b.ticker));
    Ok(positions)
}
