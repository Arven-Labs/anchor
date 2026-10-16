//! `SeaORM` Entity — Fase 29, baixa manual de empréstimo concedido.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "loan_repayments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub loan_id: i32,
    pub bank_account_id: Option<i32>,
    pub general_transaction_id: Option<i32>,
    pub data: String,
    #[sea_orm(column_type = "Double")]
    pub valor_recebido: f64,
    #[sea_orm(column_type = "Double")]
    pub valor_baixado: f64,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
