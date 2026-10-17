use std::collections::HashMap;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    Unchanged,
};
use serde::{Deserialize, Serialize};

use crate::entity::{alert_event, alert_rule, indicator_thresholds, valuation};
use crate::error::AppError;

const STOCK_PRICE: &str = "stock_price";
const CRYPTO_INDICATOR: &str = "crypto_indicator";
const STOCK_CONDITIONS: [&str; 2] = ["BELOW_FAIR_PRICE", "ABOVE_FAIR_PRICE"];
const CRYPTO_CONDITIONS: [&str; 2] = ["SIGNAL_GREEN", "SIGNAL_RED"];

#[derive(Deserialize)]
pub struct CreateAlertRuleRequest {
    pub target_type: String,
    pub condition: String,
    pub valuation_id: Option<i32>,
    pub coin: Option<String>,
    pub indicator: Option<String>,
}

// Editing target/condition after creation is out of scope for Fase 5.1
// (delete + recreate instead), so there's no `UpdateAlertRuleRequest`.
// Cross-field validation lives here rather than in a domain module — unlike
// the 7 valuation models this isn't a calculation with a pure function to
// unit-test, it's "does this combination of fields make sense" gating, same
// spirit as `record_crypto_indicator`'s threshold lookup.
#[tauri::command]
pub async fn create_alert_rule(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateAlertRuleRequest,
) -> Result<alert_rule::Model, AppError> {
    let (valuation_id, coin, indicator) = match request.target_type.as_str() {
        STOCK_PRICE => {
            if !STOCK_CONDITIONS.contains(&request.condition.as_str()) {
                return Err(AppError::InvalidGuard(format!(
                    "condition '{}' is not valid for target_type 'stock_price' (expected BELOW_FAIR_PRICE or ABOVE_FAIR_PRICE)",
                    request.condition
                )));
            }
            let valuation_id = request.valuation_id.ok_or_else(|| {
                AppError::InvalidGuard(
                    "valuation_id is required for target_type 'stock_price'".to_string(),
                )
            })?;
            let target = valuation::Entity::find_by_id(valuation_id)
                .one(db.inner())
                .await?
                .ok_or_else(|| AppError::NotFound(format!("valuation {valuation_id}")))?;
            if target.fair_price.is_none() {
                return Err(AppError::InvalidGuard(format!(
                    "valuation {valuation_id} has no fair price yet"
                )));
            }
            (Some(valuation_id), None, None)
        }
        CRYPTO_INDICATOR => {
            if !CRYPTO_CONDITIONS.contains(&request.condition.as_str()) {
                return Err(AppError::InvalidGuard(format!(
                    "condition '{}' is not valid for target_type 'crypto_indicator' (expected SIGNAL_GREEN or SIGNAL_RED)",
                    request.condition
                )));
            }
            let coin = request
                .coin
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .ok_or_else(|| {
                    AppError::InvalidGuard(
                        "coin is required for target_type 'crypto_indicator'".to_string(),
                    )
                })?
                .to_uppercase();
            let indicator = request.indicator.clone().ok_or_else(|| {
                AppError::InvalidGuard(
                    "indicator is required for target_type 'crypto_indicator'".to_string(),
                )
            })?;
            // Same guard as `record_crypto_indicator`: an indicator key only
            // means something if it has a configured threshold row for that
            // coin (BTC has its own; every other coin uses ETH's).
            indicator_thresholds::Entity::find()
                .filter(
                    indicator_thresholds::Column::Coin
                        .eq(crate::domain::crypto_score::threshold_coin(&coin)),
                )
                .filter(indicator_thresholds::Column::Indicator.eq(&indicator))
                .one(db.inner())
                .await?
                .ok_or_else(|| AppError::UnknownIndicator(indicator.clone()))?;
            (None, Some(coin), Some(indicator))
        }
        other => {
            return Err(AppError::InvalidGuard(format!(
                "unknown target_type '{other}' (expected 'stock_price' or 'crypto_indicator')"
            )))
        }
    };

    let rule = alert_rule::ActiveModel {
        target_type: Set(request.target_type),
        valuation_id: Set(valuation_id),
        condition: Set(request.condition),
        coin: Set(coin),
        indicator: Set(indicator),
        is_active: Set(true),
        created_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db.inner())
    .await?;

    Ok(rule)
}

// Flat view for the frontend list — avoids N+1 by batch-fetching the
// distinct `valuation` rows referenced by stock_price rules and merging in
// Rust, keeping the frontend a dumb renderer.
#[derive(Serialize)]
pub struct AlertRuleView {
    pub id: i32,
    pub target_type: String,
    pub condition: String,
    pub is_active: bool,
    pub created_at: String,
    pub valuation_id: Option<i32>,
    pub ticker: Option<String>,
    pub fair_price: Option<f64>,
    pub coin: Option<String>,
    pub indicator: Option<String>,
    pub is_triggered: bool,
    pub last_message: Option<String>,
}

#[tauri::command]
pub async fn list_alert_rules(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<AlertRuleView>, AppError> {
    let rules = alert_rule::Entity::find()
        .order_by_desc(alert_rule::Column::CreatedAt)
        .all(db.inner())
        .await?;

    let valuation_ids: Vec<i32> = rules.iter().filter_map(|r| r.valuation_id).collect();
    let valuations: HashMap<i32, valuation::Model> = if valuation_ids.is_empty() {
        HashMap::new()
    } else {
        valuation::Entity::find()
            .filter(valuation::Column::Id.is_in(valuation_ids))
            .all(db.inner())
            .await?
            .into_iter()
            .map(|v| (v.id, v))
            .collect()
    };

    // Same batch-then-fold pattern as `valuations` above — surfaces the
    // background checker's (Fase 5.2, alert_checker.rs) latest known state
    // for each rule without a second round-trip from the frontend.
    let rule_ids: Vec<i32> = rules.iter().map(|r| r.id).collect();
    let latest_events: HashMap<i32, alert_event::Model> = alert_event::Entity::find()
        .filter(alert_event::Column::AlertRuleId.is_in(rule_ids))
        .order_by_desc(alert_event::Column::CreatedAt)
        .all(db.inner())
        .await?
        .into_iter()
        .fold(HashMap::new(), |mut acc, event| {
            acc.entry(event.alert_rule_id).or_insert(event);
            acc
        });

    let views = rules
        .into_iter()
        .map(|rule| {
            let related = rule.valuation_id.and_then(|id| valuations.get(&id));
            let last_event = latest_events.get(&rule.id);
            AlertRuleView {
                id: rule.id,
                target_type: rule.target_type,
                condition: rule.condition,
                is_active: rule.is_active,
                created_at: rule.created_at,
                valuation_id: rule.valuation_id,
                ticker: related.map(|v| v.ticker.clone()),
                fair_price: related.and_then(|v| v.fair_price),
                coin: rule.coin,
                indicator: rule.indicator,
                is_triggered: last_event.map(|e| e.is_triggered).unwrap_or(false),
                last_message: last_event.map(|e| e.message.clone()),
            }
        })
        .collect();

    Ok(views)
}

#[derive(Deserialize)]
pub struct SetAlertRuleActiveRequest {
    pub alert_rule_id: i32,
    pub is_active: bool,
}

#[tauri::command]
pub async fn set_alert_rule_active(
    db: tauri::State<'_, DatabaseConnection>,
    request: SetAlertRuleActiveRequest,
) -> Result<alert_rule::Model, AppError> {
    let existing = alert_rule::Entity::find_by_id(request.alert_rule_id)
        .one(db.inner())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("alert rule {}", request.alert_rule_id)))?;

    let updated = alert_rule::ActiveModel {
        id: Unchanged(existing.id),
        is_active: Set(request.is_active),
        ..Default::default()
    }
    .update(db.inner())
    .await?;

    Ok(updated)
}

#[tauri::command]
pub async fn delete_alert_rule(
    db: tauri::State<'_, DatabaseConnection>,
    alert_rule_id: i32,
) -> Result<(), AppError> {
    alert_rule::Entity::delete_by_id(alert_rule_id)
        .exec(db.inner())
        .await?;

    Ok(())
}
