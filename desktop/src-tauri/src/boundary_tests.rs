//! Vigia de fronteira das folhas (ADR 0007, passo 1): `sync_backup`, `wallet_viewer` e `alertas`.
//! Cada teste abaixo fixa uma regra do vigia com código-fonte sintético; os dois últimos
//! aplicam o vigia aos arquivos reais do crate.
//!
//! É análise textual (não o compilador): lê os `crate::...` do código de produção (corta no
//! primeiro `#[cfg(test)]` e ignora comentários `//`). Não pega `use super::*`, re-export nem
//! SQL cru. É um alarme barato, não uma prova de isolamento. O mapa de domínios está em
//! `project/MODULES.md`; ao renomear ou mover um arquivo das tabelas abaixo, atualize-as
//! (um teste confere que cada entrada existe).

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Leaf {
    SyncBackup,
    WalletViewer,
    Alertas,
}

type Prefix = &'static [&'static str];

/// Quem é dono de quê: um caminho de módulo (`entity::storage_settings`) pertence à folha.
/// Vale para arquivos (`entity/storage_settings.rs`) e para referências (`crate::entity::storage_settings`).
const OWNED: &[(Leaf, Prefix)] = &[
    (Leaf::SyncBackup, &["storage"]),
    (Leaf::SyncBackup, &["backup"]),
    (Leaf::SyncBackup, &["sync_registry"]),
    (Leaf::SyncBackup, &["arweave"]),
    (Leaf::SyncBackup, &["dead_drop"]),
    (Leaf::SyncBackup, &["ecies"]),
    (Leaf::SyncBackup, &["ipns_key"]),
    (Leaf::SyncBackup, &["lan_sweep"]),
    (Leaf::SyncBackup, &["pin_content_cipher"]),
    (Leaf::SyncBackup, &["sync_snapshot_cipher"]),
    (Leaf::SyncBackup, &["workspace_file"]),
    (Leaf::SyncBackup, &["commands", "backup"]),
    (Leaf::SyncBackup, &["commands", "storage_settings"]),
    (Leaf::SyncBackup, &["commands", "sync_registry"]),
    (Leaf::SyncBackup, &["commands", "truthid"]),
    (Leaf::SyncBackup, &["commands", "workspace_file"]),
    (Leaf::SyncBackup, &["entity", "storage_settings"]),
    (Leaf::WalletViewer, &["wallet_viewer"]),
    (Leaf::WalletViewer, &["commands", "wallet_viewer"]),
    (Leaf::WalletViewer, &["domain", "bitcoin_xpub"]),
    (Leaf::WalletViewer, &["domain", "wallet_address"]),
    (Leaf::WalletViewer, &["entity", "wallet_viewer_address"]),
    (Leaf::WalletViewer, &["entity", "wallet_viewer_settings"]),
    (Leaf::WalletViewer, &["entity", "wallet_viewer_xpub"]),
    (Leaf::Alertas, &["alert_checker"]),
    (Leaf::Alertas, &["commands", "alert_rule"]),
    (Leaf::Alertas, &["domain", "alert_check"]),
    (Leaf::Alertas, &["entity", "alert_event"]),
    (Leaf::Alertas, &["entity", "alert_rule"]),
];

/// O que uma folha pode importar de fora de si (além de `error` e `db`, transversais).
const ALLOWED_OUT: &[(Leaf, Prefix)] = &[
    // cliente HTTP compartilhado do domínio de mercado
    (Leaf::WalletViewer, &["finance_core", "http"]),
    // o que os alertas avaliam: teses e cotações
    (Leaf::Alertas, &["entity", "valuation"]),
    (Leaf::Alertas, &["entity", "indicator_thresholds"]),
    (Leaf::Alertas, &["entity", "crypto_indicators"]),
    (Leaf::Alertas, &["entity", "stock_quotes"]),
    (Leaf::Alertas, &["domain", "crypto_score"]),
];

/// As únicas portas de entrada: (arquivo de fora, o que ele pode importar de uma folha).
const ALLOWED_IN: &[(&str, Prefix)] = &[
    ("commands/property.rs", &["storage"]),
    ("commands/thesis.rs", &["storage"]),
];

struct SourceFile {
    /// Caminho relativo a `src/`, com `/`.
    rel: String,
    source: String,
}

/// `commands/backup.rs` vira `["commands", "backup"]`; `storage/mod.rs` vira `["storage"]`.
fn path_segments(rel: &str) -> Vec<String> {
    let mut segments: Vec<String> = rel.trim_end_matches(".rs").split('/').map(str::to_string).collect();
    if segments.len() > 1 && segments.last().is_some_and(|last| last == "mod") {
        segments.pop();
    }
    segments
}

fn starts_with(path: &[String], prefix: &[&str]) -> bool {
    path.len() >= prefix.len() && path.iter().zip(prefix).all(|(segment, wanted)| segment == wanted)
}

fn leaf_of(path: &[String]) -> Option<Leaf> {
    OWNED.iter().find(|(_, prefix)| starts_with(path, prefix)).map(|(leaf, _)| *leaf)
}

/// Testes não contam como dependência (como na medição do `MODULES.md`).
fn is_skipped(rel: &str) -> bool {
    rel.ends_with("tests.rs") || rel == "test_support.rs"
}

/// Quem só registra ou lista módulos, e as entidades (geradas pelo `sea-orm-cli`, ligadas por `super::`).
fn is_registration(rel: &str) -> bool {
    matches!(rel, "lib.rs" | "main.rs" | "commands/mod.rs") || rel.starts_with("entity/")
}

fn skip_whitespace(text: &str) -> usize {
    text.len() - text.trim_start().len()
}

/// Lê `a::b`, `a::{b, c::{self, d}}` ou `a::*` e acrescenta cada caminho completo a `out`.
/// Devolve quantos bytes consumiu.
fn parse_use_tree(text: &str, prefix: &[String], out: &mut Vec<Vec<String>>) -> usize {
    let mut path = prefix.to_vec();
    let mut pos = skip_whitespace(text);
    loop {
        if text[pos..].starts_with('{') {
            pos += 1;
            loop {
                pos += parse_use_tree(&text[pos..], &path, out);
                pos += skip_whitespace(&text[pos..]);
                if text[pos..].starts_with(',') {
                    pos += 1;
                    pos += skip_whitespace(&text[pos..]);
                    if text[pos..].starts_with('}') {
                        return pos + 1;
                    }
                } else if text[pos..].starts_with('}') {
                    return pos + 1;
                } else {
                    return pos;
                }
            }
        }
        let start = pos;
        pos += text[pos..].find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(text.len() - pos);
        if start == pos {
            // `*` (glob) ou fim inesperado: vale o caminho até aqui
            if path.len() > prefix.len() {
                out.push(path);
            }
            return pos;
        }
        let ident = &text[start..pos];
        if ident == "self" {
            out.push(path);
            return pos;
        }
        path.push(ident.to_string());
        let after = pos + skip_whitespace(&text[pos..]);
        if text[after..].starts_with("::") {
            pos = after + 2;
            pos += skip_whitespace(&text[pos..]);
        } else {
            out.push(path);
            return pos;
        }
    }
}

/// Todo caminho `crate::...` do código de produção de um arquivo.
fn crate_refs(source: &str) -> Vec<Vec<String>> {
    let production = source.split("#[cfg(test)]").next().unwrap_or(source);
    let code: String = production.lines().map(|line| line.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
    let mut refs = Vec::new();
    for (index, _) in code.match_indices("crate::") {
        let glued_to_identifier = code[..index].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_');
        if !glued_to_identifier {
            parse_use_tree(&code[index + "crate::".len()..], &[], &mut refs);
        }
    }
    refs
}

/// Quebras de fronteira, uma linha cada. Vazio quando tudo respeita as regras.
fn violations(files: &[SourceFile]) -> Vec<String> {
    let mut found = Vec::new();
    for file in files.iter().filter(|file| !is_skipped(&file.rel)) {
        let own = leaf_of(&path_segments(&file.rel));
        for path in crate_refs(&file.source) {
            if matches!(path.first().map(String::as_str), Some("error" | "db")) {
                continue;
            }
            let target = leaf_of(&path);
            let allowed = match own {
                Some(leaf) => {
                    target == Some(leaf) || ALLOWED_OUT.iter().any(|(owner, prefix)| *owner == leaf && starts_with(&path, prefix))
                }
                None => {
                    target.is_none()
                        || is_registration(&file.rel)
                        || ALLOWED_IN.iter().any(|(rel, prefix)| *rel == file.rel && starts_with(&path, prefix))
                }
            };
            if !allowed {
                let side = if own.is_some() { "leaf imports outside its boundary" } else { "imports a leaf through a door that is not listed" };
                found.push(format!("{}: {side}: crate::{}", file.rel, path.join("::")));
            }
        }
    }
    found
}

fn collect_sources(root: &Path, dir: &Path, out: &mut Vec<SourceFile>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_sources(root, &path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.push(SourceFile { rel, source: std::fs::read_to_string(&path).unwrap() });
        }
    }
}

fn read_crate_sources() -> Vec<SourceFile> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_sources(&src, &src, &mut files);
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(rel: &str, source: &str) -> SourceFile {
        SourceFile { rel: rel.to_string(), source: source.to_string() }
    }

    fn found(files: &[SourceFile]) -> String {
        violations(files).join("\n")
    }

    #[test]
    fn leaf_file_importing_another_domain_is_flagged() {
        let files = [file("wallet_viewer/store.rs", "use crate::commands::tax;\n")];
        let report = found(&files);
        assert!(report.contains("wallet_viewer/store.rs"), "{report}");
        assert!(report.contains("commands::tax"), "{report}");
    }

    #[test]
    fn leaf_file_importing_its_own_domain_and_allowed_items_passes() {
        let files = [
            file(
                "wallet_viewer/esplora.rs",
                "use crate::domain::bitcoin_xpub::parse_xpub;\nuse crate::finance_core::http;\nuse crate::wallet_viewer::store;\n",
            ),
            file("storage/mod.rs", "use crate::entity::storage_settings;\n"),
            file("alert_checker.rs", "use crate::entity::{alert_event, alert_rule, crypto_indicators, stock_quotes, valuation};\n"),
        ];
        assert_eq!(found(&files), "");
    }

    #[test]
    fn a_leaf_cannot_import_another_leaf() {
        let files = [file("backup.rs", "use crate::wallet_viewer::store;\n")];
        assert!(found(&files).contains("wallet_viewer::store"));
    }

    #[test]
    fn other_file_importing_a_leaf_is_flagged() {
        let files = [file("commands/tax.rs", "use crate::wallet_viewer::store;\n")];
        let report = found(&files);
        assert!(report.contains("commands/tax.rs"), "{report}");
        assert!(report.contains("wallet_viewer::store"), "{report}");
    }

    #[test]
    fn only_the_listed_entry_points_may_use_storage() {
        let allowed = [
            file("commands/property.rs", "let p = crate::storage::resolve_active_provider(&app, db).await?;\n"),
            file("commands/thesis.rs", "let p = crate::storage::resolve_active_provider(&app, db).await?;\n"),
        ];
        assert_eq!(found(&allowed), "");

        let other_door = [file("commands/thesis.rs", "use crate::backup::export_snapshot;\n")];
        assert!(found(&other_door).contains("backup::export_snapshot"));

        let other_file = [file("commands/asset.rs", "use crate::storage::StorageProvider;\n")];
        assert!(found(&other_file).contains("commands/asset.rs"));
    }

    #[test]
    fn test_code_and_comments_are_not_dependencies() {
        let source = "\
use crate::error::AppError;
// use crate::commands::tax;
/// veja `crate::commands::tax`
fn run() {}

#[cfg(test)]
mod tests {
    use crate::commands::tax;
    use crate::test_support::memory_db;
}
";
        let files = [file("wallet_viewer/store.rs", source)];
        assert_eq!(found(&files), "");
    }

    #[test]
    fn brace_lists_are_expanded_across_lines_and_nesting() {
        let source = "\
use crate::entity::{
    alert_event,
    alert_rule,
    tax_rules,
};
use crate::domain::{alert_check, tax::{self, brackets}};
";
        let report = found(&[file("alert_checker.rs", source)]);
        assert!(report.contains("entity::tax_rules"), "{report}");
        assert!(report.contains("domain::tax::brackets"), "{report}");
        assert!(!report.contains("entity::alert_event"), "{report}");
        assert!(!report.contains("entity::alert_rule"), "{report}");
        assert!(!report.contains("domain::alert_check"), "{report}");
        assert_eq!(violations(&[file("alert_checker.rs", source)]).len(), 3, "{report}");
    }

    #[test]
    fn error_and_db_are_transversal() {
        let files = [file("arweave.rs", "use crate::error::AppError;\nuse crate::db::connect;\n")];
        assert_eq!(found(&files), "");
    }

    #[test]
    fn registration_files_entities_and_test_files_are_not_checked_inbound() {
        let files = [
            file("lib.rs", "commands::wallet_viewer::wallet_viewer_get_settings,\nuse crate::wallet_viewer::store;\n"),
            file("commands/mod.rs", "use crate::backup;\n"),
            file("entity/valuation.rs", "use crate::entity::alert_rule;\n"),
            file("flow_tests.rs", "use crate::storage::LocalFSProvider;\n"),
            file("wallet_viewer/tests.rs", "use crate::entity::assets;\n"),
            file("test_support.rs", "use crate::storage::LocalFSProvider;\n"),
        ];
        assert_eq!(found(&files), "");
    }

    #[test]
    fn real_sources_respect_the_leaf_boundaries() {
        let files = read_crate_sources();
        assert!(files.len() > 150, "expected the whole crate, found {} files", files.len());
        for leaf in [Leaf::SyncBackup, Leaf::WalletViewer, Leaf::Alertas] {
            let count = files.iter().filter(|f| leaf_of(&path_segments(&f.rel)) == Some(leaf)).count();
            assert!(count >= 3, "{leaf:?} owns only {count} files: the ownership table is stale");
        }
        let report = found(&files);
        assert_eq!(report, "", "leaf boundary broken:\n{report}");
    }

    #[test]
    fn every_entry_in_the_tables_points_at_a_real_file_or_folder() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let exists = |segments: &[&str]| {
            let base = src.join(segments.join("/"));
            base.with_extension("rs").exists() || base.join("mod.rs").exists() || base.is_dir()
        };
        for (leaf, prefix) in OWNED.iter().chain(ALLOWED_OUT.iter()) {
            assert!(exists(prefix), "{leaf:?}: `{}` does not exist in src/", prefix.join("::"));
        }
        for (rel, prefix) in ALLOWED_IN {
            assert!(src.join(rel).exists(), "entry point `{rel}` does not exist");
            assert!(exists(prefix), "`{}` does not exist in src/", prefix.join("::"));
        }
    }
}
