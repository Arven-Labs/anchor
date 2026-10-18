// Fase 24 — IR sobre renda variável. A apuração é sempre recalculada a partir
// de `transactions` (nunca persistida), no nível do Workspace inteiro — o
// imposto é por CPF, então somam-se as vendas de todas as carteiras dele.
// Os parâmetros (alíquotas, limites…) vêm de `tax_rules`: seed da migration,
// atualizável pela tabela embarcada no app (`sync_tax_rules`) e editável pelo
// usuário — edição do usuário nunca é sobrescrita pela sincronização.

use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait, Unchanged,
};
use serde::{Deserialize, Serialize};

use crate::commands::transaction::{split_ratio, BONUS, REVERSE_SPLIT, SPLIT};
use crate::domain::option_position as option_calc;
use crate::domain::tax::assessment::{self, Assessment, OptionResult, Side, Trade};
use crate::domain::tax::classify::{treatment, TaxTreatment};
use crate::domain::tax::rules::{self, Rule, RuleSet};
use crate::entity::{
    assets, bank_account, general_transaction, general_transaction_category, option_position,
    portfolio, tax_payments, tax_rules, transactions,
};
use crate::error::AppError;
use crate::finance_api::client;

const BUY: &str = "compra";
const SELL: &str = "venda";
const DESPESA: &str = "despesa";
const TAXES_CATEGORY: &str = "Taxes";
const EPS: f64 = 1e-9;

fn parse_date(value: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value.get(0..10).unwrap_or(value), "%Y-%m-%d")
        .map_err(|_| AppError::InvalidGuard(format!("invalid date '{value}'")))
}

pub async fn load_rule_set(db: &DatabaseConnection) -> Result<RuleSet, AppError> {
    let rows = tax_rules::Entity::find().all(db).await?;
    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        rules.push(Rule {
            effective_from: parse_date(&row.effective_from)?,
            key: row.rule_key,
            value: row.value,
        });
    }
    Ok(RuleSet::new(rules))
}

/// IRRF do JCP vigente na data do pagamento (`YYYY-MM-DD`).
pub fn jcp_rate_on(rules: &RuleSet, date: &str) -> Result<f64, AppError> {
    rules.get(rules::JCP_IRRF_RATE, parse_date(date)?)
}

async fn workspace_portfolio_ids(db: &DatabaseConnection, workspace_id: i32) -> Result<Vec<i32>, AppError> {
    Ok(portfolio::Entity::find()
        .filter(portfolio::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.id)
        .collect())
}

/// Compras e vendas de todas as carteiras do Workspace, já resolvidas com os
/// dados do ativo (transação sem ativo é ignorada).
async fn load_trades(db: &DatabaseConnection, workspace_id: i32) -> Result<Vec<Trade>, AppError> {
    let portfolio_ids = workspace_portfolio_ids(db, workspace_id).await?;
    if portfolio_ids.is_empty() {
        return Ok(Vec::new());
    }

    let txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.is_in(portfolio_ids))
        .filter(transactions::Column::TransactionType.is_in([BUY, SELL, SPLIT, REVERSE_SPLIT, BONUS]))
        .all(db)
        .await?;

    let asset_ids: HashSet<i32> = txs.iter().filter_map(|t| t.asset_id).collect();
    let assets_map: HashMap<i32, assets::Model> = if asset_ids.is_empty() {
        HashMap::new()
    } else {
        assets::Entity::find()
            .filter(assets::Column::Id.is_in(asset_ids.into_iter().collect::<Vec<i32>>()))
            .all(db)
            .await?
            .into_iter()
            .map(|a| (a.id, a))
            .collect()
    };

    let mut trades = Vec::with_capacity(txs.len());
    for tx in &txs {
        let Some(asset) = tx.asset_id.and_then(|id| assets_map.get(&id)) else {
            continue;
        };
        trades.push(Trade {
            tx_id: tx.id,
            portfolio_id: tx.portfolio_id,
            asset_id: asset.id,
            ticker: asset.ticker.clone(),
            asset_class: asset.asset_class.clone(),
            currency: asset.currency.clone(),
            custodia_id: tx.custodia_id,
            date: parse_date(&tx.transaction_date)?,
            side: match tx.transaction_type.as_str() {
                BUY => Side::Buy,
                SPLIT | REVERSE_SPLIT => Side::Split,
                BONUS => Side::Bonus,
                _ => Side::Sell,
            },
            quantity: match tx.transaction_type.as_str() {
                SPLIT | REVERSE_SPLIT => split_ratio(&tx.transaction_type, tx.quantity.unwrap_or(0.0)),
                _ => tx.quantity.unwrap_or(0.0),
            },
            gross_value: tx.total_value,
            fee: tx.fee.unwrap_or(0.0),
        });
    }

    Ok(trades)
}

/// Posições em opções **encerradas** das carteiras do Workspace (Fase 18), no
/// formato da apuração. Posição aberta não entra: só vira resultado ao encerrar.
/// Day trade = aberta e encerrada no mesmo dia.
async fn load_option_results(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Vec<OptionResult>, AppError> {
    let portfolio_ids = workspace_portfolio_ids(db, workspace_id).await?;
    if portfolio_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = option_position::Entity::find()
        .filter(option_position::Column::PortfolioId.is_in(portfolio_ids))
        .filter(option_position::Column::CloseDate.is_not_null())
        .all(db)
        .await?;

    let mut results = Vec::with_capacity(rows.len());
    for row in rows {
        let (Some(close_date), Some(close_price)) = (row.close_date.as_deref(), row.close_price) else {
            continue;
        };
        let (proceeds, cost) = option_calc::proceeds_and_cost(
            &row.side,
            row.premium,
            row.quantity,
            close_price,
            row.open_fee,
            row.close_fee,
        );
        results.push(OptionResult {
            position_id: row.id,
            portfolio_id: row.portfolio_id,
            ticker: row.series_ticker.clone(),
            date: parse_date(close_date)?,
            day_trade: row.open_date == close_date,
            quantity: row.quantity,
            proceeds,
            cost,
            exercised: row.close_kind.as_deref() == Some(option_calc::EXERCISED),
        });
    }
    Ok(results)
}

pub(crate) async fn assess_workspace(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Assessment, AppError> {
    let rules = load_rule_set(db).await?;
    let trades = load_trades(db, workspace_id).await?;
    let options = load_option_results(db, workspace_id).await?;
    assessment::assess_with_options(&trades, &options, &rules)
}

#[derive(Deserialize)]
pub struct SimulateTaxSaleRequest {
    pub workspace_id: i32,
    pub asset_id: i32,
    pub quantity: f64,
    pub unit_price: f64,
    pub fee: Option<f64>,
    /// `YYYY-MM-DD`
    pub date: String,
}

/// Simula uma venda sem gravar nada: mostra quanto de prejuízo ela consome,
/// o imposto que geraria e o efeito nos meses seguintes. Só ativos do escopo
/// B3 (ações, units, ETF, BDR, FII).
#[tauri::command]
pub async fn simulate_tax_sale(
    db: tauri::State<'_, DatabaseConnection>,
    request: SimulateTaxSaleRequest,
) -> Result<assessment::SaleSimulation, AppError> {
    simulate_tax_sale_in(db.inner(), request).await
}

pub async fn simulate_tax_sale_in(
    db: &DatabaseConnection,
    request: SimulateTaxSaleRequest,
) -> Result<assessment::SaleSimulation, AppError> {
    if !request.quantity.is_finite() || request.quantity <= 0.0 {
        return Err(AppError::InvalidGuard("quantity must be greater than zero".to_string()));
    }
    if !request.unit_price.is_finite() || request.unit_price <= 0.0 {
        return Err(AppError::InvalidGuard("price must be greater than zero".to_string()));
    }
    let fee = request.fee.unwrap_or(0.0);
    if !fee.is_finite() || fee < 0.0 {
        return Err(AppError::InvalidGuard("fee cannot be negative".to_string()));
    }
    let date = parse_date(&request.date)?;

    let asset = assets::Entity::find_by_id(request.asset_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {}", request.asset_id)))?;
    if !matches!(
        treatment(&asset.asset_class, &asset.ticker),
        TaxTreatment::StockExemptEligible | TaxTreatment::CommonNoExemption | TaxTreatment::Fii
    ) {
        return Err(AppError::InvalidInput(format!(
            "{} is outside the B3 capital gains scope Anchor calculates",
            asset.ticker
        )));
    }

    let rules = load_rule_set(db).await?;
    let trades = load_trades(db, request.workspace_id).await?;
    // Carteira da venda: a da última operação do ativo (a posição que
    // provavelmente seria vendida).
    let portfolio_id = trades
        .iter()
        .filter(|t| t.asset_id == asset.id)
        .max_by_key(|t| (t.date, t.tx_id))
        .map(|t| t.portfolio_id)
        .ok_or_else(|| AppError::InvalidGuard("this asset has no recorded purchases to sell".to_string()))?;

    let sale = Trade {
        tx_id: assessment::SIMULATED_TX_ID,
        portfolio_id,
        asset_id: asset.id,
        ticker: asset.ticker,
        asset_class: asset.asset_class,
        currency: asset.currency,
        custodia_id: None,
        date,
        side: Side::Sell,
        quantity: request.quantity,
        gross_value: request.quantity * request.unit_price,
        fee,
    };
    let options = load_option_results(db, request.workspace_id).await?;
    assessment::simulate_sale_with_options(&trades, &options, &rules, sale)
}

// ---------------------------------------------------------------------------
// Regras
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_tax_rules(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<tax_rules::Model>, AppError> {
    Ok(tax_rules::Entity::find()
        .order_by_asc(tax_rules::Column::RuleKey)
        .order_by_asc(tax_rules::Column::EffectiveFrom)
        .all(db.inner())
        .await?)
}

async fn find_rule(db: &DatabaseConnection, id: i32) -> Result<tax_rules::Model, AppError> {
    tax_rules::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("tax rule {id}")))
}

#[tauri::command]
pub async fn update_tax_rule(
    db: tauri::State<'_, DatabaseConnection>,
    id: i32,
    value: f64,
) -> Result<tax_rules::Model, AppError> {
    if !value.is_finite() || value < 0.0 {
        return Err(AppError::InvalidInput(format!("invalid tax rule value {value}")));
    }
    let existing = find_rule(db.inner(), id).await?;
    Ok(tax_rules::ActiveModel {
        id: Unchanged(existing.id),
        value: Set(value),
        user_edited: Set((value - existing.default_value).abs() > EPS),
        updated_at: Set(Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .update(db.inner())
    .await?)
}

#[tauri::command]
pub async fn reset_tax_rule(
    db: tauri::State<'_, DatabaseConnection>,
    id: i32,
) -> Result<tax_rules::Model, AppError> {
    let existing = find_rule(db.inner(), id).await?;
    Ok(tax_rules::ActiveModel {
        id: Unchanged(existing.id),
        value: Set(existing.default_value),
        user_edited: Set(false),
        updated_at: Set(Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .update(db.inner())
    .await?)
}

#[derive(Serialize)]
pub struct TaxRuleConflict {
    pub id: i32,
    pub rule_key: String,
    pub effective_from: String,
    pub your_value: f64,
    pub official_value: f64,
}

#[derive(Serialize)]
pub struct TaxRuleSyncResult {
    pub added: u32,
    pub updated: u32,
    /// Regras editadas pelo usuário cujo valor oficial mudou na tabela embarcada
    /// — não são tocadas; a UI oferece "reset" pra aplicar o valor novo.
    pub conflicts: Vec<TaxRuleConflict>,
}

#[tauri::command]
pub async fn sync_tax_rules(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<TaxRuleSyncResult, AppError> {
    let db = db.inner();
    let response = client::fetch_tax_rules_br().await?;
    let existing: HashMap<(String, String), tax_rules::Model> = tax_rules::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|r| ((r.rule_key.clone(), r.effective_from.clone()), r))
        .collect();

    let now = Utc::now().to_rfc3339();
    let mut result = TaxRuleSyncResult { added: 0, updated: 0, conflicts: Vec::new() };

    for rule in response.rules {
        let effective_from = rule.effective_from.format("%Y-%m-%d").to_string();
        let verified_at = rule.verified_at.format("%Y-%m-%d").to_string();
        match existing.get(&(rule.key.clone(), effective_from.clone())) {
            None => {
                tax_rules::ActiveModel {
                    rule_key: Set(rule.key),
                    effective_from: Set(effective_from),
                    value: Set(rule.value),
                    default_value: Set(rule.value),
                    description: Set(rule.description),
                    source: Set(rule.source),
                    verified_at: Set(verified_at),
                    user_edited: Set(false),
                    updated_at: Set(now.clone()),
                    ..Default::default()
                }
                .insert(db)
                .await?;
                result.added += 1;
            }
            Some(row) => {
                let official_changed = (row.default_value - rule.value).abs() > EPS;
                let keep_user_value = row.user_edited;
                if keep_user_value && (row.value - rule.value).abs() > EPS {
                    result.conflicts.push(TaxRuleConflict {
                        id: row.id,
                        rule_key: row.rule_key.clone(),
                        effective_from: row.effective_from.clone(),
                        your_value: row.value,
                        official_value: rule.value,
                    });
                }
                if official_changed && !keep_user_value {
                    result.updated += 1;
                }
                tax_rules::ActiveModel {
                    id: Unchanged(row.id),
                    value: Set(if keep_user_value { row.value } else { rule.value }),
                    default_value: Set(rule.value),
                    description: Set(rule.description),
                    source: Set(rule.source),
                    verified_at: Set(verified_at),
                    updated_at: Set(now.clone()),
                    ..Default::default()
                }
                .update(db)
                .await?;
            }
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Apuração e pagamento
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct TaxAssessmentView {
    pub assessment: Assessment,
    pub payments: Vec<tax_payments::Model>,
}

#[tauri::command]
pub async fn get_tax_assessment(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<TaxAssessmentView, AppError> {
    let db = db.inner();
    let assessment = assess_workspace(db, workspace_id).await?;
    let payments = tax_payments::Entity::find()
        .filter(tax_payments::Column::WorkspaceId.eq(workspace_id))
        .order_by_asc(tax_payments::Column::Period)
        .all(db)
        .await?;
    Ok(TaxAssessmentView { assessment, payments })
}

#[derive(Deserialize)]
pub struct MarkTaxPaidRequest {
    pub workspace_id: i32,
    pub period: String,
    pub amount: f64,
    pub paid_date: String,
    pub bank_account_id: i32,
}

#[tauri::command]
pub async fn mark_tax_paid(
    db: tauri::State<'_, DatabaseConnection>,
    request: MarkTaxPaidRequest,
) -> Result<tax_payments::Model, AppError> {
    mark_tax_paid_in(db.inner(), request).await
}

/// Categoria "Taxes" do Workspace, criada se ainda não existir (DARF e IR retido
/// de renda fixa caem nela).
pub(crate) async fn ensure_taxes_category<C: sea_orm::ConnectionTrait>(
    db: &C,
    workspace_id: i32,
) -> Result<i32, AppError> {
    if let Some(category) = general_transaction_category::Entity::find()
        .filter(general_transaction_category::Column::WorkspaceId.eq(workspace_id))
        .filter(general_transaction_category::Column::Nome.eq(TAXES_CATEGORY))
        .one(db)
        .await?
    {
        return Ok(category.id);
    }
    Ok(general_transaction_category::ActiveModel {
        workspace_id: Set(workspace_id),
        nome: Set(TAXES_CATEGORY.to_string()),
        limite_mensal: Set(None),
        created_at: Set(Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db)
    .await?
    .id)
}

pub async fn mark_tax_paid_in(
    db: &DatabaseConnection,
    request: MarkTaxPaidRequest,
) -> Result<tax_payments::Model, AppError> {
    if request.amount <= 0.0 || !request.amount.is_finite() {
        return Err(AppError::InvalidInput("amount must be positive".to_string()));
    }
    parse_date(&request.paid_date)?;
    parse_date(&format!("{}-01", request.period))?;

    let account = bank_account::Entity::find_by_id(request.bank_account_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("bank account {}", request.bank_account_id)))?;
    if account.workspace_id != request.workspace_id {
        return Err(AppError::InvalidGuard("bank account belongs to another workspace".to_string()));
    }
    // O DARF é sempre em reais; numa conta em outra moeda o débito sairia no
    // valor errado.
    if account.currency != "BRL" {
        return Err(AppError::InvalidInput(format!(
            "the tax is paid in BRL, but the account is in {}",
            account.currency
        )));
    }

    let now = Utc::now().to_rfc3339();
    let txn = db.begin().await?;

    let category_id = ensure_taxes_category(&txn, request.workspace_id).await?;

    let expense = general_transaction::ActiveModel {
        workspace_id: Set(request.workspace_id),
        bank_account_id: Set(request.bank_account_id),
        transaction_type: Set(DESPESA.to_string()),
        category_id: Set(Some(category_id)),
        valor: Set(request.amount),
        transaction_date: Set(request.paid_date.clone()),
        notes: Set(Some(format!("DARF — capital gains tax (stock exchange), period {}", request.period))),
        created_at: Set(now.clone()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    let payment = tax_payments::ActiveModel {
        workspace_id: Set(request.workspace_id),
        period: Set(request.period),
        amount: Set(request.amount),
        paid_date: Set(request.paid_date),
        general_transaction_id: Set(Some(expense.id)),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;
    Ok(payment)
}

/// Desfaz o "pago": apaga o registro e a despesa criada junto em Finanças Gerais.
#[tauri::command]
pub async fn unmark_tax_paid(
    db: tauri::State<'_, DatabaseConnection>,
    payment_id: i32,
) -> Result<(), AppError> {
    let db = db.inner();
    let payment = tax_payments::Entity::find_by_id(payment_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("tax payment {payment_id}")))?;

    let txn = db.begin().await?;
    tax_payments::Entity::delete_by_id(payment.id).exec(&txn).await?;
    if let Some(gt_id) = payment.general_transaction_id {
        general_transaction::Entity::delete_by_id(gt_id).exec(&txn).await?;
    }
    txn.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Rentabilidade líquida de imposto
// ---------------------------------------------------------------------------

/// Imposto pago que cabe às carteiras/ativo pedidos, como (data do
/// pagamento, valor). Cada DARF é rateado entre as vendas do mês apurado,
/// pró-rata do ganho tributável de cada uma (lucro isento e prejuízo não
/// geram imposto, então não recebem rateio). Valor acumulado de meses
/// anteriores (abaixo do mínimo) é rateado pelas vendas do mês do DARF —
/// aproximação aceita.
pub(crate) async fn tax_cost_flows(
    db: &DatabaseConnection,
    portfolio_ids: &[i32],
    asset_id: Option<i32>,
) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let workspace_ids: HashSet<i32> = portfolio::Entity::find()
        .filter(portfolio::Column::Id.is_in(portfolio_ids.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.workspace_id)
        .collect();

    let mut flows = Vec::new();
    for workspace_id in workspace_ids {
        let payments = tax_payments::Entity::find()
            .filter(tax_payments::Column::WorkspaceId.eq(workspace_id))
            .all(db)
            .await?;
        if payments.is_empty() {
            continue;
        }
        let assessment = assess_workspace(db, workspace_id).await?;
        for payment in payments {
            let Some(month) = assessment.months.iter().find(|m| m.period == payment.period) else {
                continue;
            };
            let weight = |s: &assessment::SaleResult| {
                if s.exemption_eligible && month.stock_exempt {
                    0.0
                } else {
                    s.gain.max(0.0)
                }
            };
            let total: f64 = month.sales.iter().map(weight).sum();
            if total <= EPS {
                continue;
            }
            let ours: f64 = month
                .sales
                .iter()
                .filter(|s| portfolio_ids.contains(&s.portfolio_id))
                .filter(|s| asset_id.is_none_or(|a| s.asset_id == a))
                .map(weight)
                .sum();
            if ours > EPS {
                flows.push((parse_date(&payment.paid_date)?, payment.amount * ours / total));
            }
        }
    }
    Ok(flows)
}

// Fase 26 — a tabela embarcada (`finance_core::tax_rules`, o que
// `sync_tax_rules` aplica) e o seed da migration (fallback offline) guardam
// o mesmo conteúdo; este teste pega a divergência sem rede nem Lume.
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn embedded_rules_match_the_migration_seed() {
        let db = crate::test_support::memory_db().await;
        let seeded: HashMap<(String, String), tax_rules::Model> = tax_rules::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|r| ((r.rule_key.clone(), r.effective_from.clone()), r))
            .collect();

        let embedded = crate::finance_core::tax_rules::rules_br();
        assert_eq!(embedded.len(), seeded.len());
        for rule in embedded {
            let key = (rule.key.to_string(), rule.effective_from.format("%Y-%m-%d").to_string());
            let seed = seeded.get(&key).unwrap_or_else(|| panic!("missing seed for {key:?}"));
            assert!((seed.default_value - rule.value).abs() < EPS, "{key:?} value");
            assert_eq!(seed.description, rule.description, "{key:?} description");
            assert_eq!(seed.source, rule.source, "{key:?} source");
        }
    }
}
