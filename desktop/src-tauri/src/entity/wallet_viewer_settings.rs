//! `SeaORM` Entity, hand-written to match the generated style (see `storage_settings.rs`).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "wallet_viewer_settings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub enabled: bool,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
