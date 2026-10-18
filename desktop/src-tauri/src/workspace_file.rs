// Fase 34 — arquivo portátil de UM Workspace (JSON), diferente do backup
// (`backup.rs`, o banco inteiro com os mesmos ids e restauração total). Exportar
// leva as linhas que pertencem ao Workspace (suas carteiras, contas,
// lançamentos, empréstimos, dívidas…) e as linhas do catálogo global de ativos
// que elas referenciam. Importar **cria um Workspace novo**, nunca sobrescreve:
// os ids são renumerados tabela a tabela e os ativos são casados por
// (ticker, classe) com os que o banco de destino já tem.
//
// A importação insere só as colunas que existem nos dois lados, então um arquivo
// de uma versão anterior (sem colunas novas) entra usando o default delas.
// Anexos (teses, ativos), segredos do keyring e a fila de pendentes da importação
// (`import_pending`, estado transitório de revisão) ficam de fora do v1.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::backup::{json_to_sea_value, row_to_json, table_columns};
use crate::error::AppError;

pub const FORMAT: &str = "anchor-workspace";
pub const VERSION: u32 = 1;

/// Tabelas do Workspace, na ordem em que podem ser inseridas (pais antes dos
/// filhos): (tabela, coluna que a prende ao dono, tabela dona). A ligação é
/// explícita de propósito: seguir as FKs declaradas não basta, porque nem toda
/// referência tem FK e uma tabela com várias FKs ficaria ambígua.
const SPEC: [(&str, &str, &str); 20] = [
    ("workspace_settings", "workspace_id", "workspace"),
    ("home_widget_layout", "workspace_id", "workspace"),
    ("portfolio", "workspace_id", "workspace"),
    ("custodia", "workspace_id", "workspace"),
    ("asset_favorites", "workspace_id", "workspace"),
    ("watchlists", "workspace_id", "workspace"),
    ("watchlist_items", "watchlist_id", "watchlists"),
    ("theses", "workspace_id", "workspace"),
    ("general_transaction_categories", "workspace_id", "workspace"),
    ("bank_accounts", "workspace_id", "workspace"),
    ("general_transactions", "workspace_id", "workspace"),
    ("transactions", "portfolio_id", "portfolio"),
    ("option_positions", "portfolio_id", "portfolio"),
    ("suggested_dividends", "portfolio_id", "portfolio"),
    ("borrowers", "workspace_id", "workspace"),
    ("loans", "workspace_id", "workspace"),
    ("loan_repayments", "loan_id", "loans"),
    ("liabilities", "workspace_id", "workspace"),
    ("liability_installments", "liability_id", "liabilities"),
    ("tax_payments", "workspace_id", "workspace"),
];

/// Referências a outra tabela do Workspace sem FK declarada: (tabela, coluna,
/// tabela alvo). Renumeradas junto com as FKs declaradas.
const EXTRA_REFS: [(&str, &str, &str); 1] =
    [("general_transactions", "category_id", "general_transaction_categories")];

/// Referências de uma linha a outra da própria tabela, sem FK: só podem ser
/// reescritas depois que todas as linhas têm id novo.
const SELF_REFS: [(&str, &str); 1] = [("general_transactions", "transfer_id")];

/// Tabelas do catálogo global copiadas só para ativos que o destino ainda não tinha.
const ASSET_DEPENDENT: [&str; 1] = ["asset_valuations"];

#[derive(Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    /// Só informativo: a importação não exige o mesmo schema.
    pub schema_version: String,
    pub workspace: Map<String, Json>,
    pub tables: BTreeMap<String, Vec<Map<String, Json>>>,
    pub assets: Vec<Map<String, Json>>,
    pub asset_tables: BTreeMap<String, Vec<Map<String, Json>>>,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceImportSummary {
    pub workspace_id: i64,
    pub workspace_name: String,
    pub table_count: usize,
    pub row_count: usize,
}

#[derive(Clone)]
struct Reference {
    column: String,
    parent: String,
}

/// FKs declaradas da tabela mais as referências de `EXTRA_REFS`.
async fn references(conn: &impl ConnectionTrait, table: &str) -> Result<Vec<Reference>, AppError> {
    let rows = conn
        .query_all(Statement::from_string(DbBackend::Sqlite, format!("PRAGMA foreign_key_list(\"{table}\")")))
        .await?;
    let mut refs = rows
        .into_iter()
        .map(|r| Ok(Reference { column: r.try_get_by("from")?, parent: r.try_get_by("table")? }))
        .collect::<Result<Vec<_>, sea_orm::DbErr>>()?;
    refs.extend(
        EXTRA_REFS
            .iter()
            .filter(|(t, _, _)| *t == table)
            .map(|(_, column, parent)| Reference { column: column.to_string(), parent: parent.to_string() }),
    );
    Ok(refs)
}

async fn existing_tables(conn: &impl ConnectionTrait) -> Result<BTreeSet<String>, AppError> {
    let rows = conn
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table'".to_owned(),
        ))
        .await?;
    rows.into_iter().map(|r| r.try_get_by::<String, _>("name").map_err(AppError::from)).collect()
}

fn sql_ids(ids: &[i64]) -> String {
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}

async fn select_rows(
    conn: &impl ConnectionTrait,
    table: &str,
    filter: &str,
) -> Result<Vec<Map<String, Json>>, AppError> {
    let columns = table_columns(conn, table).await?;
    let rows = conn
        .query_all(Statement::from_string(DbBackend::Sqlite, format!("SELECT * FROM \"{table}\" WHERE {filter}")))
        .await?;
    rows.iter().map(|r| row_to_json(r, &columns)).collect()
}

fn ids_of(rows: &[Map<String, Json>]) -> Vec<i64> {
    rows.iter().filter_map(|r| r.get("id").and_then(Json::as_i64)).collect()
}

pub async fn export_workspace(db: &DatabaseConnection, workspace_id: i32) -> Result<WorkspaceFile, AppError> {
    let workspace_rows = select_rows(db, "workspace", &format!("id = {workspace_id}")).await?;
    let workspace = workspace_rows
        .into_iter()
        .next()
        .ok_or_else(|| AppError::NotFound(format!("workspace {workspace_id}")))?;

    let present = existing_tables(db).await?;
    let mut ids: HashMap<String, Vec<i64>> = HashMap::new();
    ids.insert("workspace".to_string(), vec![workspace_id as i64]);
    let mut tables: BTreeMap<String, Vec<Map<String, Json>>> = BTreeMap::new();
    let mut asset_ids: BTreeSet<i64> = BTreeSet::new();

    for (table, anchor_column, owner) in SPEC {
        if !present.contains(table) {
            continue;
        }
        let owner_ids = ids.get(owner).cloned().unwrap_or_default();
        if owner_ids.is_empty() {
            continue;
        }
        let rows = select_rows(db, table, &format!("\"{anchor_column}\" IN ({})", sql_ids(&owner_ids))).await?;
        for reference in references(db, table).await?.iter().filter(|r| r.parent == "assets") {
            asset_ids.extend(rows.iter().filter_map(|r| r.get(&reference.column).and_then(Json::as_i64)));
        }
        ids.insert(table.to_string(), ids_of(&rows));
        tables.insert(table.to_string(), rows);
    }

    let asset_list: Vec<i64> = asset_ids.into_iter().collect();
    let (assets, asset_tables) = if asset_list.is_empty() {
        (Vec::new(), BTreeMap::new())
    } else {
        let assets = select_rows(db, "assets", &format!("id IN ({})", sql_ids(&asset_list))).await?;
        let mut dependent = BTreeMap::new();
        for table in ASSET_DEPENDENT.iter().filter(|t| present.contains(**t)) {
            let rows = select_rows(db, table, &format!("asset_id IN ({})", sql_ids(&asset_list))).await?;
            dependent.insert(table.to_string(), rows);
        }
        (assets, dependent)
    };

    let schema_version = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT version FROM seaql_migrations ORDER BY version DESC LIMIT 1".to_owned(),
        ))
        .await?
        .and_then(|r| r.try_get_by::<String, _>("version").ok())
        .unwrap_or_default();

    Ok(WorkspaceFile {
        format: FORMAT.to_string(),
        version: VERSION,
        exported_at: chrono::Utc::now().to_rfc3339(),
        schema_version,
        workspace,
        tables,
        assets,
        asset_tables,
    })
}

type IdMap = HashMap<i64, i64>;

/// Insere a linha (sem o `id`) e devolve o id novo. Colunas de referência são
/// renumeradas pelos mapas já construídos; colunas que o destino não tem somem.
async fn insert_row(
    conn: &impl ConnectionTrait,
    table: &str,
    row: &Map<String, Json>,
    maps: &HashMap<String, IdMap>,
    refs: &[Reference],
) -> Result<i64, AppError> {
    let columns = table_columns(conn, table).await?;
    let mut names = Vec::new();
    let mut values = Vec::new();
    for col in columns.iter().filter(|c| c.name != "id") {
        let Some(raw) = row.get(&col.name) else { continue };
        let value = if let Some(reference) = refs.iter().find(|r| r.column == col.name) {
            match raw.as_i64() {
                None => sea_orm::Value::BigInt(None),
                Some(old) => {
                    let new = maps.get(&reference.parent).and_then(|m| m.get(&old)).copied().ok_or_else(|| {
                        AppError::InvalidInput(format!(
                            "{table}.{} aponta para {} #{old}, que não está no arquivo",
                            col.name, reference.parent
                        ))
                    })?;
                    sea_orm::Value::from(new)
                }
            }
        } else if SELF_REFS.iter().any(|(t, c)| *t == table && *c == col.name) {
            // Reescrita depois, quando todas as linhas da tabela já têm id novo.
            sea_orm::Value::BigInt(None)
        } else {
            json_to_sea_value(raw, col.affinity)?
        };
        names.push(format!("\"{}\"", col.name));
        values.push(value);
    }
    let sql = if names.is_empty() {
        format!("INSERT INTO \"{table}\" DEFAULT VALUES")
    } else {
        format!("INSERT INTO \"{table}\" ({}) VALUES ({})", names.join(", "), vec!["?"; names.len()].join(", "))
    };
    let result = conn.execute(Statement::from_sql_and_values(DbBackend::Sqlite, &sql, values)).await?;
    Ok(result.last_insert_id() as i64)
}

async fn unique_workspace_name(conn: &impl ConnectionTrait, wanted: &str) -> Result<String, AppError> {
    let rows = conn
        .query_all(Statement::from_string(DbBackend::Sqlite, "SELECT name FROM workspace".to_owned()))
        .await?;
    let taken: BTreeSet<String> = rows.into_iter().filter_map(|r| r.try_get_by::<String, _>("name").ok()).collect();
    if !taken.contains(wanted) {
        return Ok(wanted.to_string());
    }
    let mut n = 1;
    loop {
        let candidate = if n == 1 { format!("{wanted} (imported)") } else { format!("{wanted} (imported {n})") };
        if !taken.contains(&candidate) {
            return Ok(candidate);
        }
        n += 1;
    }
}

pub async fn import_workspace(
    db: &DatabaseConnection,
    file: WorkspaceFile,
) -> Result<WorkspaceImportSummary, AppError> {
    if file.format != FORMAT {
        return Err(AppError::InvalidInput(format!("não é um arquivo de Workspace do Anchor (formato '{}')", file.format)));
    }
    if file.version != VERSION {
        return Err(AppError::InvalidInput(format!(
            "versão {} do arquivo de Workspace não suportada (esta versão do Anchor lê a {VERSION})",
            file.version
        )));
    }
    let old_workspace_id = file
        .workspace
        .get("id")
        .and_then(Json::as_i64)
        .ok_or_else(|| AppError::InvalidInput("arquivo sem o id do Workspace".to_string()))?;
    let wanted_name = file.workspace.get("name").and_then(Json::as_str).unwrap_or("Imported").to_string();

    let txn = db.begin().await?;
    txn.execute_unprepared("PRAGMA defer_foreign_keys = ON").await?;

    let name = unique_workspace_name(&txn, &wanted_name).await?;
    let mut workspace_row = file.workspace.clone();
    workspace_row.insert("name".to_string(), Json::from(name.clone()));
    let new_workspace_id = insert_row(&txn, "workspace", &workspace_row, &HashMap::new(), &[]).await?;

    let mut maps: HashMap<String, IdMap> = HashMap::new();
    maps.insert("workspace".to_string(), HashMap::from([(old_workspace_id, new_workspace_id)]));
    let mut row_count = 1;

    // Ativos: casa por (ticker, classe); cria só o que o destino não tem.
    let mut asset_map: IdMap = HashMap::new();
    let mut created_assets: BTreeSet<i64> = BTreeSet::new();
    for asset in &file.assets {
        let (Some(old_id), Some(ticker), Some(class)) = (
            asset.get("id").and_then(Json::as_i64),
            asset.get("ticker").and_then(Json::as_str),
            asset.get("asset_class").and_then(Json::as_str),
        ) else {
            continue;
        };
        let existing = txn
            .query_one(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT id FROM assets WHERE ticker = ? AND asset_class = ?",
                [ticker.into(), class.into()],
            ))
            .await?
            .and_then(|r| r.try_get_by::<i64, _>("id").ok());
        let new_id = match existing {
            Some(id) => id,
            None => {
                created_assets.insert(old_id);
                row_count += 1;
                insert_row(&txn, "assets", asset, &maps, &[]).await?
            }
        };
        asset_map.insert(old_id, new_id);
    }
    maps.insert("assets".to_string(), asset_map);

    let present = existing_tables(&txn).await?;
    let mut table_count = 1;
    for (table, _, _) in SPEC {
        let Some(rows) = file.tables.get(table) else { continue };
        if !present.contains(table) {
            continue;
        }
        let refs = references(&txn, table).await?;
        let mut id_map: IdMap = HashMap::new();
        for row in rows {
            let new_id = insert_row(&txn, table, row, &maps, &refs).await?;
            if let Some(old_id) = row.get("id").and_then(Json::as_i64) {
                id_map.insert(old_id, new_id);
            }
            row_count += 1;
        }
        table_count += 1;
        maps.insert(table.to_string(), id_map);
    }

    // Valorações manuais só dos ativos que esta importação criou.
    for table in ASSET_DEPENDENT {
        let Some(rows) = file.asset_tables.get(table) else { continue };
        let refs = references(&txn, table).await?;
        for row in rows {
            let old_asset = row.get("asset_id").and_then(Json::as_i64);
            if old_asset.is_some_and(|id| created_assets.contains(&id)) {
                insert_row(&txn, table, row, &maps, &refs).await?;
                row_count += 1;
            }
        }
    }

    // Referências à própria tabela (ex.: a outra perna de uma transferência).
    for (table, column) in SELF_REFS {
        let Some(rows) = file.tables.get(table) else { continue };
        let own = maps.get(table).cloned().unwrap_or_default();
        for row in rows {
            let (Some(old_id), Some(old_ref)) =
                (row.get("id").and_then(Json::as_i64), row.get(column).and_then(Json::as_i64))
            else {
                continue;
            };
            let (Some(new_id), Some(new_ref)) = (own.get(&old_id), own.get(&old_ref)) else {
                return Err(AppError::InvalidInput(format!(
                    "{table}.{column} aponta para a linha #{old_ref}, que não está no arquivo"
                )));
            };
            txn.execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                &format!("UPDATE \"{table}\" SET \"{column}\" = ? WHERE id = ?"),
                [(*new_ref).into(), (*new_id).into()],
            ))
            .await?;
        }
    }

    txn.commit().await?;
    Ok(WorkspaceImportSummary { workspace_id: new_workspace_id, workspace_name: name, table_count, row_count })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guarda-corpo: falha, listando tudo de uma vez, quando uma migration nova
    /// cria uma coluna `*_id` numa tabela do Workspace que a importação não sabe
    /// renumerar (nem FK declarada, nem `EXTRA_REFS`, `SELF_REFS` ou a âncora).
    #[tokio::test]
    async fn every_id_column_of_an_owned_table_is_remapped_or_listed() {
        let db = crate::test_support::memory_db().await;
        let mut missing = Vec::new();
        for (table, anchor, _) in SPEC {
            let refs = references(&db, table).await.unwrap();
            for col in table_columns(&db, table).await.unwrap() {
                let handled = col.name == anchor
                    || refs.iter().any(|r| r.column == col.name)
                    || SELF_REFS.iter().any(|(t, c)| *t == table && *c == col.name);
                if col.name.ends_with("_id") && !handled {
                    missing.push(format!("{table}.{}", col.name));
                }
            }
        }
        assert!(missing.is_empty(), "colunas de referência sem renumeração: {missing:?}");
    }

    #[tokio::test]
    async fn the_spec_order_puts_every_parent_before_its_children() {
        let db = crate::test_support::memory_db().await;
        let order: Vec<&str> = SPEC.iter().map(|(t, _, _)| *t).collect();
        let mut wrong = Vec::new();
        for (index, (table, anchor, owner)) in SPEC.iter().enumerate() {
            // A âncora precisa ser, de fato, uma referência da tabela ao dono.
            let refs = references(&db, table).await.unwrap();
            if !refs.iter().any(|r| r.column == *anchor && r.parent == *owner) {
                wrong.push(format!("{table}.{anchor} não é uma FK para {owner}"));
            }
            for reference in refs {
                if let Some(parent_index) = order.iter().position(|t| *t == reference.parent) {
                    if parent_index > index {
                        wrong.push(format!("{table} vem antes de {}", reference.parent));
                    }
                }
            }
        }
        assert!(wrong.is_empty(), "{wrong:?}");
    }
}
