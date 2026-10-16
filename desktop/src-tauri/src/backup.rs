// Fase 16 — Backup do Workspace (ideia registrada na Sessão 96, ver PHASE.md). Export/import
// estruturado (JSON) de todo o schema SQLite via introspecção (`sqlite_master`/`PRAGMA
// table_info`), em vez de código específico por entidade — o schema já tem mais de 55 tabelas
// (`entity/`) e cresce a cada fase nova, então uma implementação genérica cobre qualquer tabela
// presente ou futura sem precisar ser tocada de novo. Anexos (Fase 15) entram via
// `StorageProvider::export_all`/`import_all`, já existentes.
use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::AppError;
use crate::storage::StorageProvider;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct BackupSnapshot {
    pub format_version: u32,
    pub schema_version: String,
    pub created_at: String,
    pub tables: BTreeMap<String, Vec<Map<String, Json>>>,
    pub attachments: Vec<(String, String)>, // (relative_path, base64 content)
}

#[derive(Debug, Serialize)]
pub struct BackupSummary {
    pub table_count: usize,
    pub row_count: usize,
    pub attachment_count: usize,
}

pub fn summarize(snapshot: &BackupSnapshot) -> BackupSummary {
    BackupSummary {
        table_count: snapshot.tables.len(),
        row_count: snapshot.tables.values().map(Vec::len).sum(),
        attachment_count: snapshot.attachments.len(),
    }
}

// SQLite affinity, derivada do tipo declarado (`PRAGMA table_info`), decide como cada valor é
// lido/escrito. Esta é a lista completa de tipos que os helpers de `sea_orm_migration::schema`
// realmente geram neste projeto (conferido em `migration/src/*.rs`): integer/pk_auto/
// big_integer -> INTEGER, string/text -> TEXT, double -> REAL, boolean -> nenhuma palavra-chave
// reconhecida (afinidade NUMERIC do SQLite), mas como todo valor booleano gravado é sempre 0/1,
// tratar o fallback NUMERIC como Integer funciona pra tudo que existe hoje.
#[derive(Clone, Copy)]
enum Affinity {
    Integer,
    Real,
    Text,
    Blob,
}

fn affinity_of(decl_type: &str) -> Affinity {
    let t = decl_type.to_uppercase();
    if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") {
        Affinity::Text
    } else if t.contains("BLOB") {
        Affinity::Blob
    } else if t.contains("REAL") || t.contains("FLOA") || t.contains("DOUB") {
        Affinity::Real
    } else {
        Affinity::Integer
    }
}

struct ColumnDef {
    name: String,
    affinity: Affinity,
}

async fn list_tables(conn: &impl ConnectionTrait) -> Result<Vec<String>, AppError> {
    let rows = conn
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'"
                .to_owned(),
        ))
        .await?;
    rows.into_iter()
        .map(|r| r.try_get_by::<String, _>("name").map_err(AppError::from))
        .collect()
}

async fn table_columns(conn: &impl ConnectionTrait, table: &str) -> Result<Vec<ColumnDef>, AppError> {
    let rows = conn
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            format!("PRAGMA table_info(\"{table}\")"),
        ))
        .await?;
    rows.into_iter()
        .map(|r| {
            let name: String = r.try_get_by("name")?;
            let decl_type: String = r.try_get_by("type")?;
            Ok(ColumnDef { affinity: affinity_of(&decl_type), name })
        })
        .collect::<Result<Vec<_>, sea_orm::DbErr>>()
        .map_err(AppError::from)
}

fn row_to_json(row: &sea_orm::QueryResult, columns: &[ColumnDef]) -> Result<Map<String, Json>, AppError> {
    let mut out = Map::with_capacity(columns.len());
    for col in columns {
        let value = match col.affinity {
            Affinity::Integer => row
                .try_get_by::<Option<i64>, _>(col.name.as_str())?
                .map(Json::from)
                .unwrap_or(Json::Null),
            Affinity::Real => row
                .try_get_by::<Option<f64>, _>(col.name.as_str())?
                .map(Json::from)
                .unwrap_or(Json::Null),
            Affinity::Text => row
                .try_get_by::<Option<String>, _>(col.name.as_str())?
                .map(Json::from)
                .unwrap_or(Json::Null),
            Affinity::Blob => row
                .try_get_by::<Option<Vec<u8>>, _>(col.name.as_str())?
                .map(|bytes| Json::from(B64.encode(bytes)))
                .unwrap_or(Json::Null),
        };
        out.insert(col.name.clone(), value);
    }
    Ok(out)
}

fn json_to_sea_value(value: &Json, affinity: Affinity) -> Result<sea_orm::Value, AppError> {
    if value.is_null() {
        return Ok(match affinity {
            Affinity::Integer => sea_orm::Value::BigInt(None),
            Affinity::Real => sea_orm::Value::Double(None),
            Affinity::Text => sea_orm::Value::String(None),
            Affinity::Blob => sea_orm::Value::Bytes(None),
        });
    }
    match affinity {
        Affinity::Integer => value
            .as_i64()
            .map(|v| v.into())
            .ok_or_else(|| AppError::InvalidInput(format!("expected integer, got {value}"))),
        Affinity::Real => value
            .as_f64()
            .map(|v| v.into())
            .ok_or_else(|| AppError::InvalidInput(format!("expected number, got {value}"))),
        Affinity::Text => value
            .as_str()
            .map(|v| v.to_owned().into())
            .ok_or_else(|| AppError::InvalidInput(format!("expected string, got {value}"))),
        Affinity::Blob => {
            let s = value
                .as_str()
                .ok_or_else(|| AppError::InvalidInput(format!("expected base64 string, got {value}")))?;
            let bytes = B64
                .decode(s)
                .map_err(|e| AppError::InvalidInput(format!("invalid base64 blob: {e}")))?;
            Ok(bytes.into())
        }
    }
}

async fn current_schema_version(conn: &impl ConnectionTrait) -> Result<String, AppError> {
    let row = conn
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT version FROM seaql_migrations ORDER BY version DESC LIMIT 1".to_owned(),
        ))
        .await?;
    match row {
        Some(r) => Ok(r.try_get_by::<String, _>("version")?),
        None => Ok(String::new()),
    }
}

pub async fn export_snapshot(
    db: &DatabaseConnection,
    provider: &dyn StorageProvider,
) -> Result<BackupSnapshot, AppError> {
    let tables = list_tables(db).await?;
    let mut out = BTreeMap::new();
    for table in &tables {
        let columns = table_columns(db, table).await?;
        let rows = db
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                format!("SELECT * FROM \"{table}\""),
            ))
            .await?;
        let table_rows = rows
            .iter()
            .map(|r| row_to_json(r, &columns))
            .collect::<Result<Vec<_>, _>>()?;
        out.insert(table.clone(), table_rows);
    }

    let attachments = provider
        .export_all()?
        .into_iter()
        .map(|(path, bytes)| (path, B64.encode(bytes)))
        .collect();

    Ok(BackupSnapshot {
        format_version: FORMAT_VERSION,
        schema_version: current_schema_version(db).await?,
        created_at: chrono::Utc::now().to_rfc3339(),
        tables: out,
        attachments,
    })
}

// Restaura dentro de uma única transação. `PRAGMA defer_foreign_keys = ON` (diferente de
// `PRAGMA foreign_keys`, que não pode mudar dentro de uma transação em aberto) adia a checagem
// de toda constraint de FK pro momento do commit — dá pra apagar+inserir todas as tabelas em
// qualquer ordem sem violar constraint no meio do caminho, e o `commit()` falha sozinho (com
// rollback automático) se alguma referência ficar quebrada no fim.
pub async fn import_snapshot(
    db: &DatabaseConnection,
    provider: &dyn StorageProvider,
    snapshot: BackupSnapshot,
) -> Result<BackupSummary, AppError> {
    if snapshot.format_version != FORMAT_VERSION {
        return Err(AppError::InvalidInput(format!(
            "unsupported backup format_version {} (expected {FORMAT_VERSION})",
            snapshot.format_version
        )));
    }

    let txn = db.begin().await?;
    let current_version = current_schema_version(&txn).await?;
    if snapshot.schema_version != current_version {
        return Err(AppError::InvalidInput(format!(
            "backup was made with schema version '{}', but this Anchor install is on '{}' — \
             update/downgrade Anchor to a matching version before restoring",
            snapshot.schema_version, current_version
        )));
    }

    txn.execute_unprepared("PRAGMA defer_foreign_keys = ON").await?;

    for (table, rows) in &snapshot.tables {
        txn.execute_unprepared(&format!("DELETE FROM \"{table}\""))
            .await?;
        if rows.is_empty() {
            continue;
        }

        let columns = table_columns(&txn, table).await?;
        for row in rows {
            let mut values = Vec::with_capacity(columns.len());
            for col in &columns {
                let json_value = row.get(&col.name).unwrap_or(&Json::Null);
                values.push(json_to_sea_value(json_value, col.affinity)?);
            }
            let column_list = columns
                .iter()
                .map(|c| format!("\"{}\"", c.name))
                .collect::<Vec<_>>()
                .join(", ");
            let placeholders = vec!["?"; columns.len()].join(", ");
            let sql = format!("INSERT INTO \"{table}\" ({column_list}) VALUES ({placeholders})");
            txn.execute(Statement::from_sql_and_values(DbBackend::Sqlite, &sql, values))
                .await?;
        }
    }

    txn.commit().await?;

    let attachments = snapshot
        .attachments
        .into_iter()
        .map(|(path, b64)| {
            let bytes = B64
                .decode(&b64)
                .map_err(|e| AppError::InvalidInput(format!("invalid base64 attachment '{path}': {e}")))?;
            Ok((path, bytes))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let attachment_count = attachments.len();
    provider.import_all(attachments)?;

    Ok(BackupSummary {
        table_count: snapshot.tables.len(),
        row_count: snapshot.tables.values().map(Vec::len).sum(),
        attachment_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::LocalFSProvider;
    use rand::rngs::OsRng;
    use rand::RngCore;

    async fn memory_db_with_schema_version(version: &str) -> DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE seaql_migrations (version TEXT PRIMARY KEY)")
            .await
            .unwrap();
        db.execute_unprepared(&format!(
            "INSERT INTO seaql_migrations (version) VALUES ('{version}')"
        ))
        .await
        .unwrap();
        db
    }

    fn temp_provider() -> (LocalFSProvider, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("anchor_backup_test_{:08x}", OsRng.next_u32()));
        (LocalFSProvider::new(dir.clone()), dir)
    }

    #[tokio::test]
    async fn round_trip_preserves_rows_nulls_and_respects_foreign_keys() {
        let db = memory_db_with_schema_version("v1").await;
        db.execute_unprepared(
            "CREATE TABLE parent (id INTEGER PRIMARY KEY, name TEXT, score REAL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER, note TEXT, \
             FOREIGN KEY (parent_id) REFERENCES parent(id))",
        )
        .await
        .unwrap();
        db.execute_unprepared("INSERT INTO parent (id, name, score) VALUES (1, 'a', 1.5)")
            .await
            .unwrap();
        db.execute_unprepared("INSERT INTO parent (id, name, score) VALUES (2, NULL, NULL)")
            .await
            .unwrap();
        db.execute_unprepared("INSERT INTO child (id, parent_id, note) VALUES (10, 1, 'hi')")
            .await
            .unwrap();

        let (provider, dir) = temp_provider();
        provider.write("asset_attachments/1/a.pdf", b"pdf-bytes").unwrap();

        let snapshot = export_snapshot(&db, &provider).await.unwrap();
        let summary = summarize(&snapshot);
        // 3 linhas de app (2 parent + 1 child) + 1 linha de `seaql_migrations` — a introspecção
        // genérica cobre todas as tabelas do banco de propósito, ver comentário em `import_snapshot`.
        assert_eq!(summary.row_count, 4);
        assert_eq!(summary.attachment_count, 1);

        // Simula um banco/provider destino vazios (mesmo schema/versão).
        let target_db = memory_db_with_schema_version("v1").await;
        target_db
            .execute_unprepared(
                "CREATE TABLE parent (id INTEGER PRIMARY KEY, name TEXT, score REAL)",
            )
            .await
            .unwrap();
        target_db
            .execute_unprepared(
                "CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER, note TEXT, \
                 FOREIGN KEY (parent_id) REFERENCES parent(id))",
            )
            .await
            .unwrap();
        let (target_provider, target_dir) = temp_provider();

        let restore_summary = import_snapshot(&target_db, &target_provider, snapshot)
            .await
            .unwrap();
        assert_eq!(restore_summary.row_count, 4);
        assert_eq!(restore_summary.attachment_count, 1);
        assert_eq!(
            target_provider.read("asset_attachments/1/a.pdf").unwrap(),
            b"pdf-bytes"
        );

        let rows = target_db
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT name, score FROM parent WHERE id = 2".to_owned(),
            ))
            .await
            .unwrap();
        let name: Option<String> = rows[0].try_get_by("name").unwrap();
        let score: Option<f64> = rows[0].try_get_by("score").unwrap();
        assert_eq!(name, None);
        assert_eq!(score, None);

        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&target_dir).ok();
    }

    #[tokio::test]
    async fn import_rejects_unknown_format_version() {
        let db = memory_db_with_schema_version("v1").await;
        let (provider, dir) = temp_provider();
        let snapshot = BackupSnapshot {
            format_version: 999,
            schema_version: "v1".to_owned(),
            created_at: "".to_owned(),
            tables: BTreeMap::new(),
            attachments: vec![],
        };
        let err = import_snapshot(&db, &provider, snapshot).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(msg) if msg.contains("format_version")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn import_rejects_mismatched_schema_version() {
        let db = memory_db_with_schema_version("v2-current").await;
        let (provider, dir) = temp_provider();
        let snapshot = BackupSnapshot {
            format_version: FORMAT_VERSION,
            schema_version: "v1-old".to_owned(),
            created_at: "".to_owned(),
            tables: BTreeMap::new(),
            attachments: vec![],
        };
        let err = import_snapshot(&db, &provider, snapshot).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(msg) if msg.contains("schema version")));
        std::fs::remove_dir_all(&dir).ok();
    }
}
