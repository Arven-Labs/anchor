//! `SeaORM` Entity — Fase 24, valor da cota de um fundo numa data, lançado pelo usuário ou lido da CVM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "fund_quota_values")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub asset_id: i32,
    /// `YYYY-MM-DD`.
    pub quota_date: String,
    /// Valor de uma cota, em BRL.
    #[sea_orm(column_type = "Double")]
    pub value: f64,
    /// `manual` (lançado pelo usuário) ou `cvm` (lido do informe diário); a CVM nunca sobrescreve uma manual.
    pub source: String,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
