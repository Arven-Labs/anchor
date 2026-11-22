// Previdência privada: estimativa informativa do IR no resgate, regime com tabela por prazo (ver
// `domain::tax::pension`). Lê as alíquotas de `tax_rules` (editáveis em Settings → Taxes). Não grava nada e não alimenta
// a apuração de IR, o patrimônio nem a Rentabilidade.

use chrono::{NaiveDate, Utc};
use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::commands::tax::load_rule_set;
use crate::domain::tax::pension::{estimate_regressive, PensionKind, RegressiveEstimate};
use crate::domain::tax::pension_deduction::{estimate_deduction_limit, DeductionLimitEstimate};
use crate::error::AppError;

#[derive(Deserialize)]
pub struct EstimatePensionRedemptionRequest {
    pub kind: PensionKind,
    pub redeemed: f64,
    /// Parcela do resgate que é rendimento (só VGBL; o informe da seguradora traz).
    pub income: Option<f64>,
    pub years_since_contribution: f64,
}

pub async fn estimate_pension_redemption_in(
    db: &DatabaseConnection,
    request: EstimatePensionRedemptionRequest,
    on: NaiveDate,
) -> Result<RegressiveEstimate, AppError> {
    let rules = load_rule_set(db).await?;
    estimate_regressive(&rules, on, request.kind, request.redeemed, request.income, request.years_since_contribution)
}

#[derive(Deserialize)]
pub struct EstimatePensionDeductionLimitRequest {
    /// Total dos rendimentos da base de cálculo da declaração, informado por quem usa.
    pub base: f64,
    /// Aportes ao PGBL no ano, informados por quem usa (o app não grava o tipo do plano).
    pub contributions: f64,
}

pub async fn estimate_pension_deduction_limit_in(
    db: &DatabaseConnection,
    request: EstimatePensionDeductionLimitRequest,
    on: NaiveDate,
) -> Result<DeductionLimitEstimate, AppError> {
    let rules = load_rule_set(db).await?;
    estimate_deduction_limit(&rules, on, request.base, request.contributions)
}

/// Limite de 12% da dedução do PGBL (Lei 9.532, art. 11): informativo, não grava nada.
#[tauri::command]
pub async fn estimate_pension_deduction_limit(
    db: tauri::State<'_, DatabaseConnection>,
    request: EstimatePensionDeductionLimitRequest,
) -> Result<DeductionLimitEstimate, AppError> {
    estimate_pension_deduction_limit_in(db.inner(), request, Utc::now().date_naive()).await
}

#[tauri::command]
pub async fn estimate_pension_redemption(
    db: tauri::State<'_, DatabaseConnection>,
    request: EstimatePensionRedemptionRequest,
) -> Result<RegressiveEstimate, AppError> {
    estimate_pension_redemption_in(db.inner(), request, Utc::now().date_naive()).await
}
