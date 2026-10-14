// Fase 24 — IR sobre renda variável. A apuração é sempre recalculada a partir
// de `transactions` (nunca persistida), no nível do Workspace inteiro — o
// imposto é por CPF, então somam-se as vendas de todas as carteiras dele.
// Os parâmetros (alíquotas, limites…) vêm de `tax_rules`: seed da migration,
// atualizável pelo Lume (`sync_tax_rules`) e editável pelo usuário — edição
// do usuário nunca é sobrescrita pela sincronização.

use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait, Unchanged,
};
use serde::{Deserialize, Serialize};

use crate::domain::tax::assessment::{self, Assessment, Side, Trade};
use crate::domain::tax::rules::{self, Rule, RuleSet};
use crate::entity::{
    assets, bank_account, general_transaction, general_transaction_category, portfolio, tax_payments,
    tax_rules, transactions,
};
use crate::error::AppError;
use crate::finance_api::{client, FinanceApiHandle};

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

pub(crate) async fn assess_workspace(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Assessment, AppError> {
    let rules = load_rule_set(db).await?;
    let portfolio_ids = workspace_portfolio_ids(db, workspace_id).await?;
    if portfolio_ids.is_empty() {
        return assessment::assess(&[], &rules);
    }

    let txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.is_in(portfolio_ids))
        .filter(transactions::Column::TransactionType.is_in([BUY, SELL]))
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
            side: if tx.transaction_type == BUY { Side::Buy } else { Side::Sell },
            quantity: tx.quantity.unwrap_or(0.0),
            gross_value: tx.total_value,
            fee: tx.fee.unwrap_or(0.0),
        });
    }

    assessment::assess(&trades, &rules)
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
    pub lume_value: f64,
}

#[derive(Serialize)]
pub struct TaxRuleSyncResult {
    pub added: u32,
    pub updated: u32,
    /// Regras editadas pelo usuário cujo valor oficial mudou no Lume — não
    /// são tocadas; a UI oferece "reset" pra aplicar o valor novo.
    pub conflicts: Vec<TaxRuleConflict>,
}

#[tauri::command]
pub async fn sync_tax_rules(
    db: tauri::State<'_, DatabaseConnection>,
    finance_api: tauri::State<'_, FinanceApiHandle>,
) -> Result<TaxRuleSyncResult, AppError> {
    let db = db.inner();
    let response = client::fetch_tax_rules_br(&finance_api).await?;
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
                        lume_value: rule.value,
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
    if request.amount <= 0.0 || !request.amount.is_finite() {
        return Err(AppError::InvalidInput("amount must be positive".to_string()));
    }
    parse_date(&request.paid_date)?;
    parse_date(&format!("{}-01", request.period))?;

    let db = db.inner();
    let account = bank_account::Entity::find_by_id(request.bank_account_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("bank account {}", request.bank_account_id)))?;
    if account.workspace_id != request.workspace_id {
        return Err(AppError::InvalidGuard("bank account belongs to another workspace".to_string()));
    }

    let now = Utc::now().to_rfc3339();
    let txn = db.begin().await?;

    let category_id = match general_transaction_category::Entity::find()
        .filter(general_transaction_category::Column::WorkspaceId.eq(request.workspace_id))
        .filter(general_transaction_category::Column::Nome.eq(TAXES_CATEGORY))
        .one(&txn)
        .await?
    {
        Some(c) => c.id,
        None => {
            general_transaction_category::ActiveModel {
                workspace_id: Set(request.workspace_id),
                nome: Set(TAXES_CATEGORY.to_string()),
                limite_mensal: Set(None),
                created_at: Set(now.clone()),
                ..Default::default()
            }
            .insert(&txn)
            .await?
            .id
        }
    };

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

// Teste `#[ignore]` — mesma convenção de `finance_api::*::tests`: precisa do
// Lume rodando em `http://localhost:8000` (`docker compose up` de lá, chave
// `local-dev-key-change-me`) e do banco real de dev com as migrations da
// Fase 24 aplicadas. Garante que o seed da migration (fallback offline) e o
// `/v1/tax-rules/br` do Lume não divergem. Rodar com
// `cargo test --lib -- --ignored commands::tax`.
#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    #[tokio::test]
    #[ignore]
    async fn live_lume_rules_match_migration_seed() {
        let handle = FinanceApiHandle::for_test(
            "http://localhost:8000".to_string(),
            "local-dev-key-change-me".to_string(),
        );
        let response = client::fetch_tax_rules_br(&handle).await.expect("Lume reachable");
        let db = Database::connect("sqlite:///data/anchor.db?mode=rwc").await.unwrap();
        let seeded: HashMap<(String, String), f64> = tax_rules::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|r| ((r.rule_key, r.effective_from), r.default_value))
            .collect();

        assert_eq!(response.rules.len(), seeded.len());
        for rule in response.rules {
            let key = (rule.key.clone(), rule.effective_from.format("%Y-%m-%d").to_string());
            let seed = seeded.get(&key).unwrap_or_else(|| panic!("missing seed for {key:?}"));
            assert!((seed - rule.value).abs() < EPS, "{key:?}: seed {seed} vs Lume {}", rule.value);
        }
    }
}
