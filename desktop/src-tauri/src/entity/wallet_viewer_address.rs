//! `SeaORM` Entity, hand-written to match the generated style (see `storage_settings.rs`).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "wallet_viewer_address")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub address: String,
    pub label: Option<String>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
