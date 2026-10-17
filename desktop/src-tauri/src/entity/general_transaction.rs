//! `SeaORM` Entity — Fase 12, núcleo de Finanças Gerais.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "general_transactions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub workspace_id: i32,
    pub bank_account_id: i32,
    pub transaction_type: String,
    pub category_id: Option<i32>,
    #[sea_orm(column_type = "Double")]
    pub valor: f64,
    pub transaction_date: String,
    pub notes: Option<String>,
    pub created_at: String,
    // Fase 19 — as duas linhas de uma transferência entre contas compartilham
    // este valor (o `id` da linha de saída); linha comum fica `None`.
    pub transfer_id: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::bank_account::Entity",
        from = "Column::BankAccountId",
        to = "super::bank_account::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    BankAccount,
    #[sea_orm(
        belongs_to = "super::general_transaction_category::Entity",
        from = "Column::CategoryId",
        to = "super::general_transaction_category::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    GeneralTransactionCategory,
    #[sea_orm(
        belongs_to = "super::workspace::Entity",
        from = "Column::WorkspaceId",
        to = "super::workspace::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Workspace,
}

impl Related<super::bank_account::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BankAccount.def()
    }
}

impl Related<super::general_transaction_category::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GeneralTransactionCategory.def()
    }
}

impl Related<super::workspace::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Workspace.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
