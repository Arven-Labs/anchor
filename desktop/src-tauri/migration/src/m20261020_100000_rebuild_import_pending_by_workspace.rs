use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Fase 35, entrega 3 — a fila de pendentes passa a ser do Workspace: `portfolio_id`
/// vira opcional (itens de carteira) e ganham vez as contas do extrato bancário
/// (`bank_account_id` e, numa transferência com o porquinho, `counterpart_account_id`).
/// O SQLite não muda a nulabilidade de uma coluna, então a tabela é reconstruída;
/// as linhas antigas ganham o Workspace pelo join com a carteira.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE import_pending_new (
                id integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                workspace_id integer NOT NULL,
                portfolio_id integer NULL,
                bank_account_id integer NULL,
                counterpart_account_id integer NULL,
                kind varchar NOT NULL,
                source varchar NOT NULL,
                payload text NOT NULL,
                status varchar NOT NULL,
                created_at varchar NOT NULL,
                FOREIGN KEY (workspace_id) REFERENCES workspace (id) ON DELETE CASCADE,
                FOREIGN KEY (portfolio_id) REFERENCES portfolio (id) ON DELETE CASCADE,
                FOREIGN KEY (bank_account_id) REFERENCES bank_accounts (id) ON DELETE CASCADE,
                FOREIGN KEY (counterpart_account_id) REFERENCES bank_accounts (id) ON DELETE CASCADE
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO import_pending_new (id, workspace_id, portfolio_id, kind, source, payload, status, created_at)
             SELECT p.id, f.workspace_id, p.portfolio_id, p.kind, p.source, p.payload, p.status, p.created_at
             FROM import_pending p JOIN portfolio f ON f.id = p.portfolio_id",
        )
        .await?;
        db.execute_unprepared("DROP TABLE import_pending").await?;
        db.execute_unprepared("ALTER TABLE import_pending_new RENAME TO import_pending").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE import_pending_old (
                id integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                portfolio_id integer NOT NULL,
                kind varchar NOT NULL,
                source varchar NOT NULL,
                payload text NOT NULL,
                status varchar NOT NULL,
                created_at varchar NOT NULL,
                FOREIGN KEY (portfolio_id) REFERENCES portfolio (id) ON DELETE CASCADE
            )",
        )
        .await?;
        // Itens de extrato não têm carteira e não cabem no formato antigo.
        db.execute_unprepared(
            "INSERT INTO import_pending_old (id, portfolio_id, kind, source, payload, status, created_at)
             SELECT id, portfolio_id, kind, source, payload, status, created_at
             FROM import_pending WHERE portfolio_id IS NOT NULL",
        )
        .await?;
        db.execute_unprepared("DROP TABLE import_pending").await?;
        db.execute_unprepared("ALTER TABLE import_pending_old RENAME TO import_pending").await?;
        Ok(())
    }
}
