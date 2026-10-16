//! `SeaORM` Entity — Fase 29, empréstimo concedido a terceiro.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "loans")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub workspace_id: i32,
    pub borrower_id: i32,
    pub bank_account_id: Option<i32>,
    pub general_transaction_id: Option<i32>,
    #[sea_orm(column_type = "Double")]
    pub principal: f64,
    #[sea_orm(column_type = "Double", nullable)]
    pub taxa_mensal_percentual: Option<f64>,
    pub data_inicio: String,
    pub include_in_net_worth: bool,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
