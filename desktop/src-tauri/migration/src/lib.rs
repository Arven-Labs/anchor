pub use sea_orm_migration::prelude::*;

mod m20260709_010051_create_valuation_and_bazin_inputs;
mod m20260709_152010_create_graham_inputs;
mod m20260709_153134_create_gordon_inputs;
mod m20260709_154525_create_dcf_inputs;
mod m20260709_155420_create_banks_inputs;
mod m20260709_160307_create_rnav_inputs;
mod m20260709_160929_create_projected_ceiling_inputs;
mod m20260709_212958_create_crypto_score_tables;
mod m20260709_232211_create_stock_quotes;
mod m20260710_115333_create_stock_fundamentals_and_dividends;
mod m20260710_132548_create_stock_dcf_fundamentals;
mod m20260710_134142_add_tax_rate_to_stock_dcf_fundamentals;
mod m20260710_220000_rename_avg_dividend5y_column;
mod m20260711_093000_create_alert_rule_table;
mod m20260711_171445_create_alert_event_table;
mod m20260712_220000_add_payout_to_stock_fundamentals;
mod m20260712_223000_create_rim_inputs;
mod m20260715_215835_create_stock_technicals;
mod m20260715_215836_create_stock_notes;
mod m20260715_230504_add_revenue_to_stock_dcf_fundamentals;
mod m20260715_232005_create_stock_dividend_payments;
mod m20260716_005530_create_ai_api_key;
mod m20260721_120000_create_ai_conversation_and_message;
mod m20260721_140000_add_token_usage_to_ai_message;
mod m20260721_150000_create_ai_valuation_proposal;
mod m20260728_103434_add_inventory_to_stock_dcf_fundamentals;
mod m20260729_090000_create_workspace_and_portfolio;
mod m20260729_090100_create_custodia;
mod m20260729_090200_create_assets;
mod m20260729_090300_create_transactions;
mod m20260729_150000_add_details_to_stock_quotes;
mod m20260729_160000_create_stock_price_history;
mod m20260730_100000_create_watchlists;
mod m20260731_100000_create_theses;
mod m20260802_100000_add_cnpj_and_fii_cvm_tables;
mod m20260802_120000_create_fii_cnpj_cache;
mod m20260802_140000_create_crypto_fear_greed;
mod m20260803_120000_create_company_ai_info;
mod m20260803_140000_create_reit_tables;
mod m20260808_100000_create_asset_valuations_and_attachments;
mod m20260808_150000_add_equity_fields_to_assets;
mod m20260809_150000_create_bank_accounts;
mod m20260809_150100_create_general_transactions;
mod m20260809_160000_create_general_transaction_categories;
mod m20260809_170000_create_liabilities;
mod m20260811_190000_translate_valuation_verdict_to_english;
mod m20260811_200000_add_budget_limit_to_general_transaction_categories;
mod m20260812_100000_create_suggested_dividends;
mod m20260812_110000_expected_dividends_and_source_index;
mod m20260815_100000_add_payment_type_to_suggested_dividends;
mod m20260815_110000_add_payment_type_to_transactions;
mod m20260815_120000_create_macro_index_monthly;
mod m20260822_100000_create_home_widget_layout;
mod m20260823_100000_add_underlying_asset_class_to_assets;
mod m20260829_141241_create_finance_api_settings;
mod m20260829_150000_add_reit_fundamentals_unique_index;
mod m20260830_100000_create_cvm_dividend_notice_documents;
mod m20260913_100000_create_storage_settings;
mod m20261014_100000_drop_ai_tables;
mod m20261014_110000_create_tax_rules;
mod m20261014_120000_create_tax_payments;
mod m20261016_120000_create_loans;
mod m20261016_130000_seed_crypto_derivatives_thresholds;
mod m20261016_140000_seed_crypto_options_liquidity_thresholds;
mod m20261016_150000_create_selic_daily;
mod m20261016_160000_drop_finance_api_settings;
mod m20261017_100000_seed_crypto_exchange_supply_ethbtc_thresholds;
mod m20261017_110000_add_coin_to_indicator_thresholds;
mod m20261017_120000_create_option_positions;
mod m20261017_130000_add_net_worth_to_option_positions;
mod m20261017_140000_create_workspace_settings;
mod m20261017_150000_add_currency_to_accounts_and_loans;
mod m20261017_160000_add_transfer_id_to_general_transactions;
mod m20261018_100000_create_ingestion_runs;
mod m20261019_100000_create_import_pending;
mod m20261020_100000_rebuild_import_pending_by_workspace;
mod m20261021_100000_seed_fixed_income_tax_rules;
mod m20261022_100000_update_fixed_income_cdb_rule_description;
mod m20261023_100000_add_mirrored_in_portfolio_to_bank_accounts;
mod m20261024_100000_update_jcp_rule_source;
mod m20261025_100000_add_auto_yield_to_bank_accounts;
mod m20261026_100000_create_stock_adjusted_close;
mod m20261027_100000_seed_crypto_tax_rules;
mod m20261027_110000_add_kind_to_tax_payments;
mod m20261028_100000_seed_crypto_progressive_rules;
mod m20261029_100000_add_foreign_tax_withheld_to_transactions;
mod m20261029_110000_seed_foreign_tax_rules;
mod m20261030_100000_create_ptax_daily;
mod m20261031_100000_create_offshore_entities;
mod m20261031_110000_seed_offshore_tax_rules;
mod m20261101_100000_create_funds;
mod m20261101_110000_seed_come_cotas_rules;
mod m20261102_100000_seed_fund_redemption_rules;
mod m20261103_100000_add_cvm_fields_to_funds;
mod m20261104_100000_add_administrator_to_funds;
mod m20261105_100000_seed_closed_fund_rules;
mod m20261106_100000_add_fund_structure_and_estoque;
mod m20261107_100000_add_loss_credit_used_to_transactions;
mod m20261108_100000_add_tax_return_fields_to_assets;
mod m20261109_100000_add_foreign_tax_paid_on_to_transactions;
mod m20261110_100000_create_offshore_distributions;
mod m20261111_100000_add_regime_to_offshore_entities;
mod m20261112_100000_add_update_2023_value_to_assets;
mod m20261113_100000_add_update_2023_to_offshore_entities;
mod m20261114_100000_create_offshore_capital_events;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260709_010051_create_valuation_and_bazin_inputs::Migration),
            Box::new(m20260709_152010_create_graham_inputs::Migration),
            Box::new(m20260709_153134_create_gordon_inputs::Migration),
            Box::new(m20260709_154525_create_dcf_inputs::Migration),
            Box::new(m20260709_155420_create_banks_inputs::Migration),
            Box::new(m20260709_160307_create_rnav_inputs::Migration),
            Box::new(m20260709_160929_create_projected_ceiling_inputs::Migration),
            Box::new(m20260709_212958_create_crypto_score_tables::Migration),
            Box::new(m20260709_232211_create_stock_quotes::Migration),
            Box::new(m20260710_115333_create_stock_fundamentals_and_dividends::Migration),
            Box::new(m20260710_132548_create_stock_dcf_fundamentals::Migration),
            Box::new(m20260710_134142_add_tax_rate_to_stock_dcf_fundamentals::Migration),
            Box::new(m20260710_220000_rename_avg_dividend5y_column::Migration),
            Box::new(m20260711_093000_create_alert_rule_table::Migration),
            Box::new(m20260711_171445_create_alert_event_table::Migration),
            Box::new(m20260712_220000_add_payout_to_stock_fundamentals::Migration),
            Box::new(m20260712_223000_create_rim_inputs::Migration),
            Box::new(m20260715_215835_create_stock_technicals::Migration),
            Box::new(m20260715_215836_create_stock_notes::Migration),
            Box::new(m20260715_230504_add_revenue_to_stock_dcf_fundamentals::Migration),
            Box::new(m20260715_232005_create_stock_dividend_payments::Migration),
            Box::new(m20260716_005530_create_ai_api_key::Migration),
            Box::new(m20260721_120000_create_ai_conversation_and_message::Migration),
            Box::new(m20260721_140000_add_token_usage_to_ai_message::Migration),
            Box::new(m20260721_150000_create_ai_valuation_proposal::Migration),
            Box::new(m20260728_103434_add_inventory_to_stock_dcf_fundamentals::Migration),
            Box::new(m20260729_090000_create_workspace_and_portfolio::Migration),
            Box::new(m20260729_090100_create_custodia::Migration),
            Box::new(m20260729_090200_create_assets::Migration),
            Box::new(m20260729_090300_create_transactions::Migration),
            Box::new(m20260729_150000_add_details_to_stock_quotes::Migration),
            Box::new(m20260729_160000_create_stock_price_history::Migration),
            Box::new(m20260730_100000_create_watchlists::Migration),
            Box::new(m20260731_100000_create_theses::Migration),
            Box::new(m20260802_100000_add_cnpj_and_fii_cvm_tables::Migration),
            Box::new(m20260802_120000_create_fii_cnpj_cache::Migration),
            Box::new(m20260802_140000_create_crypto_fear_greed::Migration),
            Box::new(m20260803_120000_create_company_ai_info::Migration),
            Box::new(m20260803_140000_create_reit_tables::Migration),
            Box::new(m20260808_100000_create_asset_valuations_and_attachments::Migration),
            Box::new(m20260808_150000_add_equity_fields_to_assets::Migration),
            Box::new(m20260809_150000_create_bank_accounts::Migration),
            Box::new(m20260809_150100_create_general_transactions::Migration),
            Box::new(m20260809_160000_create_general_transaction_categories::Migration),
            Box::new(m20260809_170000_create_liabilities::Migration),
            Box::new(m20260811_190000_translate_valuation_verdict_to_english::Migration),
            Box::new(m20260811_200000_add_budget_limit_to_general_transaction_categories::Migration),
            Box::new(m20260812_100000_create_suggested_dividends::Migration),
            Box::new(m20260812_110000_expected_dividends_and_source_index::Migration),
            Box::new(m20260815_100000_add_payment_type_to_suggested_dividends::Migration),
            Box::new(m20260815_110000_add_payment_type_to_transactions::Migration),
            Box::new(m20260815_120000_create_macro_index_monthly::Migration),
            Box::new(m20260822_100000_create_home_widget_layout::Migration),
            Box::new(m20260823_100000_add_underlying_asset_class_to_assets::Migration),
            Box::new(m20260829_141241_create_finance_api_settings::Migration),
            Box::new(m20260829_150000_add_reit_fundamentals_unique_index::Migration),
            Box::new(m20260830_100000_create_cvm_dividend_notice_documents::Migration),
            Box::new(m20260913_100000_create_storage_settings::Migration),
            Box::new(m20261014_100000_drop_ai_tables::Migration),
            Box::new(m20261014_110000_create_tax_rules::Migration),
            Box::new(m20261014_120000_create_tax_payments::Migration),
            Box::new(m20261016_120000_create_loans::Migration),
            Box::new(m20261016_130000_seed_crypto_derivatives_thresholds::Migration),
            Box::new(m20261016_140000_seed_crypto_options_liquidity_thresholds::Migration),
            Box::new(m20261016_150000_create_selic_daily::Migration),
            Box::new(m20261016_160000_drop_finance_api_settings::Migration),
            Box::new(m20261017_100000_seed_crypto_exchange_supply_ethbtc_thresholds::Migration),
            Box::new(m20261017_110000_add_coin_to_indicator_thresholds::Migration),
            Box::new(m20261017_120000_create_option_positions::Migration),
            Box::new(m20261017_130000_add_net_worth_to_option_positions::Migration),
            Box::new(m20261017_140000_create_workspace_settings::Migration),
            Box::new(m20261017_150000_add_currency_to_accounts_and_loans::Migration),
            Box::new(m20261017_160000_add_transfer_id_to_general_transactions::Migration),
            Box::new(m20261018_100000_create_ingestion_runs::Migration),
            Box::new(m20261019_100000_create_import_pending::Migration),
            Box::new(m20261020_100000_rebuild_import_pending_by_workspace::Migration),
            Box::new(m20261021_100000_seed_fixed_income_tax_rules::Migration),
            Box::new(m20261022_100000_update_fixed_income_cdb_rule_description::Migration),
            Box::new(m20261023_100000_add_mirrored_in_portfolio_to_bank_accounts::Migration),
            Box::new(m20261024_100000_update_jcp_rule_source::Migration),
            Box::new(m20261025_100000_add_auto_yield_to_bank_accounts::Migration),
            Box::new(m20261026_100000_create_stock_adjusted_close::Migration),
            Box::new(m20261027_100000_seed_crypto_tax_rules::Migration),
            Box::new(m20261027_110000_add_kind_to_tax_payments::Migration),
            Box::new(m20261028_100000_seed_crypto_progressive_rules::Migration),
            Box::new(m20261029_100000_add_foreign_tax_withheld_to_transactions::Migration),
            Box::new(m20261029_110000_seed_foreign_tax_rules::Migration),
            Box::new(m20261030_100000_create_ptax_daily::Migration),
            Box::new(m20261031_100000_create_offshore_entities::Migration),
            Box::new(m20261031_110000_seed_offshore_tax_rules::Migration),
            Box::new(m20261101_100000_create_funds::Migration),
            Box::new(m20261101_110000_seed_come_cotas_rules::Migration),
            Box::new(m20261102_100000_seed_fund_redemption_rules::Migration),
            Box::new(m20261103_100000_add_cvm_fields_to_funds::Migration),
            Box::new(m20261104_100000_add_administrator_to_funds::Migration),
            Box::new(m20261105_100000_seed_closed_fund_rules::Migration),
            Box::new(m20261106_100000_add_fund_structure_and_estoque::Migration),
            Box::new(m20261107_100000_add_loss_credit_used_to_transactions::Migration),
            Box::new(m20261108_100000_add_tax_return_fields_to_assets::Migration),
            Box::new(m20261109_100000_add_foreign_tax_paid_on_to_transactions::Migration),
            Box::new(m20261110_100000_create_offshore_distributions::Migration),
            Box::new(m20261111_100000_add_regime_to_offshore_entities::Migration),
            Box::new(m20261112_100000_add_update_2023_value_to_assets::Migration),
            Box::new(m20261113_100000_add_update_2023_to_offshore_entities::Migration),
            Box::new(m20261114_100000_create_offshore_capital_events::Migration),
        ]
    }
}
