// Fase 35, entrega 6 — contagem dos pendentes de um Workspace para o selo da aba Pending.
// Soma as três filas que a aba mostra: lançamentos de carteira, linhas de extrato e
// rendimentos (`import_pending`) e sugestões de provento ainda acionáveis.
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};
use serde::Serialize;

use crate::commands::fixed_income_io::KIND_YIELD;
use crate::commands::portfolio_io::{KIND_TRANSACTION, STATUS_PENDING};
use crate::commands::statement_io::KIND_BANK;
use crate::entity::{import_pending, portfolio, suggested_dividends};
use crate::error::AppError;

/// Status de uma sugestão de provento que ainda espera decisão (os mesmos da tela de proventos).
const ACTIONABLE_DIVIDEND_STATUSES: [&str; 3] = ["pending", "matched", "divergent"];

#[derive(Debug, Serialize)]
pub struct PendingCounts {
    pub transactions: u64,
    pub statements: u64,
    pub dividends: u64,
    pub total: u64,
}

pub async fn count_pending_in(db: &DatabaseConnection, workspace_id: i32) -> Result<PendingCounts, AppError> {
    let queue = |kinds: Vec<&str>| {
        import_pending::Entity::find()
            .filter(import_pending::Column::WorkspaceId.eq(workspace_id))
            .filter(import_pending::Column::Kind.is_in(kinds))
            .filter(import_pending::Column::Status.eq(STATUS_PENDING))
            .count(db)
    };
    let transactions = queue(vec![KIND_TRANSACTION]).await?;
    let statements = queue(vec![KIND_BANK, KIND_YIELD]).await?;

    let portfolio_ids = portfolio::Entity::find()
        .select_only()
        .column(portfolio::Column::Id)
        .filter(portfolio::Column::WorkspaceId.eq(workspace_id))
        .into_tuple::<i32>()
        .all(db)
        .await?;
    let dividends = suggested_dividends::Entity::find()
        .filter(suggested_dividends::Column::PortfolioId.is_in(portfolio_ids))
        .filter(suggested_dividends::Column::Status.is_in(ACTIONABLE_DIVIDEND_STATUSES))
        .count(db)
        .await?;

    Ok(PendingCounts { transactions, statements, dividends, total: transactions + statements + dividends })
}

#[tauri::command]
pub async fn count_pending(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<PendingCounts, AppError> {
    count_pending_in(db.inner(), workspace_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{asset, memory_db, portfolio, workspace};
    use sea_orm::{ActiveModelTrait, Set};

    async fn queue_row(db: &DatabaseConnection, ws: i32, pf: Option<i32>, kind: &str, status: &str) {
        import_pending::ActiveModel {
            workspace_id: Set(ws),
            portfolio_id: Set(pf),
            kind: Set(kind.to_string()),
            source: Set("test".to_string()),
            payload: Set("{}".to_string()),
            status: Set(status.to_string()),
            created_at: Set("2026-10-19".to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
    }

    async fn dividend(db: &DatabaseConnection, pf: i32, asset_id: i32, status: &str) {
        suggested_dividends::ActiveModel {
            portfolio_id: Set(pf),
            asset_id: Set(asset_id),
            payment_date: Set("2026-10-20".to_string()),
            amount: Set(1.0),
            quantity: Set(10.0),
            total: Set(10.0),
            status: Set(status.to_string()),
            source: Set("manual".to_string()),
            payment_type: Set("dividendo".to_string()),
            created_at: Set("2026-10-19".to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn counts_each_queue_by_workspace_and_ignores_settled_items() {
        let db = memory_db().await;
        let ws = workspace(&db, "A").await;
        let other = workspace(&db, "B").await;
        let pf = portfolio(&db, ws.id, "Main").await;
        let other_pf = portfolio(&db, other.id, "Other").await;
        let ticker = asset(&db, "ABCD3", "stock").await;

        queue_row(&db, ws.id, Some(pf.id), KIND_TRANSACTION, "pending").await;
        queue_row(&db, ws.id, Some(pf.id), KIND_TRANSACTION, "pending").await;
        queue_row(&db, ws.id, Some(pf.id), KIND_TRANSACTION, "discarded").await;
        queue_row(&db, ws.id, None, KIND_BANK, "pending").await;
        queue_row(&db, ws.id, None, KIND_YIELD, "pending").await;
        queue_row(&db, ws.id, None, KIND_BANK, "confirmed").await;
        queue_row(&db, other.id, Some(other_pf.id), KIND_TRANSACTION, "pending").await;

        dividend(&db, pf.id, ticker.id, "pending").await;
        dividend(&db, pf.id, ticker.id, "matched").await;
        dividend(&db, pf.id, ticker.id, "divergent").await;
        dividend(&db, pf.id, ticker.id, "confirmed").await;
        dividend(&db, pf.id, ticker.id, "discarded").await;
        dividend(&db, other_pf.id, ticker.id, "pending").await;

        let counts = count_pending_in(&db, ws.id).await.unwrap();
        assert_eq!(counts.transactions, 2);
        assert_eq!(counts.statements, 2);
        assert_eq!(counts.dividends, 3);
        assert_eq!(counts.total, 7);

        let empty = count_pending_in(&db, workspace(&db, "C").await.id).await.unwrap();
        assert_eq!(empty.total, 0);
    }
}
