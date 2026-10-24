use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::commands::currency::{
    base_currency, load_fx_history, positions_in_base, workspace_of_portfolio,
};
use crate::domain::cost_basis::{self, EventKind};
use crate::domain::position_pricing::{self, PricingResult};
use crate::domain::proventos::{
    PAYMENT_TYPE_ALUGUEL, PAYMENT_TYPE_DIVIDENDO, PAYMENT_TYPE_JSCP, PAYMENT_TYPE_REEMBOLSO,
};
use crate::domain::transaction_ledger::{self, LedgerEntry};
use crate::entity::{
    asset_valuations, assets, custodia, fund_quota_value, stock_adjusted_close, stock_price_history, transactions,
};
use crate::error::AppError;

const BUY: &str = "compra";
const SELL: &str = "venda";
const DEPOSIT: &str = "aporte";
const WITHDRAWAL: &str = "retirada";
const DIVIDEND: &str = "provento";
const TRANSFER: &str = "transferencia";
// Fase 34 — eventos corporativos. `quantity` é a razão (desdobramento 2 = 1→2;
// grupamento 10 = 10→1) ou, na bonificação, as ações novas (com `total_value`
// como custo atribuído a elas).
pub const SPLIT: &str = "desdobramento";
pub const REVERSE_SPLIT: &str = "grupamento";
pub const BONUS: &str = "bonificacao";
// Come-cotas de fundo aberto: IR retido pelo fundo em cotas. `quantity` = cotas retiradas, `unit_price` = valor da cota na
// data, `total_value` = quantity × unit_price (o imposto em reais). Sem caixa. Para o custo médio é uma venda com
// proceeds zero (reduz a quantidade ao custo médio e consome o custo); só vale para a classe `fundo`.
pub const COME_COTAS: &str = "come_cotas";
// Amortização de fundo fechado (Lei 14.754): o fundo devolve capital e as cotas ficam. `unit_price` = valor da cota na data,
// `total_value` = valor recebido; sem quantidade e sem taxa. Sem caixa. Para o custo médio reduz o custo na fração
// `valor ÷ (cota × cotas em carteira)` sem tirar cotas; só vale para a classe `fundo`.
pub const AMORTIZATION: &str = "amortizacao";
const FUND_CLASS: &str = "fundo";
const TRANSACTION_TYPES: [&str; 11] =
    [BUY, SELL, DEPOSIT, WITHDRAWAL, DIVIDEND, TRANSFER, SPLIT, REVERSE_SPLIT, BONUS, COME_COTAS, AMORTIZATION];

/// Fator multiplicativo da quantidade de um desdobramento (`quantity` = 2 → ×2)
/// ou grupamento (`quantity` = 10 → ×0,1).
pub fn split_ratio(transaction_type: &str, quantity: f64) -> f64 {
    if transaction_type == REVERSE_SPLIT && quantity > 0.0 {
        1.0 / quantity
    } else {
        quantity
    }
}

/// Lançamentos do ativo como eventos do motor de custo médio. Só entram os
/// tipos que mexem na posição; data inválida é erro (não some em silêncio).
pub fn cost_events(txs: &[&transactions::Model]) -> Result<Vec<cost_basis::Event>, AppError> {
    let mut events = Vec::new();
    for tx in txs {
        let kind = match tx.transaction_type.as_str() {
            BUY => EventKind::Buy,
            // Come-cotas: venda sem proceeds (abaixo, `value` vai a zero).
            SELL | COME_COTAS => EventKind::Sell,
            TRANSFER => EventKind::Transfer,
            SPLIT | REVERSE_SPLIT => EventKind::Split,
            BONUS => EventKind::Bonus,
            AMORTIZATION => EventKind::Amortize,
            _ => continue,
        };
        let date = NaiveDate::parse_from_str(&tx.transaction_date, "%Y-%m-%d").map_err(|_| {
            AppError::InvalidGuard(format!("data de transação inválida: '{}'", tx.transaction_date))
        })?;
        let quantity = tx.quantity.unwrap_or(0.0);
        events.push(cost_basis::Event {
            date,
            id: tx.id,
            kind,
            // Na amortização `quantity` leva o valor da cota do dia (ver `EventKind::Amortize`).
            quantity: match kind {
                EventKind::Split => split_ratio(&tx.transaction_type, quantity),
                EventKind::Amortize => tx.unit_price.unwrap_or(0.0),
                _ => quantity,
            },
            value: if tx.transaction_type == COME_COTAS { 0.0 } else { tx.total_value },
            fee: tx.fee.unwrap_or(0.0),
            custodia_id: tx.custodia_id,
            to_custodia_id: tx.transfer_to_custodia_id,
        });
    }
    Ok(events)
}

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
    /// Imposto retido no exterior sobre o provento, na moeda do ativo (crédito da Lei 14.754). Só em `provento`.
    pub foreign_tax_withheld: Option<f64>,
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

    // O chrono aceita ano com menos de 4 dígitos ("05-01-26" vira o ano 5), então
    // o tamanho e um ano plausível são conferidos à parte.
    let valid_date = request.transaction_date.len() == 10
        && NaiveDate::parse_from_str(&request.transaction_date, "%Y-%m-%d").is_ok_and(|d| d.year() >= 1900);
    if !valid_date {
        return Err(AppError::InvalidGuard(format!(
            "transaction_date '{}' inválida (esperado AAAA-MM-DD)",
            request.transaction_date
        )));
    }
    if !request.total_value.is_finite() || request.total_value < 0.0 {
        return Err(AppError::InvalidGuard("total_value não pode ser negativo".to_string()));
    }
    if request.fee.is_some_and(|fee| !fee.is_finite() || fee < 0.0) {
        return Err(AppError::InvalidGuard("fee não pode ser negativa".to_string()));
    }
    if let Some(withheld) = request.foreign_tax_withheld {
        if !withheld.is_finite() || withheld < 0.0 {
            return Err(AppError::InvalidGuard("foreign_tax_withheld não pode ser negativo".to_string()));
        }
        if request.transaction_type != DIVIDEND {
            return Err(AppError::InvalidGuard("foreign_tax_withheld só vale para 'provento'".to_string()));
        }
    }
    if request.quantity.is_some_and(|qty| !qty.is_finite() || qty <= 0.0) {
        return Err(AppError::InvalidGuard("quantity deve ser maior que zero".to_string()));
    }

    match request.transaction_type.as_str() {
        BUY | SELL | COME_COTAS => {
            if request.asset_id.is_none() {
                return Err(AppError::InvalidGuard(format!(
                    "asset_id é obrigatório para '{}'",
                    request.transaction_type
                )));
            }
            let (Some(quantity), Some(unit_price)) = (request.quantity, request.unit_price) else {
                return Err(AppError::InvalidGuard(format!(
                    "quantity e unit_price são obrigatórios para '{}'",
                    request.transaction_type
                )));
            };
            if !unit_price.is_finite() || unit_price < 0.0 {
                return Err(AppError::InvalidGuard("unit_price não pode ser negativo".to_string()));
            }
            if request.transaction_type == COME_COTAS && request.fee.is_some() {
                return Err(AppError::InvalidGuard("come_cotas não tem taxa".to_string()));
            }
            // total_value = quantidade × preço (sem taxas); tolera arredondamento de centavos.
            let expected = quantity * unit_price;
            if (request.total_value - expected).abs() > 0.011_f64.max(expected.abs() * 0.001) {
                return Err(AppError::InvalidGuard(format!(
                    "total_value ({:.2}) não bate com quantity × unit_price ({expected:.2})",
                    request.total_value
                )));
            }
        }
        AMORTIZATION => {
            if request.asset_id.is_none() {
                return Err(AppError::InvalidGuard("asset_id é obrigatório para 'amortizacao'".to_string()));
            }
            if request.quantity.is_some() {
                return Err(AppError::InvalidGuard(
                    "amortizacao não tem quantidade: as cotas ficam, informe o valor recebido e a cota do dia".to_string(),
                ));
            }
            if request.fee.is_some() {
                return Err(AppError::InvalidGuard("amortizacao não tem taxa".to_string()));
            }
            if !request.unit_price.is_some_and(|price| price.is_finite() && price > 0.0) {
                return Err(AppError::InvalidGuard(
                    "unit_price (o valor da cota na data) é obrigatório e maior que zero para 'amortizacao'".to_string(),
                ));
            }
            if request.total_value <= 0.0 {
                return Err(AppError::InvalidGuard("o valor recebido na amortizacao deve ser maior que zero".to_string()));
            }
        }
        SPLIT | REVERSE_SPLIT | BONUS => {
            if request.asset_id.is_none() || request.quantity.is_none() {
                return Err(AppError::InvalidGuard(format!(
                    "asset_id e quantity são obrigatórios para '{}'",
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
                if ![PAYMENT_TYPE_DIVIDENDO, PAYMENT_TYPE_JSCP, PAYMENT_TYPE_ALUGUEL, PAYMENT_TYPE_REEMBOLSO]
                    .contains(&payment_type.as_str())
                {
                    return Err(AppError::InvalidGuard(format!(
                        "payment_type '{payment_type}' inválido (esperado '{PAYMENT_TYPE_DIVIDENDO}', '{PAYMENT_TYPE_JSCP}', '{PAYMENT_TYPE_ALUGUEL}' ou '{PAYMENT_TYPE_REEMBOLSO}')"
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
    create_transaction_in(db.inner(), request).await
}

/// Valida e grava um lançamento. Fora do comando Tauri pra servir também à
/// importação em lote (que passa uma conexão dentro de uma transação).
pub async fn create_transaction_in<C: ConnectionTrait>(
    db: &C,
    request: CreateTransactionRequest,
) -> Result<transactions::Model, AppError> {
    validate(&request)?;

    let asset = match request.asset_id {
        Some(asset_id) => Some(
            assets::Entity::find_by_id(asset_id)
                .one(db)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("asset {asset_id}")))?,
        ),
        None => None,
    };
    if request.transaction_type == COME_COTAS && asset.as_ref().map(|a| a.asset_class.as_str()) != Some(FUND_CLASS) {
        return Err(AppError::InvalidGuard("come_cotas só vale para ativo da classe 'fundo'".to_string()));
    }
    if request.transaction_type == AMORTIZATION {
        if asset.as_ref().map(|a| a.asset_class.as_str()) != Some(FUND_CLASS) {
            return Err(AppError::InvalidGuard("amortizacao só vale para ativo da classe 'fundo'".to_string()));
        }
        ensure_amortization_fits(db, &request).await?;
    }
    if request.transaction_type == SELL || request.transaction_type == COME_COTAS {
        ensure_covered_sale(db, &request).await?;
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
        foreign_tax_withheld: Set(request.foreign_tax_withheld),
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
    .insert(db)
    .await?;

    Ok(tx)
}

const CRYPTO_CLASS: &str = "cripto";

// Permuta cripto↔cripto: para a Receita é a alienação da moeda entregue pelo valor de mercado, então vira uma venda
// (conta nas vendas do mês e gera ganho sobre o custo médio) e uma compra da moeda recebida pelo mesmo valor.
#[derive(Deserialize)]
pub struct CryptoSwapRequest {
    pub portfolio_id: i32,
    pub from_asset_id: i32,
    pub to_asset_id: i32,
    pub custodia_id: Option<i32>,
    pub transaction_date: String,
    pub quantity_out: f64,
    pub quantity_in: f64,
    /// Valor de mercado da troca, na moeda dos ativos (em reais para o IR).
    pub value: f64,
    pub fee: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Serialize)]
pub struct CryptoSwapResult {
    pub sale: transactions::Model,
    pub purchase: transactions::Model,
}

#[tauri::command]
pub async fn record_crypto_swap(
    db: tauri::State<'_, DatabaseConnection>,
    request: CryptoSwapRequest,
) -> Result<CryptoSwapResult, AppError> {
    record_crypto_swap_in(db.inner(), request).await
}

/// Grava as duas pernas da permuta numa só transação do banco: se a venda for recusada (saldo insuficiente), a compra
/// também não fica.
pub async fn record_crypto_swap_in(
    db: &DatabaseConnection,
    request: CryptoSwapRequest,
) -> Result<CryptoSwapResult, AppError> {
    if request.from_asset_id == request.to_asset_id {
        return Err(AppError::InvalidGuard("a permuta precisa de duas moedas diferentes".to_string()));
    }
    for (name, number) in [("quantity_out", request.quantity_out), ("quantity_in", request.quantity_in), ("value", request.value)] {
        if !number.is_finite() || number <= 0.0 {
            return Err(AppError::InvalidGuard(format!("{name} deve ser maior que zero")));
        }
    }
    let mut legs = Vec::with_capacity(2);
    for asset_id in [request.from_asset_id, request.to_asset_id] {
        let asset = assets::Entity::find_by_id(asset_id)
            .one(db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("asset {asset_id}")))?;
        if asset.asset_class != CRYPTO_CLASS {
            return Err(AppError::InvalidGuard(format!("{} não é cripto; a permuta só vale entre criptomoedas", asset.ticker)));
        }
        legs.push(asset);
    }
    if legs[0].currency != legs[1].currency {
        return Err(AppError::InvalidGuard(format!(
            "{} ({}) e {} ({}) estão em moedas diferentes; o valor da troca é um só",
            legs[0].ticker, legs[0].currency, legs[1].ticker, legs[1].currency
        )));
    }
    let label = format!("Swap {} → {}", legs[0].ticker, legs[1].ticker);
    let notes = match request.notes.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(extra) => format!("{label} — {extra}"),
        None => label,
    };
    let leg = |asset_id: i32, kind: &str, quantity: f64, fee: Option<f64>| CreateTransactionRequest {
        portfolio_id: request.portfolio_id,
        asset_id: Some(asset_id),
        custodia_id: request.custodia_id,
        transfer_to_custodia_id: None,
        transaction_type: kind.to_string(),
        payment_type: None,
        quantity: Some(quantity),
        unit_price: Some(request.value / quantity),
        total_value: request.value,
        fee,
        foreign_tax_withheld: None,
        transaction_date: request.transaction_date.clone(),
        notes: Some(notes.clone()),
        fi_emissor: None,
        fi_indexador: None,
        fi_taxa_percentual: None,
        fi_data_vencimento: None,
        fi_liquidez: None,
    };

    let txn = db.begin().await?;
    // A taxa fica na venda: reduz o ganho, como em `assess_crypto` (valor da venda menos a taxa).
    let sale = create_transaction_in(&txn, leg(request.from_asset_id, SELL, request.quantity_out, request.fee)).await?;
    let purchase = create_transaction_in(&txn, leg(request.to_asset_id, BUY, request.quantity_in, None)).await?;
    txn.commit().await?;
    Ok(CryptoSwapResult { sale, purchase })
}

/// Quantos dias antes da troca ainda vale o último fechamento (fim de semana e feriado não têm pregão).
const SWAP_PRICE_MAX_AGE_DAYS: i64 = 5;

#[tauri::command]
pub async fn suggest_crypto_swap_value(
    db: tauri::State<'_, DatabaseConnection>,
    asset_id: i32,
    date: String,
    quantity: f64,
) -> Result<Option<f64>, AppError> {
    suggest_crypto_swap_value_in(db.inner(), asset_id, &date, quantity).await
}

/// Valor de mercado em reais de `quantity` da moeda na data, pelo fechamento `-BRL` já coletado (o mesmo que o
/// backtest usa). `None` quando não há como sugerir: ativo que não é cripto ou não está em reais, sem fechamento
/// recente, ou entrada inválida; o formulário então deixa o valor para o usuário digitar.
pub async fn suggest_crypto_swap_value_in(
    db: &DatabaseConnection,
    asset_id: i32,
    date: &str,
    quantity: f64,
) -> Result<Option<f64>, AppError> {
    let Ok(day) = NaiveDate::parse_from_str(date, "%Y-%m-%d") else { return Ok(None) };
    if !quantity.is_finite() || quantity <= 0.0 {
        return Ok(None);
    }
    let asset = assets::Entity::find_by_id(asset_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {asset_id}")))?;
    if asset.asset_class != CRYPTO_CLASS || asset.currency != "BRL" {
        return Ok(None);
    }
    let close = stock_adjusted_close::Entity::find()
        .filter(stock_adjusted_close::Column::Ticker.eq(asset.ticker.as_str()))
        .filter(stock_adjusted_close::Column::PriceDate.lte(date))
        .order_by_desc(stock_adjusted_close::Column::PriceDate)
        .one(db)
        .await?;
    let Some(close) = close else { return Ok(None) };
    let fresh = NaiveDate::parse_from_str(&close.price_date, "%Y-%m-%d")
        .is_ok_and(|priced| (day - priced).num_days() <= SWAP_PRICE_MAX_AGE_DAYS);
    Ok(fresh.then(|| (quantity * close.adj_close_price * 100.0).round() / 100.0))
}

/// Recusa a amortização sem cotas em carteira na data ou maior que o valor da posição (cota × cotas). Tolera centavos de
/// arredondamento. Só olha o histórico até a data: lançamentos posteriores não entram na conta desta amortização.
async fn ensure_amortization_fits<C: ConnectionTrait>(
    db: &C,
    request: &CreateTransactionRequest,
) -> Result<(), AppError> {
    let Some(asset_id) = request.asset_id else { return Ok(()) };
    let date = NaiveDate::parse_from_str(&request.transaction_date, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidGuard("transaction_date inválida".to_string()))?;
    let existing = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.eq(request.portfolio_id))
        .filter(transactions::Column::AssetId.eq(asset_id))
        .all(db)
        .await?;
    let until_date: Vec<&transactions::Model> = existing
        .iter()
        .filter(|t| NaiveDate::parse_from_str(&t.transaction_date, "%Y-%m-%d").is_ok_and(|d| d <= date))
        .collect();
    let state = cost_basis::replay(&cost_events(&until_date)?, &cost_basis::native_rate)
        .ok_or_else(|| AppError::InvalidGuard("não foi possível apurar a posição do fundo".to_string()))?;
    if state.quantity <= cost_basis::EPS {
        return Err(AppError::InvalidGuard("a amortização precisa de cotas em carteira nessa data".to_string()));
    }
    let position_value = state.quantity * request.unit_price.unwrap_or(0.0);
    if request.total_value > position_value + 0.011_f64.max(position_value * 0.001) {
        return Err(AppError::InvalidGuard(format!(
            "o valor amortizado ({:.2}) passa do valor da posição nessa data ({position_value:.2})",
            request.total_value
        )));
    }
    Ok(())
}

/// Recusa a venda que deixaria o ativo com saldo negativo em algum ponto do
/// histórico (inclusive vendas posteriores, se esta é retroativa). Histórico que
/// já estava descoberto antes não bloqueia: só a venda que o piora.
async fn ensure_covered_sale<C: ConnectionTrait>(
    db: &C,
    request: &CreateTransactionRequest,
) -> Result<(), AppError> {
    let Some(asset_id) = request.asset_id else { return Ok(()) };
    let existing = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.eq(request.portfolio_id))
        .filter(transactions::Column::AssetId.eq(asset_id))
        .all(db)
        .await?;
    let refs: Vec<&transactions::Model> = existing.iter().collect();
    let before = cost_basis::replay(&cost_events(&refs)?, &cost_basis::native_rate)
        .is_some_and(|state| state.uncovered_sale);

    let date = NaiveDate::parse_from_str(&request.transaction_date, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidGuard("transaction_date inválida".to_string()))?;
    let mut events = cost_events(&refs)?;
    events.push(cost_basis::Event {
        date,
        id: i32::MAX,
        kind: EventKind::Sell,
        quantity: request.quantity.unwrap_or(0.0),
        // O come-cotas não recebe nada: o valor dele é o imposto, não o produto de uma venda.
        value: if request.transaction_type == COME_COTAS { 0.0 } else { request.total_value },
        fee: request.fee.unwrap_or(0.0),
        custodia_id: request.custodia_id,
        to_custodia_id: None,
    });
    let after = cost_basis::replay(&events, &cost_basis::native_rate).is_some_and(|state| state.uncovered_sale);
    if after && !before {
        return Err(AppError::InvalidGuard(
            "a venda passa da quantidade em carteira nessa data".to_string(),
        ));
    }
    Ok(())
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

pub(crate) fn custodia_label(custodias: &HashMap<i32, custodia::Model>, id: Option<i32>) -> Option<String> {
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
    pub foreign_tax_withheld: Option<f64>,
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
        .order_by_desc(transactions::Column::Id)
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
            foreign_tax_withheld: tx.foreign_tax_withheld,
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

// Preço médio = custo médio com baixa por venda, taxa no custo e reset ao
// zerar (`domain::cost_basis`, Fase 34 — antes era a média histórica das
// compras). `provento` não move quantidade (é caixa recebido, não ativo).
// `transferencia` move quantidade entre custódias sem alterar o total líquido
// do ativo no portfolio.
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
    // Fase 34 — ganho/perda já realizado nas vendas, na mesma moeda dos demais
    // valores. Posição encerrada (quantidade 0) continua na lista com ele.
    pub realized_pl: f64,
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
            .filter(asset_valuations::Column::AssetId.is_in(asset_ids.clone()))
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

    // Valor de cota dos fundos (lançado pelo usuário ou lido da CVM): o mais recente por ativo, com a origem.
    let latest_fund_quota: HashMap<i32, (f64, String)> = {
        let mut by_asset_id: HashMap<i32, (String, f64, String)> = HashMap::new();
        for row in fund_quota_value::Entity::find()
            .filter(fund_quota_value::Column::AssetId.is_in(asset_ids))
            .all(db)
            .await?
        {
            let entry = by_asset_id.entry(row.asset_id).or_insert((row.quota_date.clone(), row.value, row.source.clone()));
            if row.quota_date > entry.0 {
                *entry = (row.quota_date, row.value, row.source);
            }
        }
        by_asset_id.into_iter().map(|(k, (_, value, source))| (k, (value, source))).collect()
    };

    let mut positions = Vec::new();
    for (asset_id, asset_txs) in by_asset {
        let Some(asset) = assets_map.get(&asset_id) else {
            continue;
        };

        // Custo médio com baixa por venda, taxa no custo e reset ao zerar
        // (`domain::cost_basis`, o mesmo motor da apuração de IR).
        let state = cost_basis::replay(&cost_events(&asset_txs)?, &cost_basis::native_rate)
            .expect("a taxa nativa sempre existe");
        let net_qty = state.quantity;
        let average_buy_price = state.average_price();
        let realized_pl = state.realized_pl;

        let by_custodia = state
            .by_custodia
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
            mut price_source,
        } = position_pricing::price_position(
            &asset.asset_class,
            net_qty,
            average_buy_price,
            // Fundo: o valor de cota (manual ou da CVM) ocupa o lugar da cotação de mercado.
            if asset.asset_class == "fundo" {
                latest_fund_quota.get(&asset_id).map(|(value, _)| *value)
            } else {
                latest_ticker_price.get(&asset.ticker).copied()
            },
            latest_manual_valuation.get(&asset_id).copied(),
        );
        if price_source == "manual_quota"
            && latest_fund_quota.get(&asset_id).is_some_and(|(_, source)| source == "cvm")
        {
            price_source = "cvm_quota".to_string();
        }

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
            realized_pl,
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
