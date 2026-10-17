// Fase 13.4 — agregados pra tela dedicada "Proventos": KPIs de 12M/all-time
// (só pago, líquido de IR — "recebidos"), donut por ativo (12M, só pago),
// série mensal empilhada pago/a-receber (`domain::proventos::bucket_monthly`)
// e a tabela linha-a-linha "My dividends" (pago vindo de `transactions`,
// a receber vindo de `suggested_dividends` ainda não confirmado/descartado).

use std::collections::{HashMap, HashSet};

use chrono::{Duration, NaiveDate, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::commands::currency::{base_currency, load_fx_history};
use crate::commands::tax::{jcp_rate_on, load_rule_set};
use crate::domain::proventos::{bucket_monthly, by_asset, net_total, ProventoEntry, PAYMENT_TYPE_DIVIDENDO};
use crate::entity::{assets, portfolio, suggested_dividends, transactions};
use crate::error::AppError;

const DIVIDEND: &str = "provento";
const RECEIVABLE_STATUSES: [&str; 3] = ["pending", "matched", "divergent"];
const STATUS_CONFIRMED: &str = "confirmed";

#[derive(Serialize)]
pub struct ProventoRow {
    pub asset_id: i32,
    pub ticker: String,
    pub name: String,
    pub asset_class: String,
    pub status: String,       // "paid" | "receivable"
    pub payment_type: String, // "dividendo" | "jscp"
    pub com_date: Option<String>,
    pub payment_date: String,
    pub quantity: f64,
    /// Na moeda do ativo (`native_currency`).
    pub amount_per_share: f64,
    /// Fase 19 — `total_value` e `net_total` estão na moeda base, convertidos
    /// pela taxa da data do pagamento; `native_total_value` é o original, em
    /// `native_currency`. Sem rota de câmbio, `fx_missing` liga, os dois ficam
    /// em 0 e a linha não entra nos agregados.
    pub total_value: f64,
    pub net_total: f64,
    pub native_currency: String,
    pub native_total_value: f64,
    pub fx_missing: bool,
}

#[derive(Serialize)]
pub struct ProventosMonthlyBucketView {
    pub year_month: String,
    pub received: f64,
    pub expected: f64,
}

#[derive(Serialize)]
pub struct ProventosAssetTotal {
    pub asset_id: i32,
    pub ticker: String,
    pub total: f64,
}

#[derive(Serialize)]
pub struct ProventosSummary {
    pub avg_monthly_12m: f64,
    pub total_12m: f64,
    pub total_all_time: f64,
    pub by_asset_12m: Vec<ProventosAssetTotal>,
    pub monthly: Vec<ProventosMonthlyBucketView>,
    pub rows: Vec<ProventoRow>,
    /// Fase 19 — moeda dos totais, do gráfico e de `total_value`/`net_total`.
    pub base_currency: String,
}

#[tauri::command]
pub async fn get_proventos_summary(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<ProventosSummary, AppError> {
    proventos_summary_in(db.inner(), portfolio_id).await
}

pub async fn proventos_summary_in(
    db: &DatabaseConnection,
    portfolio_id: i32,
) -> Result<ProventosSummary, AppError> {
    let portfolio = portfolio::Entity::find_by_id(portfolio_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("portfolio {portfolio_id}")))?;
    let base = base_currency(db, portfolio.workspace_id).await?;

    let paid_txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.eq(portfolio_id))
        .filter(transactions::Column::TransactionType.eq(DIVIDEND))
        .all(db)
        .await?;

    let all_suggestions = suggested_dividends::Entity::find()
        .filter(suggested_dividends::Column::PortfolioId.eq(portfolio_id))
        .all(db)
        .await?;

    // Data Com de linhas pagas: `confirm_dividend_suggestion` não guarda o
    // id da transação criada, então a junção é implícita por
    // (asset_id, payment_date) contra a sugestão `confirmed` — mesma data
    // que virou `transaction_date` no lançamento (ver
    // `commands/dividend_suggestion.rs::confirm_dividend_suggestion`).
    let confirmed_com_dates: HashMap<(i32, String), Option<String>> = all_suggestions
        .iter()
        .filter(|s| s.status == STATUS_CONFIRMED)
        .map(|s| ((s.asset_id, s.payment_date.clone()), s.com_date.clone()))
        .collect();

    let receivable: Vec<&suggested_dividends::Model> = all_suggestions
        .iter()
        .filter(|s| RECEIVABLE_STATUSES.contains(&s.status.as_str()))
        .collect();

    let mut asset_ids: HashSet<i32> = paid_txs.iter().filter_map(|tx| tx.asset_id).collect();
    asset_ids.extend(receivable.iter().map(|s| s.asset_id));

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

    let rules = load_rule_set(db).await?;
    let mut rows: Vec<ProventoRow> = Vec::new();

    for tx in &paid_txs {
        let Some(asset_id) = tx.asset_id else { continue };
        let Some(asset) = assets_map.get(&asset_id) else { continue };
        let payment_type = tx.payment_type.clone().unwrap_or_else(|| PAYMENT_TYPE_DIVIDENDO.to_string());
        let com_date = confirmed_com_dates
            .get(&(asset_id, tx.transaction_date.clone()))
            .cloned()
            .flatten();
        rows.push(ProventoRow {
            asset_id,
            ticker: asset.ticker.clone(),
            name: asset.name.clone(),
            asset_class: asset.asset_class.clone(),
            status: "paid".to_string(),
            net_total: net_total(
                tx.total_value,
                Some(payment_type.as_str()),
                jcp_rate_on(&rules, &tx.transaction_date)?,
            ),
            payment_type,
            com_date,
            payment_date: tx.transaction_date.clone(),
            quantity: tx.quantity.unwrap_or(0.0),
            amount_per_share: tx.unit_price.unwrap_or(0.0),
            total_value: tx.total_value,
            native_currency: asset.currency.clone(),
            native_total_value: tx.total_value,
            fx_missing: false,
        });
    }

    for s in &receivable {
        let Some(asset) = assets_map.get(&s.asset_id) else { continue };
        rows.push(ProventoRow {
            asset_id: s.asset_id,
            ticker: asset.ticker.clone(),
            name: asset.name.clone(),
            asset_class: asset.asset_class.clone(),
            status: "receivable".to_string(),
            net_total: net_total(
                s.total,
                Some(s.payment_type.as_str()),
                jcp_rate_on(&rules, &s.payment_date)?,
            ),
            payment_type: s.payment_type.clone(),
            com_date: s.com_date.clone(),
            payment_date: s.payment_date.clone(),
            quantity: s.quantity,
            amount_per_share: s.amount,
            total_value: s.total,
            native_currency: asset.currency.clone(),
            native_total_value: s.total,
            fx_missing: false,
        });
    }

    rows.sort_by(|a, b| b.payment_date.cmp(&a.payment_date));

    // Fase 19 — valores na moeda base pela taxa da data de pagamento (a
    // receber, com data futura, usa a cotação mais próxima, a mais recente).
    let currencies: HashSet<String> = rows.iter().map(|r| r.native_currency.clone()).collect();
    let history = load_fx_history(db, &base, &currencies).await?;
    for row in rows.iter_mut() {
        let rate = NaiveDate::parse_from_str(&row.payment_date, "%Y-%m-%d")
            .ok()
            .and_then(|date| history.rate_on(&row.native_currency, &base, date));
        match rate {
            Some(rate) => {
                row.total_value *= rate;
                row.net_total *= rate;
            }
            None => {
                row.fx_missing = true;
                row.total_value = 0.0;
                row.net_total = 0.0;
            }
        }
    }

    let entries: Vec<ProventoEntry> = rows
        .iter()
        .filter(|r| !r.fx_missing)
        .map(|r| ProventoEntry {
            asset_id: r.asset_id,
            payment_date: r.payment_date.clone(),
            net_total: r.net_total,
            received: r.status == "paid",
        })
        .collect();

    let monthly: Vec<ProventosMonthlyBucketView> = bucket_monthly(&entries)
        .into_iter()
        .map(|b| ProventosMonthlyBucketView {
            year_month: b.year_month,
            received: b.received,
            expected: b.expected,
        })
        .collect();

    let since = (Utc::now() - Duration::days(365)).format("%Y-%m-%d").to_string();
    let paid_entries: Vec<&ProventoEntry> = entries.iter().filter(|e| e.received).collect();
    let total_all_time: f64 = paid_entries.iter().map(|e| e.net_total).sum();
    let last_12m: Vec<ProventoEntry> = paid_entries
        .iter()
        .filter(|e| e.payment_date >= since)
        .map(|e| ProventoEntry {
            asset_id: e.asset_id,
            payment_date: e.payment_date.clone(),
            net_total: e.net_total,
            received: e.received,
        })
        .collect();
    let total_12m: f64 = last_12m.iter().map(|e| e.net_total).sum();
    let avg_monthly_12m = total_12m / 12.0;

    let by_asset_totals = by_asset(&last_12m);
    let mut by_asset_12m: Vec<ProventosAssetTotal> = by_asset_totals
        .into_iter()
        .map(|(asset_id, total)| ProventosAssetTotal {
            asset_id,
            ticker: assets_map.get(&asset_id).map(|a| a.ticker.clone()).unwrap_or_default(),
            total,
        })
        .collect();
    by_asset_12m.sort_by(|a, b| b.total.partial_cmp(&a.total).unwrap());

    Ok(ProventosSummary {
        avg_monthly_12m,
        total_12m,
        total_all_time,
        by_asset_12m,
        monthly,
        rows,
        base_currency: base,
    })
}
