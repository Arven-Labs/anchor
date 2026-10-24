//! `SeaORM` Entity — Fase 24, tipo de um fundo de investimento (decide o come-cotas).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "fund_details")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub asset_id: i32,
    /// `longo_prazo`, `curto_prazo` ou `acoes`.
    pub term: String,
    /// Subclasse do informe diário da CVM; `None` = a linha da classe (sem subclasse).
    pub cvm_subclass: Option<String>,
    /// Administradora ou intermediário, em texto livre: agrupa os fundos que compensam prejuízo entre si; `None` = isolado.
    pub administrator: Option<String>,
    /// `aberto` (padrão), `fechado` ou `exclusivo` (Lei 14.754): só os dois últimos têm estoque de rendimento de 2023.
    pub structure: String,
    /// Como o estoque de 2023 foi tributado: `geral` (15%), `opcao_8` (8%) ou nada.
    pub estoque_option: Option<String>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
