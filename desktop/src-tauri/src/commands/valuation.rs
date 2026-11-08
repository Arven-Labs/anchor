use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    Unchanged,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::domain::{banks, bazin, dcf, gordon, graham, projected_ceiling, rim, rnav};
use crate::entity::{
    banks_inputs, bazin_inputs, dcf_inputs, gordon_inputs, graham_inputs,
    projected_ceiling_inputs, rim_inputs, rnav_inputs, valuation,
};
use crate::error::AppError;

// Sessão 56 — resposta compartilhada pelas 8 `calculate_<model>` (agora só
// preview, sem insert — ver `save_<model>` em cada `commands/<model>.rs` pra
// a versão que persiste). O `domain::valuation_outcome::ValuationOutcome`
// (o mesmo pros 8 modelos) já tem exatamente este shape
// (`fair_price`/`safety_margin`/`verdict`); o `Verdict` é convertido pra
// `String` aqui, no ponto de saída de cada comando, em vez de o `domain`
// precisar derivar `Serialize`.
#[derive(Serialize)]
pub struct ValuationOutcomeResponse {
    pub fair_price: f64,
    pub safety_margin: f64,
    pub verdict: String,
}

#[tauri::command]
pub async fn list_valuations(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<valuation::Model>, AppError> {
    let valuations = valuation::Entity::find()
        .order_by_desc(valuation::Column::UpdatedAt)
        .all(db.inner())
        .await?;

    Ok(valuations)
}

// The assumptions behind each saved calculation live in a table specific to
// its model (see Fase 1 — "valuation compartilhada + inputs por modelo").
// This command resolves which one to query from `valuation.model` and hands
// back the row as a plain JSON object — the frontend already knows the field
// shape per model (same labels used in the calculator forms), so there's no
// need for a Rust-side enum with one variant per model just to carry it over.
#[tauri::command]
pub async fn get_valuation_inputs(
    db: tauri::State<'_, DatabaseConnection>,
    valuation_id: i32,
    model: String,
) -> Result<JsonValue, AppError> {
    let not_found = || AppError::NotFound(format!("inputs for valuation {valuation_id}"));

    let value = match model.as_str() {
        "bazin" => serde_json::to_value(
            bazin_inputs::Entity::find()
                .filter(bazin_inputs::Column::ValuationId.eq(valuation_id))
                .one(db.inner())
                .await?
                .ok_or_else(not_found)?,
        ),
        "graham" => serde_json::to_value(
            graham_inputs::Entity::find()
                .filter(graham_inputs::Column::ValuationId.eq(valuation_id))
                .one(db.inner())
                .await?
                .ok_or_else(not_found)?,
        ),
        "gordon" => serde_json::to_value(
            gordon_inputs::Entity::find()
                .filter(gordon_inputs::Column::ValuationId.eq(valuation_id))
                .one(db.inner())
                .await?
                .ok_or_else(not_found)?,
        ),
        "dcf" => serde_json::to_value(
            dcf_inputs::Entity::find()
                .filter(dcf_inputs::Column::ValuationId.eq(valuation_id))
                .one(db.inner())
                .await?
                .ok_or_else(not_found)?,
        ),
        "banks" => serde_json::to_value(
            banks_inputs::Entity::find()
                .filter(banks_inputs::Column::ValuationId.eq(valuation_id))
                .one(db.inner())
                .await?
                .ok_or_else(not_found)?,
        ),
        "rim" => serde_json::to_value(
            rim_inputs::Entity::find()
                .filter(rim_inputs::Column::ValuationId.eq(valuation_id))
                .one(db.inner())
                .await?
                .ok_or_else(not_found)?,
        ),
        "rnav" => serde_json::to_value(
            rnav_inputs::Entity::find()
                .filter(rnav_inputs::Column::ValuationId.eq(valuation_id))
                .one(db.inner())
                .await?
                .ok_or_else(not_found)?,
        ),
        "projected_ceiling" => serde_json::to_value(
            projected_ceiling_inputs::Entity::find()
                .filter(projected_ceiling_inputs::Column::ValuationId.eq(valuation_id))
                .one(db.inner())
                .await?
                .ok_or_else(not_found)?,
        ),
        _ => return Err(AppError::NotFound(format!("model '{model}'"))),
    };

    value.map_err(|err| AppError::NotFound(err.to_string()))
}

#[derive(Deserialize)]
pub struct UpdateValuationRequest {
    pub valuation_id: i32,
    pub ticker: String,
    pub reference_year: i32,
    pub current_price: f64,
    pub model: String,
    pub inputs: JsonValue,
}

// Edits a saved calculation in place (fixing a mistake) instead of adding a
// new row — different from the 7 `calculate_*` commands, which always
// insert. Recalculates through the same pure `domain::*::calculate()`
// function each model already uses, so there's no duplicated math here.
#[tauri::command]
pub async fn update_valuation(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateValuationRequest,
) -> Result<valuation::Model, AppError> {
    let invalid = |err: serde_json::Error| AppError::InvalidInput(err.to_string());

    let (fair_price, safety_margin, verdict) = match request.model.as_str() {
        "bazin" => {
            let inputs: bazin::BazinInputs =
                serde_json::from_value(request.inputs.clone()).map_err(invalid)?;
            let outcome = bazin::calculate(&inputs, request.current_price)?;
            bazin_inputs::Entity::update_many()
                .set(bazin_inputs::ActiveModel {
                    average_dividend: Set(inputs.average_dividend),
                    desired_yield: Set(inputs.desired_yield),
                    ..Default::default()
                })
                .filter(bazin_inputs::Column::ValuationId.eq(request.valuation_id))
                .exec(db.inner())
                .await?;
            (
                outcome.fair_price,
                outcome.safety_margin,
                outcome.verdict.as_str().to_string(),
            )
        }
        "graham" => {
            let inputs: graham::GrahamInputs =
                serde_json::from_value(request.inputs.clone()).map_err(invalid)?;
            let outcome = graham::calculate(&inputs, request.current_price)?;
            graham_inputs::Entity::update_many()
                .set(graham_inputs::ActiveModel {
                    eps: Set(inputs.eps),
                    book_value_per_share: Set(inputs.book_value_per_share),
                    ..Default::default()
                })
                .filter(graham_inputs::Column::ValuationId.eq(request.valuation_id))
                .exec(db.inner())
                .await?;
            (
                outcome.fair_price,
                outcome.safety_margin,
                outcome.verdict.as_str().to_string(),
            )
        }
        "gordon" => {
            let inputs: gordon::GordonInputs =
                serde_json::from_value(request.inputs.clone()).map_err(invalid)?;
            let outcome = gordon::calculate(&inputs, request.current_price)?;
            gordon_inputs::Entity::update_many()
                .set(gordon_inputs::ActiveModel {
                    current_dividend: Set(inputs.current_dividend),
                    expected_growth: Set(inputs.expected_growth),
                    ke: Set(inputs.ke),
                    ..Default::default()
                })
                .filter(gordon_inputs::Column::ValuationId.eq(request.valuation_id))
                .exec(db.inner())
                .await?;
            (
                outcome.fair_price,
                outcome.safety_margin,
                outcome.verdict.as_str().to_string(),
            )
        }
        "dcf" => {
            let inputs: dcf::DcfInputs =
                serde_json::from_value(request.inputs.clone()).map_err(invalid)?;
            let outcome = dcf::calculate(&inputs, request.current_price)?;
            dcf_inputs::Entity::update_many()
                .set(dcf_inputs::ActiveModel {
                    ebit: Set(inputs.ebit),
                    tax_rate: Set(inputs.tax_rate),
                    depreciation_amortization: Set(inputs.depreciation_amortization),
                    capex: Set(inputs.capex),
                    nwc_change: Set(inputs.nwc_change),
                    total_debt: Set(inputs.total_debt),
                    cash: Set(inputs.cash),
                    shares_outstanding: Set(inputs.shares_outstanding),
                    beta: Set(inputs.beta),
                    risk_free_rate: Set(inputs.risk_free_rate),
                    market_risk_premium: Set(inputs.market_risk_premium),
                    kd: Set(inputs.kd),
                    perpetuity_growth: Set(inputs.perpetuity_growth),
                    ..Default::default()
                })
                .filter(dcf_inputs::Column::ValuationId.eq(request.valuation_id))
                .exec(db.inner())
                .await?;
            (
                outcome.fair_price,
                outcome.safety_margin,
                outcome.verdict.as_str().to_string(),
            )
        }
        "banks" => {
            let inputs: banks::BanksInputs =
                serde_json::from_value(request.inputs.clone()).map_err(invalid)?;
            let outcome = banks::calculate(&inputs, request.current_price)?;
            banks_inputs::Entity::update_many()
                .set(banks_inputs::ActiveModel {
                    book_value_per_share: Set(inputs.book_value_per_share),
                    roe: Set(inputs.roe),
                    payout: Set(inputs.payout),
                    ke: Set(inputs.ke),
                    ..Default::default()
                })
                .filter(banks_inputs::Column::ValuationId.eq(request.valuation_id))
                .exec(db.inner())
                .await?;
            (
                outcome.fair_price,
                outcome.safety_margin,
                outcome.verdict.as_str().to_string(),
            )
        }
        "rim" => {
            let inputs: rim::RimInputs =
                serde_json::from_value(request.inputs.clone()).map_err(invalid)?;
            let outcome = rim::calculate(&inputs, request.current_price)?;
            rim_inputs::Entity::update_many()
                .set(rim_inputs::ActiveModel {
                    book_value_per_share: Set(inputs.book_value_per_share),
                    roe_current: Set(inputs.roe_current),
                    payout: Set(inputs.payout),
                    ke: Set(inputs.ke),
                    fade_years: Set(inputs.fade_years),
                    ..Default::default()
                })
                .filter(rim_inputs::Column::ValuationId.eq(request.valuation_id))
                .exec(db.inner())
                .await?;
            (
                outcome.fair_price,
                outcome.safety_margin,
                outcome.verdict.as_str().to_string(),
            )
        }
        "rnav" => {
            let inputs: rnav::RnavInputs =
                serde_json::from_value(request.inputs.clone()).map_err(invalid)?;
            let outcome = rnav::calculate(&inputs, request.current_price)?;
            rnav_inputs::Entity::update_many()
                .set(rnav_inputs::ActiveModel {
                    landbank: Set(inputs.landbank),
                    inventory_at_market_value: Set(inputs.inventory_at_market_value),
                    net_cash: Set(inputs.net_cash),
                    shares_outstanding: Set(inputs.shares_outstanding),
                    ..Default::default()
                })
                .filter(rnav_inputs::Column::ValuationId.eq(request.valuation_id))
                .exec(db.inner())
                .await?;
            (
                outcome.fair_price,
                outcome.safety_margin,
                outcome.verdict.as_str().to_string(),
            )
        }
        "projected_ceiling" => {
            let inputs: projected_ceiling::ProjectedCeilingInputs =
                serde_json::from_value(request.inputs.clone()).map_err(invalid)?;
            let outcome = projected_ceiling::calculate(&inputs, request.current_price)?;
            projected_ceiling_inputs::Entity::update_many()
                .set(projected_ceiling_inputs::ActiveModel {
                    current_dividend: Set(inputs.current_dividend),
                    expected_growth: Set(inputs.expected_growth),
                    projection_years: Set(inputs.projection_years),
                    desired_yield: Set(inputs.desired_yield),
                    ke: Set(inputs.ke),
                    ..Default::default()
                })
                .filter(projected_ceiling_inputs::Column::ValuationId.eq(request.valuation_id))
                .exec(db.inner())
                .await?;
            (
                outcome.fair_price,
                outcome.safety_margin,
                outcome.verdict.as_str().to_string(),
            )
        }
        other => return Err(AppError::NotFound(format!("model '{other}'"))),
    };

    let updated = valuation::ActiveModel {
        id: Unchanged(request.valuation_id),
        ticker: Set(request.ticker),
        reference_year: Set(request.reference_year),
        current_price: Set(request.current_price),
        fair_price: Set(Some(fair_price)),
        safety_margin: Set(Some(safety_margin)),
        verdict: Set(Some(verdict)),
        updated_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .update(db.inner())
    .await?;

    Ok(updated)
}

#[tauri::command]
pub async fn delete_valuation(
    db: tauri::State<'_, DatabaseConnection>,
    valuation_id: i32,
) -> Result<(), AppError> {
    valuation::Entity::delete_by_id(valuation_id)
        .exec(db.inner())
        .await?;

    Ok(())
}
