//! `SeaORM` Entity — evento do capital aplicado numa entidade controlada no exterior (Lei 14.754, art. 7º).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "offshore_capital_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub entity_id: i32,
    /// `aporte`, `venda` (alienação, baixa ou liquidação) ou `devolucao` (devolução de capital).
    pub kind: String,
    /// Data do evento, `AAAA-MM-DD`.
    pub event_date: String,
    /// Valor, na moeda da entidade (aportado, recebido na venda ou devolvido).
    #[sea_orm(column_type = "Double")]
    pub amount: f64,
    /// Só no aporte: o custo em reais declarado (aportes antigos). Nulo = o app converte pela PTAX do dia.
    #[sea_orm(column_type = "Double", nullable)]
    pub brl_cost: Option<f64>,
    /// Só na venda: a fração da participação vendida, de 0 (exclusive) a 1.
    #[sea_orm(column_type = "Double", nullable)]
    pub fraction: Option<f64>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
