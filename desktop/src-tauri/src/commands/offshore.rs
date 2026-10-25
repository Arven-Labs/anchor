// Fase 24 — entidades controladas no exterior (Lei 14.754/2023). O usuário cadastra a entidade (país, moeda, participação) e
// informa o resultado de cada exercício; a apuração (`domain::tax::offshore`, chamada por `commands::tax`) converte pela PTAX
// de 31/12. Aqui só mora o cadastro.

use chrono::{Datelike, NaiveDate, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};

use crate::domain::tax::foreign::FIRST_YEAR;
use crate::domain::tax::offshore::{
    offshore_credits, OffshoreCreditRow, OffshoreDistribution, OffshoreDistributionView, OffshoreEntity, OffshoreResult,
};
use crate::entity::{offshore_distribution, offshore_entity, offshore_result};
use crate::error::AppError;

/// Uma entidade com os resultados que o usuário já informou, do ano mais antigo ao mais novo.
#[derive(Serialize)]
pub struct OffshoreEntityView {
    #[serde(flatten)]
    pub entity: offshore_entity::Model,
    pub results: Vec<offshore_result::Model>,
    /// As distribuições recebidas, da mais antiga à mais nova (as de entidade `ativa` são o que gera imposto).
    pub distributions: Vec<offshore_distribution::Model>,
}

#[derive(Deserialize)]
pub struct OffshoreEntityRequest {
    pub name: String,
    pub country: String,
    pub currency: String,
    pub ownership_percent: f64,
    /// `automatico` (padrão) ou `ativa` (art. 6º da Lei 14.754: tributada só na disponibilização do lucro).
    #[serde(default)]
    pub regime: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateOffshoreEntityRequest {
    pub workspace_id: i32,
    #[serde(flatten)]
    pub entity: OffshoreEntityRequest,
}

#[derive(Deserialize)]
pub struct UpdateOffshoreEntityRequest {
    pub id: i32,
    #[serde(flatten)]
    pub entity: OffshoreEntityRequest,
}

#[derive(Deserialize)]
pub struct UpsertOffshoreResultRequest {
    pub entity_id: i32,
    pub year: i32,
    pub profit: f64,
    pub tax_paid_abroad: f64,
}

/// Texto aparado e não vazio.
fn required(value: &str, field: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput(format!("{field} is required")));
    }
    Ok(trimmed.to_string())
}

struct CleanEntity {
    name: String,
    country: String,
    currency: String,
    ownership_percent: f64,
    regime: &'static str,
    notes: Option<String>,
}

pub const REGIME_AUTOMATIC: &str = "automatico";
pub const REGIME_ACTIVE: &str = "ativa";

fn clean_entity(request: OffshoreEntityRequest) -> Result<CleanEntity, AppError> {
    let regime = match request.regime.as_deref().map(str::trim) {
        None | Some("") | Some(REGIME_AUTOMATIC) => REGIME_AUTOMATIC,
        Some(REGIME_ACTIVE) => REGIME_ACTIVE,
        Some(other) => {
            return Err(AppError::InvalidInput(format!("regime must be '{REGIME_AUTOMATIC}' or '{REGIME_ACTIVE}', got '{other}'")))
        }
    };
    let currency = required(&request.currency, "currency")?.to_uppercase();
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(AppError::InvalidInput("currency must be a 3-letter code (e.g. USD)".to_string()));
    }
    if !request.ownership_percent.is_finite() || request.ownership_percent <= 0.0 || request.ownership_percent > 100.0 {
        return Err(AppError::InvalidInput("ownership must be above 0 and at most 100 percent".to_string()));
    }
    Ok(CleanEntity {
        name: required(&request.name, "name")?,
        country: required(&request.country, "country")?,
        currency,
        ownership_percent: request.ownership_percent,
        regime,
        notes: request.notes.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
    })
}

#[tauri::command]
pub async fn list_offshore_entities(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<OffshoreEntityView>, AppError> {
    list_offshore_entities_in(db.inner(), workspace_id).await
}

pub async fn list_offshore_entities_in(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Vec<OffshoreEntityView>, AppError> {
    let entities = offshore_entity::Entity::find()
        .filter(offshore_entity::Column::WorkspaceId.eq(workspace_id))
        .order_by_asc(offshore_entity::Column::Name)
        .all(db)
        .await?;
    let ids: Vec<i32> = entities.iter().map(|e| e.id).collect();
    let (results, distributions) = if ids.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        let results = offshore_result::Entity::find()
            .filter(offshore_result::Column::EntityId.is_in(ids.clone()))
            .order_by_asc(offshore_result::Column::Year)
            .all(db)
            .await?;
        let distributions = offshore_distribution::Entity::find()
            .filter(offshore_distribution::Column::EntityId.is_in(ids))
            .order_by_asc(offshore_distribution::Column::ReceivedOn)
            .all(db)
            .await?;
        (results, distributions)
    };
    Ok(entities
        .into_iter()
        .map(|entity| {
            let results = results.iter().filter(|r| r.entity_id == entity.id).cloned().collect();
            let distributions = distributions.iter().filter(|d| d.entity_id == entity.id).cloned().collect();
            OffshoreEntityView { entity, results, distributions }
        })
        .collect())
}

#[tauri::command]
pub async fn create_offshore_entity(
    db: tauri::State<'_, DatabaseConnection>,
    request: CreateOffshoreEntityRequest,
) -> Result<offshore_entity::Model, AppError> {
    create_offshore_entity_in(db.inner(), request).await
}

pub async fn create_offshore_entity_in(
    db: &DatabaseConnection,
    request: CreateOffshoreEntityRequest,
) -> Result<offshore_entity::Model, AppError> {
    let clean = clean_entity(request.entity)?;
    Ok(offshore_entity::ActiveModel {
        workspace_id: Set(request.workspace_id),
        name: Set(clean.name),
        country: Set(clean.country),
        currency: Set(clean.currency),
        ownership_percent: Set(clean.ownership_percent),
        regime: Set(clean.regime.to_string()),
        notes: Set(clean.notes),
        created_at: Set(Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

#[tauri::command]
pub async fn update_offshore_entity(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpdateOffshoreEntityRequest,
) -> Result<offshore_entity::Model, AppError> {
    update_offshore_entity_in(db.inner(), request).await
}

pub async fn update_offshore_entity_in(
    db: &DatabaseConnection,
    request: UpdateOffshoreEntityRequest,
) -> Result<offshore_entity::Model, AppError> {
    let clean = clean_entity(request.entity)?;
    let existing = offshore_entity::Entity::find_by_id(request.id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("offshore entity {}", request.id)))?;
    let mut active: offshore_entity::ActiveModel = existing.into();
    active.name = Set(clean.name);
    active.country = Set(clean.country);
    active.currency = Set(clean.currency);
    active.ownership_percent = Set(clean.ownership_percent);
    active.regime = Set(clean.regime.to_string());
    active.notes = Set(clean.notes);
    Ok(active.update(db).await?)
}

#[tauri::command]
pub async fn delete_offshore_entity(
    db: tauri::State<'_, DatabaseConnection>,
    entity_id: i32,
) -> Result<(), AppError> {
    // Os resultados saem junto (FK em cascata).
    offshore_entity::Entity::delete_by_id(entity_id).exec(db.inner()).await?;
    Ok(())
}

#[tauri::command]
pub async fn upsert_offshore_result(
    db: tauri::State<'_, DatabaseConnection>,
    request: UpsertOffshoreResultRequest,
) -> Result<offshore_result::Model, AppError> {
    upsert_offshore_result_in(db.inner(), request).await
}

pub async fn upsert_offshore_result_in(
    db: &DatabaseConnection,
    request: UpsertOffshoreResultRequest,
) -> Result<offshore_result::Model, AppError> {
    if request.year < FIRST_YEAR || request.year > 2100 {
        return Err(AppError::InvalidInput(format!("year must be {FIRST_YEAR} or later (Lei 14.754)")));
    }
    if !request.profit.is_finite() {
        return Err(AppError::InvalidInput("profit must be a number".to_string()));
    }
    if !request.tax_paid_abroad.is_finite() || request.tax_paid_abroad < 0.0 {
        return Err(AppError::InvalidInput("tax paid abroad must be zero or more".to_string()));
    }
    offshore_entity::Entity::find_by_id(request.entity_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("offshore entity {}", request.entity_id)))?;

    let existing = offshore_result::Entity::find()
        .filter(offshore_result::Column::EntityId.eq(request.entity_id))
        .filter(offshore_result::Column::Year.eq(request.year))
        .one(db)
        .await?;
    match existing {
        Some(row) => {
            let mut active: offshore_result::ActiveModel = row.into();
            active.profit = Set(request.profit);
            active.tax_paid_abroad = Set(request.tax_paid_abroad);
            Ok(active.update(db).await?)
        }
        None => Ok(offshore_result::ActiveModel {
            entity_id: Set(request.entity_id),
            year: Set(request.year),
            profit: Set(request.profit),
            tax_paid_abroad: Set(request.tax_paid_abroad),
            created_at: Set(Utc::now().to_rfc3339()),
            ..Default::default()
        }
        .insert(db)
        .await?),
    }
}

#[tauri::command]
pub async fn delete_offshore_result(
    db: tauri::State<'_, DatabaseConnection>,
    result_id: i32,
) -> Result<(), AppError> {
    offshore_result::Entity::delete_by_id(result_id).exec(db.inner()).await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct AddOffshoreDistributionRequest {
    pub entity_id: i32,
    /// Ano do lucro distribuído. Ignorado (vale o ano do recebimento) para entidade `ativa`, que não tem crédito por ano.
    #[serde(default)]
    pub origin_year: i32,
    /// `AAAA-MM-DD`.
    pub received_on: String,
    /// Valor recebido, na moeda da entidade.
    pub amount: f64,
}

#[tauri::command]
pub async fn add_offshore_distribution(
    db: tauri::State<'_, DatabaseConnection>,
    request: AddOffshoreDistributionRequest,
) -> Result<offshore_distribution::Model, AppError> {
    add_offshore_distribution_in(db.inner(), request).await
}

/// Lança uma distribuição de lucros de uma controlada (art. 5º §11). O lucro só estava tributado em 31/12 do ano de origem,
/// então o recebimento não pode ser anterior a essa data.
pub async fn add_offshore_distribution_in(
    db: &DatabaseConnection,
    request: AddOffshoreDistributionRequest,
) -> Result<offshore_distribution::Model, AppError> {
    let entity = offshore_entity::Entity::find_by_id(request.entity_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("offshore entity {}", request.entity_id)))?;
    let active = entity.regime == REGIME_ACTIVE;
    if !active && (request.origin_year < FIRST_YEAR || request.origin_year > 2100) {
        return Err(AppError::InvalidInput(format!("origin year must be {FIRST_YEAR} or later (Lei 14.754)")));
    }
    if !request.amount.is_finite() || request.amount <= 0.0 {
        return Err(AppError::InvalidInput("amount must be above zero".to_string()));
    }
    let received_on = NaiveDate::parse_from_str(&request.received_on, "%Y-%m-%d")
        .ok()
        .filter(|d| request.received_on.len() == 10 && d.year() >= 1900)
        .ok_or_else(|| AppError::InvalidInput(format!("received_on '{}' is not a date (YYYY-MM-DD)", request.received_on)))?;
    // Entidade ativa (art. 6º): o imposto é do ano do recebimento, sem ano de origem; a lei vale a partir de 2024.
    let origin_year = if active {
        if received_on.year() < FIRST_YEAR {
            return Err(AppError::InvalidInput(format!("the distribution must be received in {FIRST_YEAR} or later (Lei 14.754)")));
        }
        received_on.year()
    } else {
        request.origin_year
    };
    let year_end = NaiveDate::from_ymd_opt(origin_year, 12, 31)
        .ok_or_else(|| AppError::InvalidInput(format!("invalid year {origin_year}")))?;
    if !active && received_on < year_end {
        return Err(AppError::InvalidInput(format!(
            "the profit of {origin_year} was only taxed on 31/12/{origin_year}: the distribution cannot be received before that"
        )));
    }
    Ok(offshore_distribution::ActiveModel {
        entity_id: Set(request.entity_id),
        origin_year: Set(origin_year),
        received_on: Set(request.received_on),
        amount: Set(request.amount),
        created_at: Set(Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

#[tauri::command]
pub async fn delete_offshore_distribution(
    db: tauri::State<'_, DatabaseConnection>,
    distribution_id: i32,
) -> Result<(), AppError> {
    offshore_distribution::Entity::delete_by_id(distribution_id).exec(db.inner()).await?;
    Ok(())
}

/// Os créditos de dividendo a receber das controladas (ficha Bens e Direitos) e o que cada distribuição reduziu.
#[derive(Serialize)]
pub struct OffshoreCredits {
    pub rows: Vec<OffshoreCreditRow>,
    pub distributions: Vec<OffshoreDistributionView>,
}

#[tauri::command]
pub async fn get_offshore_credits(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<OffshoreCredits, AppError> {
    offshore_credits_in(db.inner(), workspace_id).await
}

pub async fn offshore_credits_in(db: &DatabaseConnection, workspace_id: i32) -> Result<OffshoreCredits, AppError> {
    let assessment = crate::commands::tax::assess_workspace(db, workspace_id).await?;
    // Entidade ativa (art. 6º) não tem crédito de dividendo: as distribuições dela são tributadas, não reduzem crédito.
    let distributions = load_distributions(db, workspace_id, false).await?;
    let (rows, views) = offshore_credits(&assessment.offshore_years, &distributions, Utc::now().date_naive());
    Ok(OffshoreCredits { rows, distributions: views })
}

/// As distribuições do Workspace, das entidades ativas (`active`, art. 6º) ou das demais, da mais antiga à mais nova.
pub(crate) async fn load_distributions(
    db: &DatabaseConnection,
    workspace_id: i32,
    active: bool,
) -> Result<Vec<OffshoreDistribution>, AppError> {
    let regime = offshore_entity::Column::Regime;
    let entity_ids: Vec<i32> = offshore_entity::Entity::find()
        .filter(offshore_entity::Column::WorkspaceId.eq(workspace_id))
        .filter(if active { regime.eq(REGIME_ACTIVE) } else { regime.ne(REGIME_ACTIVE) })
        .all(db)
        .await?
        .into_iter()
        .map(|e| e.id)
        .collect();
    if entity_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = offshore_distribution::Entity::find()
        .filter(offshore_distribution::Column::EntityId.is_in(entity_ids))
        .order_by_asc(offshore_distribution::Column::ReceivedOn)
        .all(db)
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(OffshoreDistribution {
                id: row.id,
                entity_id: row.entity_id,
                origin_year: row.origin_year,
                received_on: NaiveDate::parse_from_str(&row.received_on, "%Y-%m-%d")
                    .map_err(|_| AppError::InvalidInput(format!("invalid date '{}'", row.received_on)))?,
                amount: row.amount,
            })
        })
        .collect()
}

/// As entidades e os resultados do Workspace no formato da apuração.
pub(crate) async fn load_inputs(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<(Vec<OffshoreEntity>, Vec<OffshoreResult>), AppError> {
    let views = list_offshore_entities_in(db, workspace_id).await?;
    let mut entities = Vec::with_capacity(views.len());
    let mut results = Vec::new();
    for view in views {
        entities.push(OffshoreEntity {
            id: view.entity.id,
            name: view.entity.name,
            country: view.entity.country,
            currency: view.entity.currency,
            ownership_percent: view.entity.ownership_percent,
            active_regime: view.entity.regime == REGIME_ACTIVE,
        });
        results.extend(view.results.into_iter().map(|r| OffshoreResult {
            entity_id: r.entity_id,
            year: r.year,
            profit: r.profit,
            tax_paid_abroad: r.tax_paid_abroad,
        }));
    }
    Ok((entities, results))
}
