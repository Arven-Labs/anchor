// Previdência privada: estimativa informativa do IR no resgate, regime com tabela por prazo (ver
// `domain::tax::pension`). Lê as alíquotas de `tax_rules` (editáveis em Settings → Taxes). Não grava nada e não alimenta
// a apuração de IR, o patrimônio nem a Rentabilidade.

use chrono::{NaiveDate, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use crate::commands::tax::{load_rule_set, workspace_portfolio_ids};
use crate::domain::tax::pension_declaration::{year_end_position, Movement, Statement, YearEndPosition};
use crate::entity::{asset_valuations, assets, transactions};
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

/// O que o informe de Bens e Direitos pede de um plano em 31/12 de `year`: grupo e código como o dono digitou (`None` =
/// não informado), custo e saldo.
#[derive(Serialize)]
pub struct PensionDeclaration {
    pub asset_id: i32,
    pub group: Option<String>,
    pub code: Option<String>,
    #[serde(flatten)]
    pub position: YearEndPosition,
}

pub async fn pension_declaration_in(
    db: &DatabaseConnection,
    workspace_id: i32,
    asset_id: i32,
    year: i32,
) -> Result<PensionDeclaration, AppError> {
    let asset = assets::Entity::find_by_id(asset_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {asset_id}")))?;
    if asset.asset_class != "previdencia" {
        return Err(AppError::InvalidInput(format!("asset {asset_id} is not a private pension plan")));
    }
    if !(1990..=2200).contains(&year) {
        return Err(AppError::InvalidInput(format!("invalid year {year}")));
    }
    let portfolio_ids = workspace_portfolio_ids(db, workspace_id).await?;
    let trades = transactions::Entity::find()
        .filter(transactions::Column::AssetId.eq(asset_id))
        .filter(transactions::Column::PortfolioId.is_in(portfolio_ids))
        .all(db)
        .await?;
    let valuations = asset_valuations::Entity::find()
        .filter(asset_valuations::Column::AssetId.eq(asset_id))
        .all(db)
        .await?;
    let movements: Vec<Movement> = trades
        .iter()
        .map(|t| Movement {
            date: &t.transaction_date,
            transaction_type: &t.transaction_type,
            quantity: t.quantity.unwrap_or(0.0),
        })
        .collect();
    let statements: Vec<Statement> =
        valuations.iter().map(|v| Statement { date: &v.valuation_date, value: v.value }).collect();
    Ok(PensionDeclaration {
        asset_id,
        group: asset.dirpf_group,
        code: asset.dirpf_code,
        position: year_end_position(year, &movements, &statements),
    })
}

/// Custo e saldo do plano em 31/12 e a ficha digitada pelo dono: informativo, não grava nada.
#[tauri::command]
pub async fn pension_declaration(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
    asset_id: i32,
    year: i32,
) -> Result<PensionDeclaration, AppError> {
    pension_declaration_in(db.inner(), workspace_id, asset_id, year).await
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
