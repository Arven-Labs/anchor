//! `SeaORM` Entity, hand-written to match the generated style (see `selic_daily.rs`).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ptax_daily")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub currency: String,
    pub reference_date: String,
    #[sea_orm(column_type = "Double")]
    pub buy: f64,
    #[sea_orm(column_type = "Double")]
    pub sell: f64,
    pub source: String,
    pub fetched_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
