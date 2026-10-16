use std::collections::HashMap;
use std::time::Duration;

use notify_rust::Notification as DesktopNotification;
#[cfg(all(unix, not(target_os = "macos")))]
use notify_rust::Urgency;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::domain::alert_check;
use crate::entity::{alert_event, alert_rule, crypto_indicators, stock_quotes, valuation};
use crate::error::AppError;

const CHECK_INTERVAL: Duration = Duration::from_secs(300);
const STOCK_PRICE: &str = "stock_price";
const CRYPTO_INDICATOR: &str = "crypto_indicator";

fn evaluate_stock_rule(
    rule: &alert_rule::Model,
    valuations: &HashMap<i32, valuation::Model>,
    latest_price_by_ticker: &HashMap<String, f64>,
) -> Option<(bool, String)> {
    let v = valuations.get(&rule.valuation_id?)?;
    let fair_price = v.fair_price?;
    let current_price = *latest_price_by_ticker.get(&v.ticker)?;
    let is_triggered = alert_check::evaluate_stock_price(&rule.condition, fair_price, current_price);
    let message = format!(
        "{} price {current_price:.2} {} fair price {fair_price:.2}",
        v.ticker,
        if is_triggered { "now crosses" } else { "no longer crosses" },
    );
    Some((is_triggered, message))
}

fn evaluate_crypto_rule(
    rule: &alert_rule::Model,
    latest_signal_by_coin_indicator: &HashMap<(String, String), String>,
) -> Option<(bool, String)> {
    let coin = rule.coin.clone()?;
    let indicator = rule.indicator.clone()?;
    let signal = latest_signal_by_coin_indicator.get(&(coin.clone(), indicator.clone()))?;
    let is_triggered = alert_check::evaluate_crypto_indicator(&rule.condition, signal);
    let message = format!("{coin} {indicator} signal is {signal}");
    Some((is_triggered, message))
}

/// Reevaluates every active `alert_rule` against whatever data is already in
/// the DB (never triggers the Python collector itself — Fase 5.2 is purely
/// "check what's there"), appending a new `alert_event` row only when a
/// rule's triggered state flips, in either direction.
pub async fn check_active_rules(db: &DatabaseConnection) -> Result<(), AppError> {
    let rules = alert_rule::Entity::find()
        .filter(alert_rule::Column::IsActive.eq(true))
        .all(db)
        .await?;

    if rules.is_empty() {
        return Ok(());
    }

    let valuation_ids: Vec<i32> = rules.iter().filter_map(|r| r.valuation_id).collect();
    let valuations: HashMap<i32, valuation::Model> = if valuation_ids.is_empty() {
        HashMap::new()
    } else {
        valuation::Entity::find()
            .filter(valuation::Column::Id.is_in(valuation_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|v| (v.id, v))
            .collect()
    };

    // Neither stock_quotes nor crypto_indicators has a "latest only"
    // constraint (they're append-only time series, same convention as
    // valuation) — order desc and keep the first row seen per key, same
    // batch-then-fold-in-Rust pattern list_alert_rules already uses.
    let latest_price_by_ticker: HashMap<String, f64> = stock_quotes::Entity::find()
        .order_by_desc(stock_quotes::Column::FetchedAt)
        .all(db)
        .await?
        .into_iter()
        .fold(HashMap::new(), |mut acc, quote| {
            acc.entry(quote.ticker).or_insert(quote.price);
            acc
        });

    let latest_signal_by_coin_indicator: HashMap<(String, String), String> =
        crypto_indicators::Entity::find()
            .order_by_desc(crypto_indicators::Column::ReadingDate)
            .all(db)
            .await?
            .into_iter()
            .fold(HashMap::new(), |mut acc, reading| {
                acc.entry((reading.coin, reading.indicator))
                    .or_insert(reading.signal);
                acc
            });

    let rule_ids: Vec<i32> = rules.iter().map(|r| r.id).collect();
    let latest_event_by_rule: HashMap<i32, alert_event::Model> = alert_event::Entity::find()
        .filter(alert_event::Column::AlertRuleId.is_in(rule_ids))
        .order_by_desc(alert_event::Column::CreatedAt)
        .all(db)
        .await?
        .into_iter()
        .fold(HashMap::new(), |mut acc, event| {
            acc.entry(event.alert_rule_id).or_insert(event);
            acc
        });

    for rule in &rules {
        let evaluation = match rule.target_type.as_str() {
            STOCK_PRICE => evaluate_stock_rule(rule, &valuations, &latest_price_by_ticker),
            CRYPTO_INDICATOR => evaluate_crypto_rule(rule, &latest_signal_by_coin_indicator),
            _ => None,
        };

        let Some((is_triggered, message)) = evaluation else {
            continue; // no quote/indicator reading yet for this rule's target
        };

        let previously_triggered = latest_event_by_rule
            .get(&rule.id)
            .map(|e| e.is_triggered)
            .unwrap_or(false);

        if is_triggered != previously_triggered {
            if is_triggered {
                // Critical urgency, not just cosmetic: some desktop environments (confirmed on
                // KDE Plasma 6/Wayland) silently drop normal-urgency notifications from apps
                // without a registered .desktop file into the history panel only, no popup.
                // `urgency` only exists on Linux/BSD in notify-rust (not macOS/Windows).
                let mut notification = DesktopNotification::new();
                notification.summary("Anchor").body(&message);
                #[cfg(all(unix, not(target_os = "macos")))]
                notification.urgency(Urgency::Critical);
                if let Err(err) = notification.show() {
                    eprintln!("failed to show notification: {err}");
                }
            }

            alert_event::ActiveModel {
                alert_rule_id: Set(rule.id),
                is_triggered: Set(is_triggered),
                message: Set(message),
                created_at: Set(chrono::Utc::now().to_rfc3339()),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }

    Ok(())
}

pub fn spawn_periodic_check(db: DatabaseConnection) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(CHECK_INTERVAL);
        loop {
            ticker.tick().await;
            if let Err(err) = check_active_rules(&db).await {
                eprintln!("alert check failed: {err}");
            }
        }
    });
}
