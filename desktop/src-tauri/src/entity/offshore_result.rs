//! `SeaORM` Entity — Fase 24, resultado do exercício de uma entidade controlada no exterior.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "offshore_results")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub entity_id: i32,
    pub year: i32,
    /// Lucro do exercício na moeda da entidade; negativo = prejuízo.
    #[sea_orm(column_type = "Double")]
    pub profit: f64,
    /// Imposto pago lá fora sobre esse lucro, na moeda da entidade.
    #[sea_orm(column_type = "Double")]
    pub tax_paid_abroad: f64,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
