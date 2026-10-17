//! `SeaORM` Entity — Fase 18, posição em opção da B3 (comprada ou lançada).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "option_positions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub portfolio_id: i32,
    pub series_ticker: String,
    pub underlying_ticker: String,
    /// `call` ou `put`.
    pub option_type: String,
    #[sea_orm(column_type = "Double")]
    pub strike: f64,
    pub expiration: String,
    /// `long` (comprada) ou `short` (lançada).
    pub side: String,
    #[sea_orm(column_type = "Double")]
    pub quantity: f64,
    /// Prêmio por opção, pago (comprada) ou recebido (lançada).
    #[sea_orm(column_type = "Double")]
    pub premium: f64,
    pub open_date: String,
    #[sea_orm(column_type = "Double", nullable)]
    pub open_fee: Option<f64>,
    pub notes: Option<String>,
    /// `closed`, `expired` ou `exercised`; nulo = posição aberta.
    pub close_kind: Option<String>,
    #[sea_orm(column_type = "Double", nullable)]
    pub close_price: Option<f64>,
    pub close_date: Option<String>,
    #[sea_orm(column_type = "Double", nullable)]
    pub close_fee: Option<f64>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
