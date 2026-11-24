//! Vigia de fronteira das folhas (ADR 0007, passo 1): `sync_backup`, `wallet_viewer` e `alertas`.
//! Cada teste abaixo fixa uma regra do vigia com código-fonte sintético; os dois últimos
//! aplicam o vigia aos arquivos reais do crate.

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
