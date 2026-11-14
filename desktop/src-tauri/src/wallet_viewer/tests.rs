//! Testes do módulo: comportamento (liga/desliga, endereços) e o teste-guarda do ADR 0002
//! (isolamento das tabelas de investimento e ausência de código de assinar).

use std::path::{Path, PathBuf};

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

use super::store::{
    add_address, add_xpub, is_enabled, list_addresses, list_xpubs, remove_address, remove_xpub, require_enabled,
    set_enabled,
};
use crate::domain::bitcoin_xpub::XpubKind;
use crate::error::AppError;
use crate::test_support::memory_db;

const ADDRESS: &str = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";

async fn enabled_db() -> DatabaseConnection {
    let db = memory_db().await;
    set_enabled(&db, true).await.unwrap();
    db
}

// ---------------------------------------------------------------- liga e desliga

#[tokio::test]
async fn module_starts_disabled() {
    let db = memory_db().await;
    assert!(!is_enabled(&db).await.unwrap());
}

#[tokio::test]
async fn enabling_and_disabling_is_remembered() {
    let db = memory_db().await;
    set_enabled(&db, true).await.unwrap();
    assert!(is_enabled(&db).await.unwrap());
    set_enabled(&db, true).await.unwrap(); // idempotente: continua uma linha só
    set_enabled(&db, false).await.unwrap();
    assert!(!is_enabled(&db).await.unwrap());
}

#[tokio::test]
async fn require_enabled_refuses_when_disabled_and_passes_when_enabled() {
    let db = memory_db().await;
    assert!(matches!(require_enabled(&db).await, Err(AppError::InvalidInput(_))));
    set_enabled(&db, true).await.unwrap();
    require_enabled(&db).await.unwrap();
}

#[tokio::test]
async fn adding_an_address_while_disabled_is_refused_and_stores_nothing() {
    let db = memory_db().await;
    assert!(matches!(add_address(&db, ADDRESS, None).await, Err(AppError::InvalidInput(_))));
    assert!(list_addresses(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn disabling_keeps_the_saved_addresses() {
    let db = enabled_db().await;
    add_address(&db, ADDRESS, Some("cold wallet")).await.unwrap();
    set_enabled(&db, false).await.unwrap();
    assert_eq!(list_addresses(&db).await.unwrap().len(), 1);
}

// ---------------------------------------------------------------- endereços

#[tokio::test]
async fn a_lowercase_address_is_stored_in_checksum_form_with_its_label() {
    let db = enabled_db().await;
    let saved = add_address(&db, &ADDRESS.to_lowercase(), Some("  cold wallet  ")).await.unwrap();
    assert_eq!(saved.address, ADDRESS);
    assert_eq!(saved.label.as_deref(), Some("cold wallet"));
    assert_eq!(list_addresses(&db).await.unwrap(), vec![saved]);
}

#[tokio::test]
async fn a_blank_label_is_stored_as_no_label() {
    let db = enabled_db().await;
    assert_eq!(add_address(&db, ADDRESS, Some("   ")).await.unwrap().label, None);
}

#[tokio::test]
async fn an_invalid_address_is_refused_and_stores_nothing() {
    let db = enabled_db().await;
    assert!(matches!(add_address(&db, "0x1234", None).await, Err(AppError::InvalidAddress(_))));
    assert!(list_addresses(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn the_same_address_in_another_case_is_a_duplicate() {
    let db = enabled_db().await;
    add_address(&db, ADDRESS, None).await.unwrap();
    assert!(matches!(
        add_address(&db, &ADDRESS.to_lowercase(), None).await,
        Err(AppError::InvalidInput(_))
    ));
    assert_eq!(list_addresses(&db).await.unwrap().len(), 1);
}

#[tokio::test]
async fn removing_deletes_it_and_removing_an_unknown_id_is_not_found() {
    let db = enabled_db().await;
    let saved = add_address(&db, ADDRESS, None).await.unwrap();
    remove_address(&db, saved.id).await.unwrap();
    assert!(list_addresses(&db).await.unwrap().is_empty());
    assert!(matches!(remove_address(&db, saved.id).await, Err(AppError::NotFound(_))));
}

// ---------------------------------------------------------------- xpub de Bitcoin (Fase 36.4a)

const ZPUB: &str = "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs";
const XPUB: &str = "xpub6BosfCnifzxcFwrSzQiqu2DBVTshkCXacvNsWGYJVVhhawA7d4R5WSWGFNbi8Aw6ZRc1brxMyWMzG3DSSSSoekkudhUd9yLb6qx39T9nMdj";
const XPRV: &str = "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi";

#[tokio::test]
async fn adding_an_xpub_while_disabled_is_refused_and_stores_nothing() {
    let db = memory_db().await;
    assert!(matches!(add_xpub(&db, ZPUB, None).await, Err(AppError::InvalidInput(_))));
    assert!(list_xpubs(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn an_xpub_is_stored_trimmed_with_its_kind_and_label() {
    let db = enabled_db().await;
    let zpub = add_xpub(&db, &format!("  {ZPUB}\n"), Some("  cold storage  ")).await.unwrap();
    assert_eq!(zpub.xpub, ZPUB);
    assert_eq!(zpub.kind, XpubKind::NativeSegwit);
    assert_eq!(zpub.label.as_deref(), Some("cold storage"));
    let xpub = add_xpub(&db, XPUB, Some("   ")).await.unwrap();
    assert_eq!((xpub.kind, xpub.label), (XpubKind::Legacy, None));
    assert_eq!(list_xpubs(&db).await.unwrap(), vec![zpub, xpub]);
}

#[tokio::test]
async fn a_private_key_is_refused_and_stores_nothing() {
    let db = enabled_db().await;
    assert!(matches!(add_xpub(&db, XPRV, None).await, Err(AppError::InvalidAddress(_))));
    assert!(list_xpubs(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn an_invalid_xpub_is_refused_and_stores_nothing() {
    let db = enabled_db().await;
    assert!(matches!(add_xpub(&db, "zpub-nonsense", None).await, Err(AppError::InvalidAddress(_))));
    assert!(list_xpubs(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn the_same_xpub_twice_is_a_duplicate() {
    let db = enabled_db().await;
    add_xpub(&db, ZPUB, None).await.unwrap();
    assert!(matches!(add_xpub(&db, &format!(" {ZPUB} "), None).await, Err(AppError::InvalidInput(_))));
    assert_eq!(list_xpubs(&db).await.unwrap().len(), 1);
}

#[tokio::test]
async fn removing_an_xpub_deletes_it_and_an_unknown_id_is_not_found() {
    let db = enabled_db().await;
    let saved = add_xpub(&db, ZPUB, None).await.unwrap();
    remove_xpub(&db, saved.id).await.unwrap();
    assert!(list_xpubs(&db).await.unwrap().is_empty());
    assert!(matches!(remove_xpub(&db, saved.id).await, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn disabling_keeps_the_saved_xpubs() {
    let db = enabled_db().await;
    add_xpub(&db, ZPUB, None).await.unwrap();
    set_enabled(&db, false).await.unwrap();
    assert_eq!(list_xpubs(&db).await.unwrap().len(), 1);
}

// ---------------------------------------------------------------- guarda do ADR 0002

async fn table_names(db: &DatabaseConnection) -> Vec<String> {
    let rows = db
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        ))
        .await
        .unwrap();
    rows.iter().map(|row| row.try_get::<String>("", "name").unwrap()).collect()
}

#[tokio::test]
async fn module_tables_have_no_foreign_keys_at_all() {
    let db = memory_db().await;
    let module_tables: Vec<String> =
        table_names(&db).await.into_iter().filter(|name| name.starts_with("wallet_viewer_")).collect();
    // Sem isto o teste passaria "no vazio" se a migration não existisse.
    assert!(module_tables.len() >= 2, "expected the module tables, found {module_tables:?}");
    for table in module_tables {
        let rows = db
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                format!("PRAGMA foreign_key_list('{table}')"),
            ))
            .await
            .unwrap();
        assert!(rows.is_empty(), "{table} must not declare foreign keys (ADR 0002)");
    }
}

/// Termos que só existiriam em código de assinar ou de chave privada. Minúsculas.
/// Fase 36.2: `sign_tx`, `ins_sign` e `personal_message` cobrem os pedidos de assinar da Ledger;
/// `ethereum_sign` cobre os métodos de assinar do `trezor-client` (PR do Trezor).
const SIGNING_TERMS: &[&str] = &[
    "private_key",
    "privatekey",
    "privkey",
    "mnemonic",
    "seed_phrase",
    "sign_transaction",
    "sign_message",
    "personal_sign",
    "eth_sign",
    "sendtransaction",
    "send_transaction",
    "ethereum_sign",
    "sign_tx",
    "ins_sign",
    "personal_message",
    // Fase 36.3: os nomes das mensagens protobuf do Trezor (`EthereumSignTx`, `EthereumSignMessage`...).
    "ethereumsign",
    "signtx",
];

/// Lista o que um código-fonte do módulo faz de proibido: tocar tabela de investimento ou assinar.
fn violations(source: &str, investment_tables: &[String]) -> Vec<String> {
    let lower = source.to_lowercase();
    let mut found = Vec::new();
    for table in investment_tables {
        if lower.contains(&format!("entity::{table}")) || lower.contains(&format!("\"{table}\"")) {
            found.push(format!("references investment table `{table}`"));
        }
    }
    for term in SIGNING_TERMS {
        if lower.contains(term) {
            found.push(format!("contains signing term `{term}`"));
        }
    }
    found
}

fn module_source_files() -> Vec<PathBuf> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<PathBuf> = std::fs::read_dir(src.join("wallet_viewer"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        // Este arquivo cita os termos proibidos de propósito.
        .filter(|path| path.file_name().is_some_and(|name| name != "tests.rs"))
        .collect();
    for extra in ["domain/wallet_address.rs", "domain/bitcoin_xpub.rs", "commands/wallet_viewer.rs"] {
        let path = src.join(extra);
        if path.exists() {
            files.push(path);
        }
    }
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("migration/src");
    for entry in std::fs::read_dir(migrations).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().is_some_and(|name| name.to_string_lossy().contains("create_wallet_viewer")) {
            files.push(path);
        }
    }
    files
}

#[test]
fn the_scanner_flags_investment_tables_and_signing_code_and_passes_clean_code() {
    let tables = vec!["assets".to_string(), "transactions".to_string()];
    assert!(!violations("use crate::entity::assets;", &tables).is_empty());
    assert!(!violations("let sql = \"transactions\";", &tables).is_empty());
    assert!(!violations("fn sign_transaction() {}", &tables).is_empty());
    assert!(!violations("let k = PRIVATE_KEY;", &tables).is_empty());
    assert!(!violations("trezor.ethereum_sign_tx(path)", &tables).is_empty());
    assert!(!violations("const INS_SIGN: u8 = 0x04;", &tables).is_empty());
    assert!(!violations("fn build_sign_tx_apdus() {}", &tables).is_empty());
    assert!(!violations("MessageType::MessageType_EthereumSignTx", &tables).is_empty());
    assert!(!violations("let req = protos::EthereumSignMessage::new();", &tables).is_empty());
    assert!(violations("fn parse_balance_response() {}", &tables).is_empty());
}

#[tokio::test]
async fn module_code_never_touches_investment_tables_or_signs() {
    let db = memory_db().await;
    let investment_tables: Vec<String> = table_names(&db)
        .await
        .into_iter()
        .filter(|name| !name.starts_with("wallet_viewer_") && !name.starts_with("seaql_"))
        .collect();
    assert!(investment_tables.len() > 10, "expected the real schema, found {investment_tables:?}");

    let files = module_source_files();
    assert!(files.len() >= 4, "expected the module sources, found {files:?}");
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        let problems = violations(&source, &investment_tables);
        assert!(problems.is_empty(), "{}: {problems:?}", file.display());
    }
}
