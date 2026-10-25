//! `SeaORM` Entity — Fase 24, entidade controlada no exterior (Lei 14.754/2023).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "offshore_entities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub workspace_id: i32,
    pub name: String,
    pub country: String,
    /// Moeda em que a entidade fecha o resultado (ex.: USD).
    pub currency: String,
    /// Participação do usuário, de 0 (exclusive) a 100.
    #[sea_orm(column_type = "Double")]
    pub ownership_percent: f64,
    /// `automatico` (lucro tributado todo 31/12, art. 5º) ou `ativa` (tributada na disponibilização, art. 6º).
    pub regime: String,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
