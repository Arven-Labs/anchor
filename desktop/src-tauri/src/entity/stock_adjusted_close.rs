//! Fechamento ajustado a dividendos e desdobramentos (`adjclose` do Yahoo), um por `(ticker, price_date)`.
//! Base do retorno total do backtest; `stock_price_history` continua guardando só o `close`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "stock_adjusted_close")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub ticker: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub price_date: String,
    #[sea_orm(column_type = "Double")]
    pub adj_close_price: f64,
    pub fetched_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
