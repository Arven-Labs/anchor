//! `SeaORM` Entity — distribuição de lucros recebida de uma entidade controlada no exterior (Lei 14.754, art. 5º §11).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "offshore_distributions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub entity_id: i32,
    /// Ano do lucro distribuído.
    pub origin_year: i32,
    /// Data do recebimento, `AAAA-MM-DD`.
    pub received_on: String,
    /// Valor recebido, na moeda da entidade.
    #[sea_orm(column_type = "Double")]
    pub amount: f64,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
