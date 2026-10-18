// Fase 35, entrega 10 — estimativa do rendimento de cada resgate do porquinho que a Movimentação da B3 não explica.
// Os fluxos vêm do ledger da conta (`transferencia_entrada` = aplicação, `transferencia_saida` = o que o banco
// pagou no resgate), o modelo é `domain::porquinho_yield` (CDI pela Selic meta, PEPS, IOF e IR) e cada resgate com
// rendimento vira um item da fila de rendimentos (`kind = fixed_income_yield`), revisado no mesmo popup do da B3.
use std::collections::{BTreeMap, HashSet};

use chrono::NaiveDate;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::commands::balance_reconciliation::RECONCILIATION_NOTE_PREFIX;
use crate::commands::fixed_income_io::{stage_yield_parsed, ProposedYield, YieldPayload, KIND_YIELD};
use crate::commands::tax::load_rule_set;
use crate::domain::porquinho_yield::{estimate_redemptions, Leg, LegKind};
use crate::domain::tax::rules::fixed_income_ir_key;
use crate::entity::{bank_account, general_transaction, import_pending};
use crate::error::AppError;
use crate::finance_api::selic_store;

const SOURCE: &str = "porquinho_estimate";
/// Preço unitário de referência do porquinho na B3 (R$ 0,01 a cota): só para dar uma quantidade de cotas ao item.
const NOTIONAL_UNIT_PRICE: f64 = 0.01;
const MAX_PCT_OF_CDI: f64 = 2.0;

#[derive(Deserialize)]
pub struct EstimateRequest {
    pub bank_account_id: i32,
    /// Fração do CDI que o título rende (1,0 = 100%).
    pub pct_of_cdi: f64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct EstimateResult {
    pub redemptions: usize,
    pub staged: usize,
    /// Resgates de um dia que já tem rendimento lançado ou na fila (a B3 ou uma estimativa anterior).
    pub skipped_covered_day: usize,
    /// Rendimento estimado abaixo de R$ 0,01 (por exemplo, resgate no mesmo dia da aplicação).
    pub skipped_no_yield: usize,
    pub duplicates: usize,
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn parse_day(date: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|e| AppError::InvalidInput(format!("data inválida '{date}': {e}")))
}

/// Dias da conta que já têm rendimento: receita lançada (menos as da conciliação pelo saldo, que são de outro tipo)
/// e itens de rendimento na fila, de qualquer status.
async fn days_with_yield(db: &DatabaseConnection, account_id: i32) -> Result<HashSet<String>, AppError> {
    let mut days = HashSet::new();
    let ledger = general_transaction::Entity::find()
        .filter(general_transaction::Column::BankAccountId.eq(account_id))
        .filter(general_transaction::Column::TransactionType.eq("receita"))
        .all(db)
        .await?;
    for tx in ledger {
        if !tx.notes.as_deref().is_some_and(|n| n.starts_with(RECONCILIATION_NOTE_PREFIX)) {
            days.insert(tx.transaction_date);
        }
    }
    let queued = import_pending::Entity::find()
        .filter(import_pending::Column::BankAccountId.eq(account_id))
        .filter(import_pending::Column::Kind.eq(KIND_YIELD))
        .all(db)
        .await?;
    for row in queued {
        if let Ok(payload) = serde_json::from_str::<YieldPayload>(&row.payload) {
            days.insert(payload.proposed.transaction_date);
        }
    }
    Ok(days)
}

/// Fluxos da conta em ordem: por data e, no mesmo dia, as aplicações antes dos resgates.
async fn account_legs(db: &DatabaseConnection, account_id: i32) -> Result<Vec<Leg>, AppError> {
    let rows = general_transaction::Entity::find()
        .filter(general_transaction::Column::BankAccountId.eq(account_id))
        .filter(general_transaction::Column::TransactionType.is_in(["transferencia_entrada", "transferencia_saida"]))
        .order_by_asc(general_transaction::Column::TransactionDate)
        .order_by_asc(general_transaction::Column::Id)
        .all(db)
        .await?;
    let mut legs = Vec::with_capacity(rows.len());
    for row in rows {
        let kind = if row.transaction_type == "transferencia_entrada" { LegKind::Application } else { LegKind::Redemption };
        legs.push(Leg { date: parse_day(&row.transaction_date)?, kind, amount: row.valor });
    }
    legs.sort_by_key(|leg| (leg.date, leg.kind == LegKind::Redemption));
    Ok(legs)
}

pub async fn stage_porquinho_yield_estimate_in(
    db: &DatabaseConnection,
    request: EstimateRequest,
) -> Result<EstimateResult, AppError> {
    if !request.pct_of_cdi.is_finite() || request.pct_of_cdi <= 0.0 || request.pct_of_cdi > MAX_PCT_OF_CDI {
        return Err(AppError::InvalidInput(format!("the percentage of the CDI must be between 0 and {:.0}%", MAX_PCT_OF_CDI * 100.0)));
    }
    let account = bank_account::Entity::find_by_id(request.bank_account_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("bank account {}", request.bank_account_id)))?;

    let legs = account_legs(db, account.id).await?;
    let redemptions = legs.iter().filter(|l| l.kind == LegKind::Redemption).count();
    if redemptions == 0 {
        return Ok(EstimateResult { redemptions: 0, staged: 0, skipped_covered_day: 0, skipped_no_yield: 0, duplicates: 0 });
    }

    // Série inteira da Selic meta, ordenada: busca no BCB se faltar e serve a gravada se a fonte estiver fora do ar.
    let mut selic = selic_store::load(db, "meta").await?.points;
    selic.sort_by_key(|(date, _)| *date);

    let rules = load_rule_set(db).await?;
    let ir_key = fixed_income_ir_key("cdb").expect("cdb tem regra de IR");
    let ir_rate_at = |date: NaiveDate| rules.get(ir_key, date);
    let estimates = estimate_redemptions(&legs, &selic, request.pct_of_cdi, ir_rate_at)?;

    let covered_days = days_with_yield(db, account.id).await?;
    let mut paid_by_day: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for e in &estimates {
        *paid_by_day.entry(e.date).or_insert(0.0) += e.paid;
    }

    let mut result = EstimateResult { redemptions, staged: 0, skipped_covered_day: 0, skipped_no_yield: 0, duplicates: 0 };
    let mut proposals = Vec::new();
    for e in &estimates {
        let day = e.date.format("%Y-%m-%d").to_string();
        if covered_days.contains(&day) {
            result.skipped_covered_day += 1;
            continue;
        }
        if e.gross_yield < 0.01 {
            result.skipped_no_yield += 1;
            continue;
        }
        let ir_rate = rules.get(ir_key, e.date)?;
        proposals.push(Ok(ProposedYield {
            line: e.leg_index + 1,
            transaction_date: day,
            instrument: "cdb".to_string(),
            code: "Porquinho".to_string(),
            quantity: (e.principal / NOTIONAL_UNIT_PRICE).round(),
            principal: e.principal,
            gross_value: round2(e.principal + e.gross_yield),
            gross_yield: e.gross_yield,
            iof_amount: e.iof,
            holding_days: e.holding_days,
            iof_basis: if e.fully_covered { "full" } else { "partial" }.to_string(),
            ir_rate,
            ir_amount: e.ir,
            net_yield: e.net_yield,
            base_unit_price: NOTIONAL_UNIT_PRICE,
            base_price_source: "estimate".to_string(),
            day_derived_total: round2(paid_by_day[&e.date]),
            estimate_pct_of_cdi: Some(request.pct_of_cdi),
        }));
    }

    let staged = stage_yield_parsed(db, account.workspace_id, account.id, SOURCE, proposals).await?;
    result.staged = staged.staged;
    result.duplicates = staged.duplicates;
    Ok(result)
}

#[tauri::command]
pub async fn stage_porquinho_yield_estimate(
    db: tauri::State<'_, DatabaseConnection>,
    request: EstimateRequest,
) -> Result<EstimateResult, AppError> {
    stage_porquinho_yield_estimate_in(db.inner(), request).await
}
