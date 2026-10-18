use std::sync::atomic::AtomicBool;

use tauri::Manager;

mod alert_checker;
mod arweave;
mod backup;
mod commands;
mod ingestion;
mod workspace_file;
mod crypto_sources;
mod db;
mod dead_drop;
mod domain;
mod ecies;
// `entity/` é gerado pelo `sea-orm-cli`, inclusive o `prelude.rs` com atalhos
// (`Assets`, `Portfolio`...) que o código não usa. Silenciado aqui e não lá
// porque uma nova geração sobrescreveria qualquer ajuste dentro de `entity/`.
#[allow(unused_imports)]
mod entity;
mod error;
mod finance_api;
mod finance_core;
mod ipns_key;
mod lan_sweep;
mod pin_content_cipher;
mod storage;
mod sync_registry;
mod sync_snapshot_cipher;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod flow_tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Fase 11.3 — conexão de banco precisa de um `AppHandle` pra resolver
        // `app_data_dir()` em build de release (`db::resolve_database_path`),
        // que só existe a partir daqui — não dá mais pra conectar antes do
        // `Builder` como antes.
        .setup(|app| {
            let handle = app.handle().clone();
            let db = tauri::async_runtime::block_on(db::connect(&handle))
                .expect("failed to open or migrate the database");
            alert_checker::spawn_periodic_check(db.clone());
            ingestion::spawn_daily_ingestion(handle.clone(), db.clone());
            app.manage(db);

            // Fase 26 — zips grandes das fontes públicas (CVM, SEC) ficam no
            // diretório de cache do SO; sem ele o cache cai em `temp_dir()`.
            if let Ok(cache_dir) = app.path().app_cache_dir() {
                finance_core::disk_cache::init(cache_dir);
            }

            Ok(())
        })
        .manage(AtomicBool::new(false))
        .invoke_handler(tauri::generate_handler![
            commands::bazin::calculate_bazin,
            commands::bazin::save_bazin,
            commands::graham::calculate_graham,
            commands::graham::save_graham,
            commands::gordon::calculate_gordon,
            commands::gordon::save_gordon,
            commands::dcf::calculate_dcf,
            commands::dcf::save_dcf,
            commands::banks::calculate_banks,
            commands::banks::save_banks,
            commands::rim::calculate_rim,
            commands::rim::save_rim,
            commands::rnav::calculate_rnav,
            commands::rnav::save_rnav,
            commands::projected_ceiling::calculate_projected_ceiling,
            commands::projected_ceiling::save_projected_ceiling,
            commands::crypto_indicator::record_crypto_indicator,
            commands::crypto_indicator::list_crypto_indicators,
            commands::crypto_indicator::get_crypto_score,
            commands::crypto_indicator::list_indicator_thresholds,
            commands::crypto_indicator::update_indicator_threshold,
            commands::crypto_indicator::reset_indicator_threshold,
            commands::valuation::list_valuations,
            commands::valuation::get_valuation_inputs,
            commands::valuation::update_valuation,
            commands::valuation::delete_valuation,
            commands::alert_rule::create_alert_rule,
            commands::alert_rule::list_alert_rules,
            commands::alert_rule::set_alert_rule_active,
            commands::alert_rule::delete_alert_rule,
            commands::collector::run_stock_collector,
            commands::collector::run_fx_collector,
            commands::collector::run_crypto_collector,
            commands::collector::list_stock_quotes,
            commands::collector::list_stock_fundamentals,
            commands::collector::list_stock_dividends_avg,
            commands::collector::list_stock_dcf_fundamentals,
            commands::collector::list_stock_technicals,
            commands::collector::list_stock_dividend_payments,
            commands::collector::run_price_history_backfill,
            commands::collector::run_benchmark_backfill,
            commands::portfolio_io::export_transactions_csv,
            commands::portfolio_io::preview_import,
            commands::portfolio_io::commit_import,
            commands::portfolio_io::stage_import,
            commands::portfolio_io::list_import_pending,
            commands::portfolio_io::confirm_import_pending,
            commands::portfolio_io::discard_import_pending,
            commands::portfolio_io::class_tax_hint,
            commands::fixed_income_io::stage_fixed_income_yield,
            commands::fixed_income_io::confirm_yield_pending,
            commands::statement_io::stage_statement,
            commands::statement_io::list_statement_pending,
            commands::statement_io::confirm_statement_pending,
            commands::statement_io::discard_statement_pending,
            commands::pending::count_pending,
            ingestion::get_fx_staleness,
            ingestion::list_ingestion_runs,
            ingestion::run_ingestion_now,
            commands::collector::list_stock_price_history,
            commands::collector::get_latest_crypto_fear_greed,
            commands::stock_notes::list_stock_notes,
            commands::stock_notes::save_stock_note,
            commands::truthid::test_truthid_connection,
            commands::truthid::send_test_sign_request,
            commands::truthid::create_cross_device_sign_request,
            commands::truthid::await_cross_device_sign_request_response,
            commands::truthid::update_sync_record,
            commands::truthid::pin_database_snapshot,
            commands::truthid::create_cross_device_pin_request,
            commands::truthid::push_pin_content,
            commands::truthid::await_cross_device_pin_response,
            commands::truthid::create_cross_device_sign_message_request,
            commands::truthid::await_cross_device_sign_message_response,
            commands::sync_registry::get_sync_record,
            commands::sync_registry::pull_and_verify_sync_snapshot,
            commands::workspace::get_workspace,
            commands::workspace::list_workspaces,
            commands::workspace::create_workspace,
            commands::portfolio::list_portfolios,
            commands::portfolio::create_portfolio,
            commands::portfolio::rename_portfolio,
            commands::custodia::list_custodias,
            commands::custodia::create_custodia,
            commands::custodia::delete_custodia,
            commands::asset::list_assets,
            commands::asset::create_asset,
            commands::asset::update_asset_cnpj,
            commands::asset::update_asset_equity,
            commands::asset::update_asset_underlying_class,
            commands::fii::resolve_fii_cnpj,
            commands::fii::run_fii_cvm_collector,
            commands::fii::list_fii_cvm_monthly,
            commands::fii::list_fii_cvm_properties,
            commands::data_source_credentials::list_data_source_credentials,
            commands::data_source_credentials::set_data_source_credential,
            commands::data_source_credentials::clear_data_source_credential,
            commands::storage_settings::get_storage_settings,
            commands::storage_settings::set_storage_settings,
            commands::backup::export_workspace_backup,
            commands::workspace_file::export_workspace_file,
            commands::workspace_file::import_workspace_file,
            commands::backup::import_workspace_backup,
            commands::tax::list_tax_rules,
            commands::tax::update_tax_rule,
            commands::tax::reset_tax_rule,
            commands::tax::sync_tax_rules,
            commands::tax::get_tax_assessment,
            commands::tax::simulate_tax_sale,
            commands::tax::mark_tax_paid,
            commands::tax::unmark_tax_paid,
            commands::rates::get_interest_rates,
            commands::reit::list_reit_fundamentals,
            commands::reit::get_reit_manual_indicators,
            commands::reit::save_reit_manual_indicators,
            commands::transaction::create_transaction,
            commands::transaction::delete_transaction,
            commands::transaction::list_transactions,
            commands::transaction::get_portfolio_positions,
            commands::portfolio_summary::get_portfolio_summary,
            commands::dividend_suggestion::generate_dividend_suggestions,
            commands::dividend_suggestion::create_expected_dividend,
            commands::dividend_suggestion::list_dividend_suggestions,
            commands::dividend_suggestion::confirm_dividend_suggestion,
            commands::dividend_suggestion::discard_dividend_suggestion,
            commands::cvm_notice::list_cvm_dividend_notices,
            commands::options::list_option_chain,
            commands::options::get_option_greeks,
            commands::options::simulate_option_strategy,
            commands::option_position::create_option_position,
            commands::option_position::list_option_positions,
            commands::option_position::close_option_position,
            commands::option_position::delete_option_position,
            commands::option_position::get_option_marks,
            commands::option_position::set_option_position_net_worth,
            commands::option_position::create_option_positions,
            commands::proventos::get_proventos_summary,
            commands::profitability::get_portfolio_profitability,
            commands::profitability::get_profitability_comparison,
            commands::bank_account::list_bank_accounts,
            commands::bank_account::create_bank_account,
            commands::bank_account::update_bank_account,
            commands::bank_account::delete_bank_account,
            commands::general_transaction::list_general_transactions,
            commands::general_transaction::create_general_transaction,
            commands::general_transaction::update_general_transaction,
            commands::general_transaction::delete_general_transaction,
            commands::general_transaction::create_transfer,
            commands::general_transaction_category::list_general_transaction_categories,
            commands::general_transaction_category::create_general_transaction_category,
            commands::general_transaction_category::update_general_transaction_category,
            commands::general_transaction_category::delete_general_transaction_category,
            commands::liability::create_liability,
            commands::liability::list_liabilities,
            commands::liability::delete_liability,
            commands::loan::create_borrower,
            commands::loan::list_borrowers,
            commands::loan::delete_borrower,
            commands::loan::create_loan,
            commands::loan::update_loan,
            commands::loan::list_loans,
            commands::loan::delete_loan,
            commands::loan::register_loan_repayment,
            commands::loan::delete_loan_repayment,
            commands::net_worth::get_net_worth_summary,
            commands::workspace_summary::get_workspace_summary,
            commands::workspace_summary::get_workspace_profitability,
            commands::currency::get_workspace_settings,
            commands::currency::set_base_currency,
            commands::home_layout::get_home_layout,
            commands::home_layout::save_home_layout,
            commands::watchlist::list_watchlists,
            commands::watchlist::create_watchlist,
            commands::watchlist::delete_watchlist,
            commands::watchlist::list_watchlist_items,
            commands::watchlist::add_watchlist_item,
            commands::watchlist::update_watchlist_item,
            commands::watchlist::remove_watchlist_item,
            commands::watchlist::list_favorite_assets,
            commands::watchlist::toggle_favorite,
            commands::thesis::list_theses,
            commands::thesis::create_thesis,
            commands::thesis::update_thesis,
            commands::thesis::delete_thesis,
            commands::thesis::list_thesis_attachments,
            commands::thesis::add_thesis_attachment,
            commands::thesis::delete_thesis_attachment,
            commands::thesis::get_thesis_attachment_path,
            commands::property::list_asset_valuations,
            commands::property::add_asset_valuation,
            commands::property::delete_asset_valuation,
            commands::property::list_asset_attachments,
            commands::property::add_asset_attachment,
            commands::property::delete_asset_attachment,
            commands::property::get_asset_attachment_path
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {});
}
