// Testes de fluxo: cada um percorre uma funcionalidade de ponta a ponta contra
// o schema **real** (as migrations do app, com seeds, num SQLite em memória),
// chamando as mesmas funções que os comandos Tauri usam. Pegam o que os testes
// de lógica pura não pegam: coluna que a migration não criou, seed faltando,
// lançamento de caixa que não bate, regra fiscal semeada diferente do esperado.

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::commands::crypto_indicator::{
    apply_threshold, crypto_score_for, record_crypto_indicator_in, RecordCryptoIndicatorRequest,
};
use crate::commands::currency::{base_currency, positions_in_base, save_base_currency, sum_in_base};
use crate::commands::profitability::{compare_with_benchmarks, compute_profitability};
use crate::commands::proventos::proventos_summary_in;
use crate::commands::transaction::list_transactions_in;
use crate::commands::loan::{
    create_borrower_in, create_loan_in, delete_loan_in, list_loans_view, register_loan_repayment_in,
    update_loan_in, CreateBorrowerRequest, CreateLoanRequest, RegisterLoanRepaymentRequest,
    UpdateLoanRequest,
};
use crate::commands::net_worth::compute_net_worth;
use crate::commands::option_position::{
    close_option_position_in, create_option_position_in, create_option_positions_in,
    delete_option_position_in, list_option_positions_view, option_marks_in, save_option_marks_in,
    set_option_position_net_worth_in, CloseOptionPositionRequest, CreateOptionPositionRequest,
    CreateOptionPositionsRequest, OptionMark, SetOptionNetWorthRequest, StrategyLegRequest,
};
use crate::finance_core::b3_options::{LastTrade, OptionsMarket};
use crate::commands::tax::{assess_workspace, simulate_tax_sale_in, SimulateTaxSaleRequest};
use crate::domain::crypto_score::{
    default_thresholds, CryptoScore, ScoreVerdict, ADA_DEFAULT_THRESHOLDS, BTC_DEFAULT_THRESHOLDS,
    ETH_DEFAULT_THRESHOLDS, XRP_DEFAULT_THRESHOLDS,
};
use crate::entity::{indicator_thresholds, tax_rules};
use crate::commands::bank_account::{
    create_bank_account_in, delete_bank_account_in, list_bank_accounts_view, update_bank_account_in,
    CreateBankAccountRequest, UpdateBankAccountRequest,
};
use crate::commands::general_transaction::{
    create_transfer_in, delete_general_transaction_in, update_general_transaction_in, CreateTransferRequest,
    UpdateGeneralTransactionRequest,
};
use crate::commands::general_transaction_category::list_categories_view;
use crate::test_support::{
    asset, asset_in, bank_account, bank_account_in, cash_in, memory_db, portfolio, trade, workspace,
};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

// ---------------------------------------------------------------------------
// Migrations e seeds
// ---------------------------------------------------------------------------

#[tokio::test]
async fn migrations_apply_from_scratch_and_are_idempotent() {
    let db = memory_db().await;
    // Rodar de novo num banco já migrado não pode falhar nem duplicar seed.
    crate::db::migrate(&db).await.expect("second migrate run");

    let thresholds = indicator_thresholds::Entity::find().all(&db).await.unwrap();
    assert_eq!(
        thresholds.len(),
        ETH_DEFAULT_THRESHOLDS.len()
            + BTC_DEFAULT_THRESHOLDS.len()
            + ADA_DEFAULT_THRESHOLDS.len()
            + XRP_DEFAULT_THRESHOLDS.len()
    );
}

#[tokio::test]
async fn seeded_thresholds_match_the_reset_defaults() {
    // Os defaults por moeda (o Reset do editor) duplicam os seeds das migrations;
    // este teste falha se um mudar sem o outro. As linhas do ETH que existiam
    // antes da coluna `coin` viraram `coin = 'ETH'` na migration.
    let db = memory_db().await;
    let rows = indicator_thresholds::Entity::find().all(&db).await.unwrap();

    for coin in ["ETH", "BTC", "ADA", "XRP"] {
        for (indicator, green, red) in default_thresholds(coin) {
            let row = rows
                .iter()
                .find(|r| r.coin == coin && r.indicator == *indicator)
                .unwrap_or_else(|| panic!("no seeded threshold for {coin} {indicator}"));
            assert!(close(row.green_boundary, *green), "{coin} {indicator} green");
            assert!(close(row.red_boundary, *red), "{coin} {indicator} red");
        }
        assert_eq!(rows.iter().filter(|r| r.coin == coin).count(), default_thresholds(coin).len());
    }
}

#[tokio::test]
async fn tax_rules_are_seeded() {
    let db = memory_db().await;
    let rules = tax_rules::Entity::find().all(&db).await.unwrap();
    assert!(!rules.is_empty());
    for key in ["crypto_exemption_monthly_sales", "crypto_rate", "crypto_first_tier_gain_limit", "crypto_second_tier_gain_limit", "crypto_third_tier_gain_limit", "crypto_rate_second_tier", "crypto_rate_third_tier", "crypto_rate_fourth_tier", "darf_code_crypto"] {
        assert!(rules.iter().any(|r| r.rule_key == key), "regra de cripto semeada: {key}");
    }
}

// ---------------------------------------------------------------------------
// Empréstimos: caixa, patrimônio e a flag por empréstimo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn loan_flow_moves_cash_and_net_worth() {
    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let account = bank_account(&db, ws.id, "Checking").await;
    cash_in(&db, ws.id, account.id, 5000.0, "2026-01-01").await;

    let borrower = create_borrower_in(
        &db,
        CreateBorrowerRequest { workspace_id: ws.id, nome: "Mom".to_string(), notes: None },
    )
    .await
    .unwrap();

    // Empréstimo de 1.000 saindo da conta: o caixa cai e o a receber entra.
    let loan = create_loan_in(
        &db,
        CreateLoanRequest {
            workspace_id: ws.id,
            borrower_id: borrower.id,
            bank_account_id: Some(account.id),
            principal: 1000.0,
            taxa_mensal_percentual: None,
            data_inicio: "2026-02-01".to_string(),
            include_in_net_worth: true,
            notes: None,
            currency: None,
        },
    )
    .await
    .unwrap();

    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_cash, 4000.0));
    assert!(close(nw.total_loans_receivable, 1000.0));
    assert!(close(nw.net_worth, 5000.0));

    // Baixa: recebeu 1.050 e baixou 1.000. O dinheiro que entrou é o recebido;
    // o saldo abate só o baixado; a diferença vira "extra recebido".
    register_loan_repayment_in(
        &db,
        RegisterLoanRepaymentRequest {
            loan_id: loan.id,
            data: "2026-03-01".to_string(),
            valor_recebido: 1050.0,
            valor_baixado: 1000.0,
            bank_account_id: Some(account.id),
            notes: None,
        },
    )
    .await
    .unwrap();

    let views = list_loans_view(&db, ws.id).await.unwrap();
    assert_eq!(views.len(), 1);
    assert!(close(views[0].saldo_a_receber, 0.0));
    assert!(close(views[0].extra_received, 50.0));
    assert_eq!(views[0].borrower_nome, "Mom");

    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_cash, 5050.0));
    assert!(close(nw.total_loans_receivable, 0.0));
    assert!(close(nw.net_worth, 5050.0));

    // Apagar o empréstimo desfaz os lançamentos de caixa dele (saída e baixa).
    delete_loan_in(&db, loan.id).await.unwrap();
    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_cash, 5000.0));
    assert!(list_loans_view(&db, ws.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn loan_flagged_out_of_net_worth_is_excluded_until_toggled() {
    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let borrower = create_borrower_in(
        &db,
        CreateBorrowerRequest { workspace_id: ws.id, nome: "Friend".to_string(), notes: None },
    )
    .await
    .unwrap();

    // Sem conta: só registro, não mexe no caixa.
    let loan = create_loan_in(
        &db,
        CreateLoanRequest {
            workspace_id: ws.id,
            borrower_id: borrower.id,
            bank_account_id: None,
            principal: 800.0,
            taxa_mensal_percentual: None,
            data_inicio: "2026-02-01".to_string(),
            include_in_net_worth: false,
            notes: None,
            currency: None,
        },
    )
    .await
    .unwrap();

    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_cash, 0.0));
    assert!(close(nw.total_loans_receivable, 0.0));

    update_loan_in(
        &db,
        UpdateLoanRequest {
            loan_id: loan.id,
            taxa_mensal_percentual: None,
            include_in_net_worth: true,
            notes: None,
        },
    )
    .await
    .unwrap();
    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_loans_receivable, 800.0));
    assert!(close(nw.net_worth, 800.0));
}

#[tokio::test]
async fn loan_for_a_borrower_of_another_workspace_is_rejected() {
    let db = memory_db().await;
    let mine = workspace(&db, "Mine").await;
    let other = workspace(&db, "Other").await;
    let borrower = create_borrower_in(
        &db,
        CreateBorrowerRequest { workspace_id: other.id, nome: "Stranger".to_string(), notes: None },
    )
    .await
    .unwrap();

    let result = create_loan_in(
        &db,
        CreateLoanRequest {
            workspace_id: mine.id,
            borrower_id: borrower.id,
            bank_account_id: None,
            principal: 100.0,
            taxa_mensal_percentual: None,
            data_inicio: "2026-02-01".to_string(),
            include_in_net_worth: true,
            notes: None,
            currency: None,
        },
    )
    .await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Impostos: prejuízo a compensar e simulação de venda, com as regras semeadas
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tax_loss_balance_and_sale_simulation_use_seeded_rules() {
    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let bova = asset(&db, "BOVA11", "etf_br").await;
    let ivvb = asset(&db, "IVVB11", "etf_br").await;

    // Jan: prejuízo de 1.000 em ETF (gaveta comum).
    trade(&db, pf.id, bova.id, "compra", 100.0, 10000.0, "2026-01-02").await;
    trade(&db, pf.id, bova.id, "venda", 100.0, 9000.0, "2026-01-20").await;
    trade(&db, pf.id, ivvb.id, "compra", 10.0, 3000.0, "2026-01-02").await;

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert_eq!(assessment.loss_balances.len(), 1);
    assert!(close(assessment.loss_balances[0].balance, 1000.0));

    // Simula vender o IVVB11 por 4.500 em fev: lucro 1.500, compensa 1.000 e
    // paga a alíquota semeada sobre os 500 restantes.
    let sim = simulate_tax_sale_in(
        &db,
        SimulateTaxSaleRequest {
            workspace_id: ws.id,
            asset_id: ivvb.id,
            quantity: 10.0,
            unit_price: 450.0,
            fee: None,
            date: "2026-02-10".to_string(),
        },
    )
    .await
    .unwrap();

    assert_eq!(sim.period, "2026-02");
    assert!(close(sim.sales[0].gain, 1500.0));
    assert!(close(sim.loss_used, 1000.0));
    let rate = assessment_common_rate(&sim);
    assert!(close(sim.month_tax_after, 500.0 * rate), "tax {} rate {rate}", sim.month_tax_after);
    assert!(sim.loss_balances_after.is_empty());

    // A simulação não grava nada.
    let after = assess_workspace(&db, ws.id).await.unwrap();
    assert!(close(after.loss_balances[0].balance, 1000.0));
}

/// Alíquota efetiva da gaveta comum, derivada do que a simulação devolveu
/// (imposto ÷ base tributável), pra o teste não repetir o número semeado.
fn assessment_common_rate(sim: &crate::domain::tax::assessment::SaleSimulation) -> f64 {
    let taxable = sim.sales[0].gain - sim.loss_used;
    sim.month_tax_after / taxable
}

#[tokio::test]
async fn tax_simulation_rejects_assets_outside_the_b3_scope() {
    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let btc = asset(&db, "BTC", "cripto").await;
    trade(&db, pf.id, btc.id, "compra", 1.0, 100.0, "2026-01-02").await;

    let result = simulate_tax_sale_in(
        &db,
        SimulateTaxSaleRequest {
            workspace_id: ws.id,
            asset_id: btc.id,
            quantity: 1.0,
            unit_price: 200.0,
            fee: None,
            date: "2026-02-10".to_string(),
        },
    )
    .await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Score cripto: leituras, grupos, editor de thresholds
// ---------------------------------------------------------------------------

fn reading(indicator: &str, raw_value: f64) -> RecordCryptoIndicatorRequest {
    RecordCryptoIndicatorRequest {
        coin: "ETH".to_string(),
        indicator: indicator.to_string(),
        reading_date: "2026-10-16".to_string(),
        raw_value,
        source: "test".to_string(),
    }
}

#[tokio::test]
async fn crypto_score_groups_readings_and_reacts_to_threshold_edits() {
    let db = memory_db().await;

    // Funding de 12% a.a. cai no meio dos seeds (verde ≤5, vermelho ≥20).
    record_crypto_indicator_in(&db, reading("funding_rate_7d", 12.0)).await.unwrap();
    record_crypto_indicator_in(&db, reading("mvrv_z_score", -1.0)).await.unwrap();

    let score = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!(score.total, 20);
    assert_eq!(score.logged, 2);
    assert_eq!(score.green, 1); // só o MVRV
    let derivatives = score.groups.iter().find(|g| g.label == "Derivatives & options").unwrap();
    assert_eq!((derivatives.green, derivatives.logged, derivatives.total), (0, 1, 5));
    assert_eq!(score.verdict, ScoreVerdict::ReduceRisk);

    // O usuário afrouxa o limite: 12% passa a ser verde, e o score acompanha
    // sem nova coleta (o editor reclassifica as leituras guardadas).
    apply_threshold(&db, "ETH", "funding_rate_7d", 15.0, 30.0).await.unwrap();
    let score = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!(score.green, 2);
}

#[tokio::test]
async fn btc_score_has_its_own_indicators_groups_and_thresholds() {
    let db = memory_db().await;

    let btc_reading = |indicator: &str, raw_value: f64| RecordCryptoIndicatorRequest {
        coin: "BTC".to_string(),
        ..reading(indicator, raw_value)
    };

    // 17% de funding: o vermelho do ETH começa em 20 (→ neutro), o do BTC em 15 (→ vermelho).
    record_crypto_indicator_in(&db, btc_reading("funding_rate_7d", 17.0)).await.unwrap();
    record_crypto_indicator_in(&db, reading("funding_rate_7d", 17.0)).await.unwrap();
    record_crypto_indicator_in(&db, btc_reading("mvrv_z_score", 1.0)).await.unwrap();

    let btc = crypto_score_for(&db, "BTC".to_string()).await.unwrap();
    assert_eq!((btc.total, btc.logged), (14, 2));
    assert_eq!(btc.groups.len(), 4); // sem Network fundamentals
    let derivatives = btc.groups.iter().find(|g| g.label == "Derivatives & options").unwrap();
    assert_eq!(derivatives.indicators[0].reading.as_ref().unwrap().signal, "RED");

    let eth = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!((eth.total, eth.logged), (20, 1));
    let derivatives = eth.groups.iter().find(|g| g.label == "Derivatives & options").unwrap();
    assert_eq!(derivatives.indicators[0].reading.as_ref().unwrap().signal, "NEUTRAL");
}

// Score próprio do ADA: 10 indicadores em 4 grupos, com os limites da migration
// `m20261116_100000`, isolados dos do ETH e do BTC.
#[tokio::test]
async fn ada_score_has_its_own_indicators_and_thresholds_isolated_from_eth_and_btc() {
    let db = memory_db().await;
    let ada_reading = |indicator: &str, raw_value: f64| RecordCryptoIndicatorRequest {
        coin: "ADA".to_string(),
        ..reading(indicator, raw_value)
    };

    // Funding de 12% a.a.: o ETH tem vermelho em 20 (→ neutro), o ADA em 10 (→ vermelho).
    // MVRV Z de 4,0: o ETH tem vermelho em 7 (→ neutro), o ADA em 3,5 (→ vermelho).
    record_crypto_indicator_in(&db, ada_reading("funding_rate_7d", 12.0)).await.unwrap();
    record_crypto_indicator_in(&db, ada_reading("mvrv_z_score", 4.0)).await.unwrap();
    record_crypto_indicator_in(&db, ada_reading("fear_greed_index", 12.0)).await.unwrap();
    record_crypto_indicator_in(&db, reading("funding_rate_7d", 12.0)).await.unwrap();
    record_crypto_indicator_in(&db, reading("mvrv_z_score", 4.0)).await.unwrap();

    let ada = crypto_score_for(&db, "ADA".to_string()).await.unwrap();
    assert_eq!((ada.total, ada.logged, ada.green), (10, 3, 1)); // só o Fear & Greed
    assert_eq!(ada.groups.len(), 4); // sem Network fundamentals
    let group = |score: &CryptoScore, label: &str| {
        score.groups.iter().find(|g| g.label == label).unwrap().indicators.len()
    };
    assert_eq!(group(&ada, "On-chain"), 4);
    assert_eq!(group(&ada, "Derivatives & options"), 3);
    let signal = |score: &CryptoScore, indicator: &str| {
        score
            .groups
            .iter()
            .flat_map(|g| g.indicators.iter())
            .find(|i| i.indicator == indicator)
            .and_then(|i| i.reading.as_ref())
            .map(|r| r.signal.clone())
    };
    assert_eq!(signal(&ada, "funding_rate_7d").as_deref(), Some("RED"));
    assert_eq!(signal(&ada, "mvrv_z_score").as_deref(), Some("RED"));

    let eth = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!((eth.total, eth.logged), (20, 2));
    assert_eq!(signal(&eth, "funding_rate_7d").as_deref(), Some("NEUTRAL"));
    assert_eq!(signal(&eth, "mvrv_z_score").as_deref(), Some("NEUTRAL"));

    // Sem limite para o que a moeda não tem: DVOL, netflow e dominância são recusados no ADA.
    for indicator in ["dvol", "put_call_ratio", "exchange_netflow", "btc_dominance", "staking_yield"] {
        assert!(record_crypto_indicator_in(&db, ada_reading(indicator, 1.0)).await.is_err(), "{indicator}");
    }

    // Editar o ADA reclassifica só as leituras do ADA...
    apply_threshold(&db, "ADA", "funding_rate_7d", 15.0, 30.0).await.unwrap();
    let ada = crypto_score_for(&db, "ADA".to_string()).await.unwrap();
    assert_eq!(signal(&ada, "funding_rate_7d").as_deref(), Some("GREEN"));
    let eth = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!(signal(&eth, "funding_rate_7d").as_deref(), Some("NEUTRAL"));

    // ...e editar o ETH não toca nas do ADA (o ETH é dono das moedas sem conjunto próprio).
    apply_threshold(&db, "ETH", "mvrv_z_score", 5.0, 6.0).await.unwrap();
    let eth = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!(signal(&eth, "mvrv_z_score").as_deref(), Some("GREEN"));
    let ada = crypto_score_for(&db, "ADA".to_string()).await.unwrap();
    assert_eq!(signal(&ada, "mvrv_z_score").as_deref(), Some("RED"));
}

// Score próprio do XRP: 9 indicadores em 4 grupos (o do ADA sem o Puell), com os
// limites da migration `m20261117_100000`, isolados dos do ETH e do ADA.
#[tokio::test]
async fn xrp_score_has_its_own_indicators_and_thresholds_isolated_from_eth_and_ada() {
    let db = memory_db().await;
    let for_coin = |coin: &str, indicator: &str, raw_value: f64| RecordCryptoIndicatorRequest {
        coin: coin.to_string(),
        ..reading(indicator, raw_value)
    };
    let signal = |score: &CryptoScore, indicator: &str| {
        score
            .groups
            .iter()
            .flat_map(|g| g.indicators.iter())
            .find(|i| i.indicator == indicator)
            .and_then(|i| i.reading.as_ref())
            .map(|r| r.signal.clone())
    };

    // MVRV Z de 6,0: o vermelho do ETH começa em 7 (→ neutro), o do XRP em 5 (→ vermelho),
    // e o do ADA em 3,5 (→ vermelho). Endereços ativos a -22%: o XRP só fica vermelho em -25 (→ neutro),
    // o ADA em -20 (→ vermelho).
    record_crypto_indicator_in(&db, for_coin("XRP", "mvrv_z_score", 6.0)).await.unwrap();
    record_crypto_indicator_in(&db, for_coin("XRP", "active_addresses_trend", -22.0)).await.unwrap();
    record_crypto_indicator_in(&db, for_coin("ADA", "active_addresses_trend", -22.0)).await.unwrap();
    record_crypto_indicator_in(&db, for_coin("ETH", "mvrv_z_score", 6.0)).await.unwrap();

    let xrp = crypto_score_for(&db, "XRP".to_string()).await.unwrap();
    assert_eq!((xrp.total, xrp.logged, xrp.green), (9, 2, 0));
    assert_eq!(xrp.groups.len(), 4); // sem Network fundamentals
    assert_eq!(signal(&xrp, "mvrv_z_score").as_deref(), Some("RED"));
    assert_eq!(signal(&xrp, "active_addresses_trend").as_deref(), Some("NEUTRAL"));

    let ada = crypto_score_for(&db, "ADA".to_string()).await.unwrap();
    assert_eq!(signal(&ada, "active_addresses_trend").as_deref(), Some("RED"));
    let eth = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!(signal(&eth, "mvrv_z_score").as_deref(), Some("NEUTRAL"));

    // O XRP não tem Puell (sem emissão no CoinMetrics) nem o que a Deribit e o netflow davam.
    for indicator in ["puell_multiple", "dvol", "put_call_ratio", "exchange_netflow", "btc_dominance"] {
        assert!(record_crypto_indicator_in(&db, for_coin("XRP", indicator, 1.0)).await.is_err(), "{indicator}");
    }
    // O ADA segue com o Puell.
    record_crypto_indicator_in(&db, for_coin("ADA", "puell_multiple", 1.0)).await.unwrap();

    // Editar o XRP reclassifica só as leituras do XRP...
    apply_threshold(&db, "XRP", "mvrv_z_score", 8.0, 10.0).await.unwrap();
    let xrp = crypto_score_for(&db, "XRP".to_string()).await.unwrap();
    assert_eq!(signal(&xrp, "mvrv_z_score").as_deref(), Some("GREEN"));
    let eth = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!(signal(&eth, "mvrv_z_score").as_deref(), Some("NEUTRAL"));

    // ...e editar o ETH não toca nas do XRP nem nas do ADA.
    apply_threshold(&db, "ETH", "mvrv_z_score", 1.0, 2.0).await.unwrap();
    let eth = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!(signal(&eth, "mvrv_z_score").as_deref(), Some("RED"));
    let xrp = crypto_score_for(&db, "XRP".to_string()).await.unwrap();
    assert_eq!(signal(&xrp, "mvrv_z_score").as_deref(), Some("GREEN"));
    let ada = crypto_score_for(&db, "ADA".to_string()).await.unwrap();
    assert_eq!(signal(&ada, "active_addresses_trend").as_deref(), Some("RED"));
}

// Lote 4 da Fase 31: o grupo "Market sentiment" tem os seeds da migration para
// as duas moedas, e a dominância do BTC só existe no score do ETH.
#[tokio::test]
async fn market_sentiment_group_is_seeded_for_both_coins_and_dominance_is_eth_only() {
    let db = memory_db().await;
    let for_btc = |indicator: &str, raw_value: f64| RecordCryptoIndicatorRequest {
        coin: "BTC".to_string(),
        ..reading(indicator, raw_value)
    };

    // Seeds: Fear & Greed 25/75, Mayer 1,3/2,0, dominância 55/65 (menor é melhor).
    record_crypto_indicator_in(&db, reading("fear_greed_index", 12.0)).await.unwrap();
    record_crypto_indicator_in(&db, reading("mayer_multiple", 1.6)).await.unwrap();
    record_crypto_indicator_in(&db, reading("btc_dominance", 70.0)).await.unwrap();
    record_crypto_indicator_in(&db, for_btc("fear_greed_index", 80.0)).await.unwrap();
    record_crypto_indicator_in(&db, for_btc("mayer_multiple", 1.1)).await.unwrap();

    let eth = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    let sentiment = eth.groups.iter().find(|g| g.label == "Market sentiment").unwrap();
    assert_eq!((sentiment.green, sentiment.logged, sentiment.total), (1, 3, 3));
    let signals: Vec<&str> =
        sentiment.indicators.iter().map(|i| i.reading.as_ref().unwrap().signal.as_str()).collect();
    assert_eq!(signals, ["GREEN", "NEUTRAL", "RED"]);

    let btc = crypto_score_for(&db, "BTC".to_string()).await.unwrap();
    let sentiment = btc.groups.iter().find(|g| g.label == "Market sentiment").unwrap();
    assert_eq!((sentiment.green, sentiment.logged, sentiment.total), (1, 2, 2));
    let signals: Vec<&str> =
        sentiment.indicators.iter().map(|i| i.reading.as_ref().unwrap().signal.as_str()).collect();
    assert_eq!(signals, ["RED", "GREEN"]);

    // O BTC não tem limite de dominância: registrar para ele é indicador desconhecido.
    assert!(record_crypto_indicator_in(&db, for_btc("btc_dominance", 60.0)).await.is_err());

    // O editor reclassifica as leituras guardadas, como nos demais indicadores.
    apply_threshold(&db, "ETH", "mayer_multiple", 1.7, 2.5).await.unwrap();
    let eth = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!(eth.groups.iter().find(|g| g.label == "Market sentiment").unwrap().green, 2);
}

// ---------------------------------------------------------------------------
// Posições em opções (Fase 18)
// ---------------------------------------------------------------------------

fn option_request(
    portfolio_id: i32,
    series: &str,
    side: &str,
    quantity: f64,
    premium: f64,
) -> CreateOptionPositionRequest {
    CreateOptionPositionRequest {
        portfolio_id,
        series_ticker: series.to_string(),
        underlying_ticker: "petr4".to_string(),
        option_type: "call".to_string(),
        strike: 60.0,
        expiration: "2099-12-20".to_string(),
        side: side.to_string(),
        quantity,
        premium,
        open_date: "2026-10-01".to_string(),
        open_fee: None,
        notes: None,
    }
}

fn close_request(id: i32, kind: &str, price: Option<f64>, date: &str) -> CloseOptionPositionRequest {
    CloseOptionPositionRequest {
        id,
        close_kind: kind.to_string(),
        close_price: price,
        close_date: date.to_string(),
        close_fee: None,
    }
}

#[tokio::test]
async fn option_positions_open_close_and_report_the_realized_result() {
    let db = memory_db().await;
    let ws = workspace(&db, "Options WS").await;
    let pf = portfolio(&db, ws.id, "Main").await;

    // Lançada a 1,50 (100 opções) e comprada a 2,00 (100 opções).
    let short = create_option_position_in(&db, option_request(pf.id, " petrk600 ", "short", 100.0, 1.5)).await.unwrap();
    let long = create_option_position_in(&db, option_request(pf.id, "PETRW550", "long", 100.0, 2.0)).await.unwrap();
    assert_eq!((short.series_ticker.as_str(), short.underlying_ticker.as_str()), ("PETRK600", "PETR4"));

    let listed = list_option_positions_view(&db, pf.id).await.unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().all(|p| p.is_open && !p.is_expired && p.realized_pnl.is_none()));

    // Lançada que vira pó: ganha o prêmio todo. Comprada vendida a 3,50: +150.
    let expired = close_option_position_in(&db, close_request(short.id, "expired", Some(99.0), "2026-10-17")).await.unwrap();
    assert_eq!(expired.close_price, Some(0.0)); // `expired` ignora o preço informado
    close_option_position_in(&db, close_request(long.id, "closed", Some(3.5), "2026-10-17")).await.unwrap();

    let listed = list_option_positions_view(&db, pf.id).await.unwrap();
    assert!(listed.iter().all(|p| !p.is_open));
    let pnl = |series: &str| listed.iter().find(|p| p.series_ticker == series).unwrap().realized_pnl.unwrap();
    assert!(close(pnl("PETRK600"), 150.0));
    assert!(close(pnl("PETRW550"), 150.0));

    // Não fecha duas vezes.
    assert!(close_option_position_in(&db, close_request(long.id, "closed", Some(1.0), "2026-10-18")).await.is_err());
}

#[tokio::test]
async fn option_position_validation_and_isolation_between_portfolios() {
    let db = memory_db().await;
    let ws = workspace(&db, "Options WS").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let other = portfolio(&db, ws.id, "Other").await;

    let bad = |tweak: fn(&mut CreateOptionPositionRequest)| {
        let mut request = option_request(pf.id, "PETRK600", "long", 100.0, 1.0);
        tweak(&mut request);
        request
    };
    assert!(create_option_position_in(&db, bad(|r| r.series_ticker = "  ".to_string())).await.is_err());
    assert!(create_option_position_in(&db, bad(|r| r.option_type = "swap".to_string())).await.is_err());
    assert!(create_option_position_in(&db, bad(|r| r.side = "both".to_string())).await.is_err());
    assert!(create_option_position_in(&db, bad(|r| r.strike = 0.0)).await.is_err());
    assert!(create_option_position_in(&db, bad(|r| r.quantity = 0.0)).await.is_err());
    assert!(create_option_position_in(&db, bad(|r| r.premium = -1.0)).await.is_err());
    assert!(create_option_position_in(&db, bad(|r| r.open_fee = Some(-1.0))).await.is_err());
    assert!(create_option_position_in(&db, bad(|r| r.expiration = "2026-09-01".to_string())).await.is_err()); // antes da abertura
    assert!(create_option_position_in(&db, bad(|r| r.open_date = "not-a-date".to_string())).await.is_err());
    assert!(create_option_position_in(&db, bad(|r| r.portfolio_id = 9_999)).await.is_err());

    let created = create_option_position_in(&db, option_request(pf.id, "PETRK600", "long", 100.0, 1.0)).await.unwrap();
    assert!(list_option_positions_view(&db, other.id).await.unwrap().is_empty());

    // Encerramento inválido: sem preço (fora `expired`), data antes da abertura, tipo desconhecido.
    assert!(close_option_position_in(&db, close_request(created.id, "closed", None, "2026-10-17")).await.is_err());
    assert!(close_option_position_in(&db, close_request(created.id, "closed", Some(1.0), "2026-09-01")).await.is_err());
    assert!(close_option_position_in(&db, close_request(created.id, "sold", Some(1.0), "2026-10-17")).await.is_err());
    assert!(close_option_position_in(&db, close_request(9_999, "expired", None, "2026-10-17")).await.is_err());
    // Nada disso mudou a posição.
    assert!(list_option_positions_view(&db, pf.id).await.unwrap()[0].is_open);

    delete_option_position_in(&db, created.id).await.unwrap();
    assert!(delete_option_position_in(&db, created.id).await.is_err());
}

#[tokio::test]
async fn an_open_position_past_its_expiry_is_flagged_and_marks_use_the_last_trade() {
    use std::collections::HashMap;

    let db = memory_db().await;
    let ws = workspace(&db, "Options WS").await;
    let pf = portfolio(&db, ws.id, "Main").await;

    let mut stale_request = option_request(pf.id, "PETRK500", "long", 100.0, 1.0);
    stale_request.expiration = "2026-10-10".to_string();
    create_option_position_in(&db, stale_request).await.unwrap();
    create_option_position_in(&db, option_request(pf.id, "PETRK600", "short", 100.0, 1.5)).await.unwrap();
    create_option_position_in(&db, option_request(pf.id, "PETRK700", "long", 100.0, 0.5)).await.unwrap();

    let listed = list_option_positions_view(&db, pf.id).await.unwrap();
    assert_eq!(listed.iter().filter(|p| p.is_expired).count(), 1);
    assert!(listed.iter().find(|p| p.series_ticker == "PETRK500").unwrap().days_to_expiry < 0);

    // Mercado de mentira: PETRK600 negociou a 2,00; PETRK700 nunca negociou.
    let mut last_trades = HashMap::new();
    last_trades.insert(
        "PETRK600".to_string(),
        LastTrade { trade_date: chrono::NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(), close_price: 2.0 },
    );
    let market = OptionsMarket { series: vec![], last_trades };
    let marks = option_marks_in(&db, pf.id, &market).await.unwrap();
    assert_eq!(marks.len(), 3);

    let mark = |series: &str| marks.iter().find(|m| m.series_ticker == series).unwrap();
    // Lançada a 1,50 e hoje a 2,00: perde 0,50 × 100.
    assert!(close(mark("PETRK600").unrealized_pnl.unwrap(), -50.0));
    assert_eq!(mark("PETRK600").last_trade_date.as_deref(), Some("2026-10-16"));
    assert_eq!(mark("PETRK700").unrealized_pnl, None);
    assert_eq!(mark("PETRK700").last_price, None);
}

#[tokio::test]
async fn closed_option_positions_feed_the_tax_assessment() {
    use crate::domain::tax::assessment::{Bucket, TaxWarning};

    let db = memory_db().await;
    let ws = workspace(&db, "Options Tax WS").await;
    let pf = portfolio(&db, ws.id, "Main").await;

    let open_on = |series: &str, side: &str, quantity: f64, premium: f64, open_date: &str| {
        let mut request = option_request(pf.id, series, side, quantity, premium);
        request.open_date = open_date.to_string();
        request
    };

    // Swing: lançada em 05/10, expira sem valor em 12/10 → ganha o prêmio, 100 × 1,50 = +150.
    let swing = create_option_position_in(&db, open_on("PETRK600", "short", 100.0, 1.5, "2026-10-05")).await.unwrap();
    close_option_position_in(&db, close_request(swing.id, "expired", None, "2026-10-12")).await.unwrap();
    // Day trade: comprada a 1,00 e vendida a 2,00 no mesmo dia → +100.
    let day = create_option_position_in(&db, open_on("PETRW550", "long", 100.0, 1.0, "2026-10-13")).await.unwrap();
    close_option_position_in(&db, close_request(day.id, "closed", Some(2.0), "2026-10-13")).await.unwrap();
    // Exercida: só avisa. Aberta: não entra.
    let exercised = create_option_position_in(&db, open_on("PETRK650", "long", 100.0, 1.0, "2026-10-06")).await.unwrap();
    close_option_position_in(&db, close_request(exercised.id, "exercised", Some(3.0), "2026-10-14")).await.unwrap();
    create_option_position_in(&db, open_on("PETRK700", "short", 100.0, 9.0, "2026-10-07")).await.unwrap();

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert_eq!(assessment.months.len(), 1);
    let month = &assessment.months[0];
    assert_eq!(month.period, "2026-10");
    assert!(close(month.stock_sales_total, 0.0));

    let common = month.buckets.iter().find(|b| b.bucket == Bucket::Common).unwrap();
    let day_trade = month.buckets.iter().find(|b| b.bucket == Bucket::DayTrade).unwrap();
    assert!(close(common.result, 150.0));
    assert!(close(common.tax, 22.5)); // 15 %, sem a isenção dos R$ 20 mil
    assert!(close(day_trade.result, 100.0));
    assert!(close(day_trade.tax, 20.0)); // 20 %
    assert!(close(month.gross_tax, 42.5));
    // IRRF: só o day trade retém (1 % do ganho de 100); o do swing não é estimado.
    assert!(close(month.irrf_withheld, 1.0));
    assert!(close(month.due, 41.5));

    assert_eq!(month.sales.len(), 2); // a exercida e a aberta ficam de fora
    assert!(assessment.warnings.iter().any(
        |w| matches!(w, TaxWarning::OptionExercised { ticker, .. } if ticker == "PETRK650")
    ));
}

#[tokio::test]
async fn an_option_loss_offsets_a_later_stock_gain_in_the_workspace_assessment() {
    let db = memory_db().await;
    let ws = workspace(&db, "Options Tax WS").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let ivvb = asset(&db, "IVVB11", "etf_br").await;
    trade(&db, pf.id, ivvb.id, "compra", 10.0, 3000.0, "2026-09-02").await;
    trade(&db, pf.id, ivvb.id, "venda", 10.0, 4500.0, "2026-10-10").await;

    // Setembro: comprada a 5,00 que vira pó → prejuízo de 500 na gaveta comum.
    let mut request = option_request(pf.id, "PETRK600", "long", 100.0, 5.0);
    request.open_date = "2026-09-03".to_string();
    let lost = create_option_position_in(&db, request).await.unwrap();
    close_option_position_in(&db, close_request(lost.id, "expired", None, "2026-09-18")).await.unwrap();

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    let october = assessment.months.iter().find(|m| m.period == "2026-10").unwrap();
    let common = october.buckets.iter().find(|b| b.bucket == crate::domain::tax::assessment::Bucket::Common).unwrap();
    // Lucro do ETF 1.500, compensa os 500 da opção: tributa 1.000 a 15 %.
    assert!(close(common.loss_used, 500.0));
    assert!(close(common.taxable, 1000.0));
    assert!(close(october.gross_tax, 150.0));
}

#[tokio::test]
async fn open_option_positions_count_in_net_worth_at_cost_and_then_at_the_stored_price() {
    let db = memory_db().await;
    let ws = workspace(&db, "Options NW WS").await;
    let account = bank_account(&db, ws.id, "Checking").await;
    cash_in(&db, ws.id, account.id, 5000.0, "2026-01-01").await;
    let pf = portfolio(&db, ws.id, "Main").await;

    // Sem posição, opções valem 0.
    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_options_value, 0.0));
    assert_eq!(nw.options_at_cost_count, 0);
    assert!(close(nw.net_worth, 5000.0));

    // Comprada (100 a R$ 2,00) e lançada (100 a R$ 1,50), sem preço atualizado: valem o prêmio.
    let long = create_option_position_in(&db, option_request(pf.id, "PETRK600", "long", 100.0, 2.0)).await.unwrap();
    let short = create_option_position_in(&db, option_request(pf.id, "PETRW550", "short", 100.0, 1.5)).await.unwrap();
    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_options_value, 200.0 - 150.0));
    assert_eq!(nw.options_at_cost_count, 2);
    assert!(close(nw.net_worth, 5050.0));
    assert!(close(nw.total_debt, 0.0)); // a lançada não vira "dívida": a alavancagem não muda

    // Depois da atualização de preços: comprada a 3,50 (+350), lançada a 2,00 (−200).
    let mark = |position: &crate::entity::option_position::Model, price: f64| OptionMark {
        position_id: position.id,
        series_ticker: position.series_ticker.clone(),
        last_price: Some(price),
        last_trade_date: Some("2026-10-16".to_string()),
        unrealized_pnl: None,
    };
    save_option_marks_in(&db, &[mark(&long, 3.5), mark(&short, 2.0)]).await.unwrap();
    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_options_value, 350.0 - 200.0));
    assert_eq!(nw.options_at_cost_count, 0);
    let listed = list_option_positions_view(&db, pf.id).await.unwrap();
    let stored = listed.iter().find(|p| p.series_ticker == "PETRK600").unwrap();
    assert_eq!((stored.mark_price, stored.mark_date.as_deref()), (Some(3.5), Some("2026-10-16")));
    assert!(stored.marked_at.is_some());
    // Resultado a mercado com o preço guardado: comprada a 2,00 vale 3,50 → +150; lançada a 1,50 vale 2,00 → −50.
    assert!(close(stored.market_pnl.unwrap(), 150.0));
    let stored_short = listed.iter().find(|p| p.series_ticker == "PETRW550").unwrap();
    assert!(close(stored_short.market_pnl.unwrap(), -50.0));
    assert_eq!(stored_short.value_at_cost, false);

    // Uma série sem negócio mantém o preço anterior.
    let unchanged = OptionMark {
        position_id: long.id,
        series_ticker: long.series_ticker.clone(),
        last_price: None,
        last_trade_date: None,
        unrealized_pnl: None,
    };
    save_option_marks_in(&db, &[unchanged]).await.unwrap();
    assert!(close(compute_net_worth(&db, ws.id).await.unwrap().total_options_value, 150.0));

    // Flag desligada tira a lançada do total; encerrada a comprada, ela sai por inteiro.
    set_option_position_net_worth_in(&db, SetOptionNetWorthRequest { id: short.id, include: false }).await.unwrap();
    assert!(close(compute_net_worth(&db, ws.id).await.unwrap().total_options_value, 350.0));
    close_option_position_in(&db, close_request(long.id, "closed", Some(3.5), "2026-10-17")).await.unwrap();
    assert!(close(compute_net_worth(&db, ws.id).await.unwrap().total_options_value, 0.0));
    set_option_position_net_worth_in(&db, SetOptionNetWorthRequest { id: short.id, include: true }).await.unwrap();
    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_options_value, -200.0));
    assert!(close(nw.net_worth, 5000.0 - 200.0));
}

#[tokio::test]
async fn an_open_but_expired_option_is_worth_nothing_in_net_worth() {
    let db = memory_db().await;
    let ws = workspace(&db, "Options NW WS").await;
    let pf = portfolio(&db, ws.id, "Main").await;

    let mut request = option_request(pf.id, "PETRK500", "long", 100.0, 2.0);
    request.open_date = "2026-09-01".to_string();
    request.expiration = "2026-10-10".to_string(); // já passou e a posição segue aberta
    create_option_position_in(&db, request).await.unwrap();

    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_options_value, 0.0));
    assert_eq!(nw.options_at_cost_count, 0);
}

fn strategy_leg(series: &str, option_type: &str, strike: f64, side: &str, premium: f64) -> StrategyLegRequest {
    StrategyLegRequest {
        series_ticker: series.to_string(),
        option_type: option_type.to_string(),
        strike,
        expiration: "2099-12-20".to_string(),
        side: side.to_string(),
        quantity: 100.0,
        premium,
    }
}

fn strategy_request(portfolio_id: i32, legs: Vec<StrategyLegRequest>) -> CreateOptionPositionsRequest {
    CreateOptionPositionsRequest {
        portfolio_id,
        underlying_ticker: " petr4 ".to_string(),
        open_date: "2026-10-17".to_string(),
        legs,
    }
}

#[tokio::test]
async fn a_strategy_is_saved_as_independent_positions_in_one_go() {
    let db = memory_db().await;
    let ws = workspace(&db, "Strategy WS").await;
    let pf = portfolio(&db, ws.id, "Main").await;

    // Trava de alta: compra a call 40 a R$ 3,00 e vende a call 45 a R$ 1,00.
    let created = create_option_positions_in(
        &db,
        strategy_request(
            pf.id,
            vec![
                strategy_leg("petrk400", "call", 40.0, "long", 3.0),
                strategy_leg("PETRK450", "call", 45.0, "short", 1.0),
            ],
        ),
    )
    .await
    .unwrap();
    assert_eq!(created.len(), 2);

    let listed = list_option_positions_view(&db, pf.id).await.unwrap();
    assert_eq!(listed.len(), 2);
    let long = listed.iter().find(|p| p.series_ticker == "PETRK400").unwrap();
    let short = listed.iter().find(|p| p.series_ticker == "PETRK450").unwrap();
    assert_eq!((long.side.as_str(), short.side.as_str()), ("long", "short"));
    assert!(listed.iter().all(|p| p.underlying_ticker == "PETR4" && p.open_date == "2026-10-17" && p.is_open));
    assert!(close(long.premium, 3.0) && close(short.premium, 1.0));
}

#[tokio::test]
async fn a_strategy_with_an_invalid_leg_saves_nothing() {
    let db = memory_db().await;
    let ws = workspace(&db, "Strategy WS").await;
    let pf = portfolio(&db, ws.id, "Main").await;

    // A segunda perna tem strike 0: a primeira, válida, também não pode ficar.
    let result = create_option_positions_in(
        &db,
        strategy_request(
            pf.id,
            vec![
                strategy_leg("PETRK400", "call", 40.0, "long", 3.0),
                strategy_leg("PETRK450", "call", 0.0, "short", 1.0),
            ],
        ),
    )
    .await;
    assert!(result.is_err());
    assert!(list_option_positions_view(&db, pf.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_strategy_needs_between_one_and_eight_legs_and_an_existing_portfolio() {
    let db = memory_db().await;
    let ws = workspace(&db, "Strategy WS").await;
    let pf = portfolio(&db, ws.id, "Main").await;

    assert!(create_option_positions_in(&db, strategy_request(pf.id, vec![])).await.is_err());
    let nine: Vec<StrategyLegRequest> =
        (0..9).map(|i| strategy_leg(&format!("PETRK{i}00"), "call", 40.0 + f64::from(i), "long", 1.0)).collect();
    assert!(create_option_positions_in(&db, strategy_request(pf.id, nine)).await.is_err());
    assert!(
        create_option_positions_in(&db, strategy_request(9_999, vec![strategy_leg("PETRK400", "call", 40.0, "long", 1.0)]))
            .await
            .is_err()
    );
    assert!(list_option_positions_view(&db, pf.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn setting_the_net_worth_flag_of_a_missing_position_is_an_error() {
    let db = memory_db().await;
    assert!(set_option_position_net_worth_in(&db, SetOptionNetWorthRequest { id: 9_999, include: false })
        .await
        .is_err());
}

#[tokio::test]
async fn deleting_a_portfolio_deletes_its_option_positions() {
    use crate::entity::portfolio as portfolio_entity;

    let db = memory_db().await;
    let ws = workspace(&db, "Options WS").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    create_option_position_in(&db, option_request(pf.id, "PETRK600", "long", 100.0, 1.0)).await.unwrap();

    portfolio_entity::Entity::delete_by_id(pf.id).exec(&db).await.unwrap();
    assert!(list_option_positions_view(&db, pf.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn recording_an_indicator_without_a_threshold_is_an_error() {
    let db = memory_db().await;
    assert!(record_crypto_indicator_in(&db, reading("made_up", 1.0)).await.is_err());
}

// ---------------------------------------------------------------------------
// Backup (Fase 16): exportar e restaurar o schema real, com dados e anexo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn backup_round_trips_the_real_schema_with_data_and_attachments() {
    use crate::backup::{export_snapshot, import_snapshot, BackupSnapshot};
    use crate::storage::{LocalFSProvider, StorageProvider};
    use rand::{rngs::OsRng, RngCore};

    let temp_dir = |label: &str| {
        std::env::temp_dir().join(format!("anchor_flow_backup_{label}_{:08x}", OsRng.next_u32()))
    };

    // Origem: dados de várias áreas (finanças, carteira, empréstimo, cripto) + 1 anexo.
    let source = memory_db().await;
    let ws = workspace(&source, "Backup WS").await;
    let account = bank_account(&source, ws.id, "Checking").await;
    cash_in(&source, ws.id, account.id, 2500.0, "2026-01-01").await;
    let pf = portfolio(&source, ws.id, "Main").await;
    let ivvb = asset(&source, "IVVB11", "etf_br").await;
    trade(&source, pf.id, ivvb.id, "compra", 10.0, 3000.0, "2026-01-02").await;
    let borrower = create_borrower_in(
        &source,
        CreateBorrowerRequest { workspace_id: ws.id, nome: "Mom".to_string(), notes: None },
    )
    .await
    .unwrap();
    create_loan_in(
        &source,
        CreateLoanRequest {
            workspace_id: ws.id,
            borrower_id: borrower.id,
            bank_account_id: Some(account.id),
            principal: 700.0,
            taxa_mensal_percentual: Some(1.5),
            data_inicio: "2026-02-01".to_string(),
            include_in_net_worth: false,
            notes: Some("acordo verbal".to_string()),
            currency: None,
        },
    )
    .await
    .unwrap();
    record_crypto_indicator_in(&source, reading("funding_rate_7d", 7.5)).await.unwrap();
    create_option_position_in(&source, option_request(pf.id, "PETRK600", "short", 100.0, 1.5)).await.unwrap();

    let source_dir = temp_dir("source");
    let source_provider = LocalFSProvider::new(source_dir.clone());
    source_provider.write("asset_attachments/1/nota.pdf", b"conteudo-do-anexo").unwrap();

    // Passa pelo JSON, como no arquivo de backup de verdade.
    let snapshot = export_snapshot(&source, &source_provider).await.unwrap();
    let json = serde_json::to_string(&snapshot).unwrap();
    let snapshot: BackupSnapshot = serde_json::from_str(&json).unwrap();
    assert!(snapshot.tables.len() > 50, "o schema real tem dezenas de tabelas");

    // Destino: mesmo schema, mas com dados diferentes que o restore precisa substituir.
    let target = memory_db().await;
    workspace(&target, "Lixo que some no restore").await;
    let target_dir = temp_dir("target");
    let target_provider = LocalFSProvider::new(target_dir.clone());

    let summary = import_snapshot(&target, &target_provider, snapshot).await.unwrap();
    assert_eq!(summary.attachment_count, 1);

    // Tabela por tabela, o destino ficou idêntico à origem.
    let expected = export_snapshot(&source, &source_provider).await.unwrap();
    let restored = export_snapshot(&target, &target_provider).await.unwrap();
    assert_eq!(restored.tables, expected.tables);
    assert_eq!(restored.attachments, expected.attachments);

    // E o app enxerga os dados restaurados (não só as linhas).
    let views = list_loans_view(&target, ws.id).await.unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].borrower_nome, "Mom");
    assert!(!views[0].include_in_net_worth);
    let score = crypto_score_for(&target, "ETH".to_string()).await.unwrap();
    assert_eq!(score.logged, 1);
    let positions = list_option_positions_view(&target, pf.id).await.unwrap();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].series_ticker, "PETRK600");
    assert_eq!(target_provider.export_all().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(source_dir);
    let _ = std::fs::remove_dir_all(target_dir);
}

// ---------------------------------------------------------------------------
// Moeda base (Fase 19)
// ---------------------------------------------------------------------------

async fn seed_price(db: &sea_orm::DatabaseConnection, ticker: &str, date: &str, price: f64) {
    use sea_orm::{ActiveModelTrait, Set};
    crate::entity::stock_price_history::ActiveModel {
        ticker: Set(ticker.to_string()),
        price_date: Set(date.to_string()),
        close_price: Set(price),
        source: Set("test".to_string()),
        fetched_at: Set("2026-10-16T00:00:00Z".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert price");
}

async fn seed_fx_rate(db: &sea_orm::DatabaseConnection, pair: &str, rate: f64) {
    seed_price(db, pair, "2026-10-16", rate).await;
}

#[tokio::test]
async fn a_workspace_without_settings_uses_brl_as_base() {
    let db = memory_db().await;
    let ws = workspace(&db, "Base").await;
    assert_eq!(base_currency(&db, ws.id).await.unwrap(), "BRL");
}

#[tokio::test]
async fn saving_the_base_currency_validates_and_keeps_one_row_per_workspace() {
    let db = memory_db().await;
    let ws = workspace(&db, "Base").await;

    assert!(save_base_currency(&db, ws.id, "VND").await.is_err());
    assert_eq!(save_base_currency(&db, ws.id, " usd ").await.unwrap(), "USD");
    assert_eq!(save_base_currency(&db, ws.id, "EUR").await.unwrap(), "EUR");

    assert_eq!(base_currency(&db, ws.id).await.unwrap(), "EUR");
    let rows = crate::entity::workspace_settings::Entity::find().all(&db).await.unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn usd_position_is_converted_to_the_brl_base() {
    let db = memory_db().await;
    let ws = workspace(&db, "Base").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let brl = asset(&db, "PETR4", "acao").await;
    let usd = asset_in(&db, "AAPL", "acao", "USD").await;
    trade(&db, pf.id, brl.id, "compra", 10.0, 500.0, "2026-01-10").await;
    trade(&db, pf.id, usd.id, "compra", 2.0, 100.0, "2026-01-10").await;
    seed_fx_rate(&db, "USDBRL", 5.0).await;

    let (positions, base) = positions_in_base(&db, pf.id).await.unwrap();
    assert_eq!(base, "BRL");
    let aapl = positions.iter().find(|p| p.ticker == "AAPL").unwrap();
    assert_eq!(aapl.currency, "BRL");
    assert_eq!(aapl.native_currency, "USD");
    assert!(!aapl.fx_missing);
    assert!(close(aapl.market_value.unwrap(), 500.0)); // 100 USD × 5
    assert!(close(aapl.average_buy_price.unwrap(), 250.0)); // 50 USD × 5
    // Sem cotação o preço atual é o custo: o câmbio não vira lucro.
    assert!(close(aapl.unrealized_pl.unwrap(), 0.0));
    let petr = positions.iter().find(|p| p.ticker == "PETR4").unwrap();
    assert!(close(petr.market_value.unwrap(), 500.0)); // BRL fica como está
}

#[tokio::test]
async fn usd_base_inverts_the_brl_pair() {
    let db = memory_db().await;
    let ws = workspace(&db, "Base").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let brl = asset(&db, "PETR4", "acao").await;
    trade(&db, pf.id, brl.id, "compra", 10.0, 500.0, "2026-01-10").await;
    seed_fx_rate(&db, "USDBRL", 5.0).await;
    save_base_currency(&db, ws.id, "USD").await.unwrap();

    let (positions, base) = positions_in_base(&db, pf.id).await.unwrap();
    assert_eq!(base, "USD");
    assert!(close(positions[0].market_value.unwrap(), 100.0)); // 500 BRL ÷ 5
    assert_eq!(positions[0].currency, "USD");
}

#[tokio::test]
async fn position_without_a_rate_is_flagged_and_loses_its_values() {
    let db = memory_db().await;
    let ws = workspace(&db, "Base").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let usd = asset_in(&db, "AAPL", "acao", "USD").await;
    trade(&db, pf.id, usd.id, "compra", 2.0, 100.0, "2026-01-10").await;

    let (positions, _) = positions_in_base(&db, pf.id).await.unwrap();
    assert!(positions[0].fx_missing);
    assert_eq!(positions[0].currency, "USD"); // sem conversão, segue na nativa
    assert!(positions[0].market_value.is_none());
    assert!(positions[0].average_buy_price.is_none());
}

#[tokio::test]
async fn dividends_are_summed_in_the_base_currency() {
    let db = memory_db().await;
    let ws = workspace(&db, "Base").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let brl = asset(&db, "PETR4", "acao").await;
    let usd = asset_in(&db, "AAPL", "acao", "USD").await;
    let eur = asset_in(&db, "SAP", "acao", "EUR").await;
    let a = trade(&db, pf.id, brl.id, "provento", 1.0, 10.0, "2026-05-01").await;
    let b = trade(&db, pf.id, usd.id, "provento", 1.0, 4.0, "2026-05-01").await;
    let c = trade(&db, pf.id, eur.id, "provento", 1.0, 99.0, "2026-05-01").await;
    seed_fx_rate(&db, "USDBRL", 5.0).await;
    seed_price(&db, "USDBRL", "2026-05-01", 4.0).await;

    // 10 BRL + 4 USD × 4 (a taxa do dia do provento, não a de hoje); o EUR sem
    // cotação fica de fora da soma.
    assert!(close(sum_in_base(&db, &[a, b, c], "BRL").await.unwrap(), 26.0));
}

#[tokio::test]
async fn unrealized_pl_includes_the_exchange_move_since_each_purchase() {
    let db = memory_db().await;
    let ws = workspace(&db, "Base").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let usd = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    // Comprou 2 por US$ 100 com o dólar a 5; hoje a ação vale US$ 60 e o dólar está a 6.
    trade(&db, pf.id, usd.id, "compra", 2.0, 100.0, "2026-01-10").await;
    seed_price(&db, "USDBRL", "2026-01-10", 5.0).await;
    seed_fx_rate(&db, "USDBRL", 6.0).await;
    seed_price(&db, "AAPL", "2026-10-16", 60.0).await;

    let (positions, _) = positions_in_base(&db, pf.id).await.unwrap();
    let aapl = &positions[0];
    assert!(close(aapl.average_buy_price.unwrap(), 250.0)); // 50 USD × 5 (não × 6)
    assert!(close(aapl.current_price.unwrap(), 360.0)); // 60 USD × 6
    assert!(close(aapl.market_value.unwrap(), 720.0));
    assert!(close(aapl.unrealized_pl.unwrap(), 220.0)); // em USD seria 20 × 6 = 120
    assert!(close(aapl.unrealized_pl_pct.unwrap(), 0.44)); // 220 ÷ 500
    // Preço: (60 − 50) × 2 × 5 = 100; câmbio: 120 USD × (6 − 5) = 120; juntos, os 220.
    assert!(close(aapl.unrealized_pl_price.unwrap(), 100.0));
    assert!(close(aapl.unrealized_pl_fx.unwrap(), 120.0));

    // Na base USD só o preço andou.
    save_base_currency(&db, ws.id, "USD").await.unwrap();
    let (in_usd, _) = positions_in_base(&db, pf.id).await.unwrap();
    assert!(close(in_usd[0].unrealized_pl.unwrap(), 20.0));
    // Na moeda do próprio ativo não há decomposição.
    assert!(in_usd[0].unrealized_pl_price.is_none() && in_usd[0].unrealized_pl_fx.is_none());
}

async fn expense(
    db: &sea_orm::DatabaseConnection,
    workspace_id: i32,
    account_id: i32,
    category_id: i32,
    valor: f64,
    date: &str,
) {
    use sea_orm::{ActiveModelTrait, Set};
    crate::entity::general_transaction::ActiveModel {
        workspace_id: Set(workspace_id),
        bank_account_id: Set(account_id),
        transaction_type: Set("despesa".to_string()),
        category_id: Set(Some(category_id)),
        valor: Set(valor),
        transaction_date: Set(date.to_string()),
        notes: Set(None),
        created_at: Set("2026-10-01T00:00:00Z".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert expense");
}

#[tokio::test]
async fn a_category_budget_sums_expenses_in_the_base_currency() {
    use chrono::Datelike;
    use sea_orm::{ActiveModelTrait, Set};
    let db = memory_db().await;
    let ws = workspace(&db, "Budget").await;
    let brl = bank_account(&db, ws.id, "Nubank").await;
    let usd = bank_account_in(&db, ws.id, "Chase", "USD").await;
    let category = crate::entity::general_transaction_category::ActiveModel {
        workspace_id: Set(ws.id),
        nome: Set("Food".to_string()),
        limite_mensal: Set(Some(200.0)),
        created_at: Set("2026-10-01T00:00:00Z".to_string()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let day = chrono::Utc::now().date_naive().with_day(1).unwrap().format("%Y-%m-%d").to_string();
    expense(&db, ws.id, brl.id, category.id, 100.0, &day).await;
    expense(&db, ws.id, usd.id, category.id, 10.0, &day).await;

    // Sem o par USD/BRL a despesa em dólar fica de fora e é contada.
    let without_rate = list_categories_view(&db, ws.id).await.unwrap();
    assert!(close(without_rate[0].spent_this_month, 100.0));
    assert_eq!(without_rate[0].spent_missing_fx, 1);
    assert_eq!(without_rate[0].base_currency, "BRL");

    seed_fx_rate(&db, "USDBRL", 5.0).await;
    let in_brl = list_categories_view(&db, ws.id).await.unwrap();
    assert!(close(in_brl[0].spent_this_month, 150.0)); // 100 + 10 × 5
    assert_eq!(in_brl[0].spent_missing_fx, 0);

    save_base_currency(&db, ws.id, "USD").await.unwrap();
    let in_usd = list_categories_view(&db, ws.id).await.unwrap();
    assert!(close(in_usd[0].spent_this_month, 30.0)); // 100 ÷ 5 + 10
    assert_eq!(in_usd[0].base_currency, "USD");
}

fn transfer_request(
    workspace_id: i32,
    from: i32,
    to: i32,
    saida: f64,
    entrada: f64,
) -> CreateTransferRequest {
    CreateTransferRequest {
        workspace_id,
        from_account_id: from,
        to_account_id: to,
        valor_saida: saida,
        valor_entrada: entrada,
        transaction_date: "2026-03-01".to_string(),
        notes: None,
    }
}

#[tokio::test]
async fn a_transfer_between_currencies_moves_both_balances_without_income_or_expense() {
    let db = memory_db().await;
    let ws = workspace(&db, "Move").await;
    let brl = bank_account(&db, ws.id, "Nubank").await;
    let usd = bank_account_in(&db, ws.id, "Chase", "USD").await;
    cash_in(&db, ws.id, brl.id, 1000.0, "2026-01-01").await;
    seed_fx_rate(&db, "USDBRL", 5.0).await;

    // R$ 500 saem e US$ 95 chegam (o câmbio de 5,26 já com spread).
    let result =
        create_transfer_in(&db, transfer_request(ws.id, brl.id, usd.id, 500.0, 95.0)).await.unwrap();
    assert_eq!((result.from_currency.as_str(), result.to_currency.as_str()), ("BRL", "USD"));
    assert!(close(result.effective_rate.unwrap(), 95.0 / 500.0));

    let accounts = list_bank_accounts_view(&db, ws.id).await.unwrap();
    let balance_of = |id: i32| accounts.iter().find(|a| a.id == id).unwrap().balance;
    assert!(close(balance_of(brl.id), 500.0));
    assert!(close(balance_of(usd.id), 95.0));

    // No patrimônio em BRL: 500 + 95 × 5; só o câmbio explica a diferença de R$ 25 sobre os R$ 1.000.
    let net_worth = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(net_worth.total_cash, 500.0 + 95.0 * 5.0));
}

#[tokio::test]
async fn a_transfer_is_not_counted_as_a_category_expense() {
    use chrono::Datelike;
    let db = memory_db().await;
    let ws = workspace(&db, "Move").await;
    let a = bank_account(&db, ws.id, "Nubank").await;
    let b = bank_account(&db, ws.id, "Itaú").await;
    cash_in(&db, ws.id, a.id, 1000.0, "2026-01-01").await;
    let category = crate::entity::general_transaction_category::ActiveModel {
        workspace_id: sea_orm::Set(ws.id),
        nome: sea_orm::Set("Everything".to_string()),
        limite_mensal: sea_orm::Set(None),
        created_at: sea_orm::Set("2026-10-01T00:00:00Z".to_string()),
        ..Default::default()
    };
    sea_orm::ActiveModelTrait::insert(category, &db).await.unwrap();

    let mut request = transfer_request(ws.id, a.id, b.id, 300.0, 300.0);
    request.transaction_date = chrono::Utc::now().date_naive().with_day(1).unwrap().format("%Y-%m-%d").to_string();
    create_transfer_in(&db, request).await.unwrap();

    let categories = list_categories_view(&db, ws.id).await.unwrap();
    assert!(close(categories[0].spent_this_month, 0.0));
    // As duas pernas são de transferência: nenhuma é receita nem despesa.
    let rows = crate::entity::general_transaction::Entity::find().all(&db).await.unwrap();
    let legs: Vec<_> = rows.iter().filter(|r| r.transfer_id.is_some()).collect();
    assert_eq!(legs.len(), 2);
    assert!(legs.iter().all(|r| r.transaction_type.starts_with("transferencia_")));
}

#[tokio::test]
async fn deleting_either_leg_of_a_transfer_deletes_both() {
    let db = memory_db().await;
    let ws = workspace(&db, "Move").await;
    let a = bank_account(&db, ws.id, "Nubank").await;
    let b = bank_account(&db, ws.id, "Itaú").await;
    cash_in(&db, ws.id, a.id, 1000.0, "2026-01-01").await;
    let id = create_transfer_in(&db, transfer_request(ws.id, a.id, b.id, 300.0, 300.0)).await.unwrap().transfer_id;

    // Os saldos já refletem a transferência.
    let before = list_bank_accounts_view(&db, ws.id).await.unwrap();
    assert!(close(before.iter().find(|x| x.id == b.id).unwrap().balance, 300.0));

    // A linha de entrada é a que não tem o mesmo id do transfer_id.
    let entrada = crate::entity::general_transaction::Entity::find()
        .filter(crate::entity::general_transaction::Column::TransactionType.eq("transferencia_entrada"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(entrada.id, id);
    delete_general_transaction_in(&db, entrada.id).await.unwrap();

    let after = list_bank_accounts_view(&db, ws.id).await.unwrap();
    assert!(close(after.iter().find(|x| x.id == a.id).unwrap().balance, 1000.0));
    assert!(close(after.iter().find(|x| x.id == b.id).unwrap().balance, 0.0));
}

#[tokio::test]
async fn deleting_an_account_removes_the_opposite_leg_of_its_transfers() {
    let db = memory_db().await;
    let ws = workspace(&db, "Move").await;
    let a = bank_account(&db, ws.id, "Nubank").await;
    let b = bank_account(&db, ws.id, "Itaú").await;
    cash_in(&db, ws.id, a.id, 1000.0, "2026-01-01").await;
    create_transfer_in(&db, transfer_request(ws.id, a.id, b.id, 300.0, 300.0)).await.unwrap();

    delete_bank_account_in(&db, b.id).await.unwrap();
    let accounts = list_bank_accounts_view(&db, ws.id).await.unwrap();
    assert_eq!(accounts.len(), 1);
    // Sem a perna de entrada, a de saída também some: os R$ 1.000 voltam inteiros pra conta de origem.
    assert!(close(accounts[0].balance, 1000.0));
}

#[tokio::test]
async fn invalid_transfers_are_rejected_and_a_transfer_cannot_be_edited() {
    let db = memory_db().await;
    let ws = workspace(&db, "Move").await;
    let other = workspace(&db, "Other").await;
    let a = bank_account(&db, ws.id, "Nubank").await;
    let b = bank_account(&db, ws.id, "Itaú").await;
    let foreign = bank_account(&db, other.id, "Elsewhere").await;
    let usd = bank_account_in(&db, ws.id, "Chase", "USD").await;

    assert!(create_transfer_in(&db, transfer_request(ws.id, a.id, a.id, 10.0, 10.0)).await.is_err());
    assert!(create_transfer_in(&db, transfer_request(ws.id, a.id, foreign.id, 10.0, 10.0)).await.is_err());
    assert!(create_transfer_in(&db, transfer_request(ws.id, a.id, b.id, 0.0, 0.0)).await.is_err());
    assert!(create_transfer_in(&db, transfer_request(ws.id, a.id, b.id, -5.0, -5.0)).await.is_err());
    // Mesma moeda tem de receber exatamente o que saiu.
    assert!(create_transfer_in(&db, transfer_request(ws.id, a.id, b.id, 100.0, 99.0)).await.is_err());
    // Moedas diferentes aceitam valores diferentes.
    assert!(create_transfer_in(&db, transfer_request(ws.id, a.id, usd.id, 100.0, 19.0)).await.is_ok());

    let leg = crate::entity::general_transaction::Entity::find().one(&db).await.unwrap().unwrap();
    let edit = UpdateGeneralTransactionRequest {
        general_transaction_id: leg.id,
        bank_account_id: leg.bank_account_id,
        transaction_type: "despesa".to_string(),
        category_id: None,
        valor: 1.0,
        transaction_date: "2026-03-01".to_string(),
        notes: None,
    };
    assert!(update_general_transaction_in(&db, edit).await.is_err());
}

#[tokio::test]
async fn the_dividends_summary_converts_each_payment_at_its_own_date() {
    let db = memory_db().await;
    let ws = workspace(&db, "Income").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let brl = asset(&db, "PETR4", "acao_br").await;
    let usd = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    let eur = asset_in(&db, "SAP", "acao_internacional", "EUR").await;
    trade(&db, pf.id, brl.id, "provento", 1.0, 10.0, "2026-05-01").await;
    trade(&db, pf.id, usd.id, "provento", 1.0, 4.0, "2026-05-01").await;
    trade(&db, pf.id, eur.id, "provento", 1.0, 99.0, "2026-05-01").await;
    seed_price(&db, "USDBRL", "2026-05-01", 4.0).await;
    seed_fx_rate(&db, "USDBRL", 5.0).await;

    let summary = proventos_summary_in(&db, pf.id).await.unwrap();
    assert_eq!(summary.base_currency, "BRL");
    // 10 BRL + 4 USD × 4 (a taxa do dia do pagamento, não a de hoje); o EUR sem câmbio fica de fora.
    assert!(close(summary.total_all_time, 26.0));
    let aapl = summary.rows.iter().find(|r| r.ticker == "AAPL").unwrap();
    assert!(close(aapl.total_value, 16.0) && close(aapl.native_total_value, 4.0));
    assert_eq!(aapl.native_currency, "USD");
    let sap = summary.rows.iter().find(|r| r.ticker == "SAP").unwrap();
    assert!(sap.fx_missing && close(sap.total_value, 0.0));
}

#[tokio::test]
async fn the_ledger_shows_each_entry_in_its_own_currency_and_in_the_base() {
    let db = memory_db().await;
    let ws = workspace(&db, "Ledger").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let usd = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    trade(&db, pf.id, usd.id, "compra", 2.0, 100.0, "2026-01-10").await;
    seed_price(&db, "USDBRL", "2026-01-10", 5.0).await;
    seed_fx_rate(&db, "USDBRL", 6.0).await;

    let entries = list_transactions_in(&db, pf.id).await.unwrap();
    assert_eq!(entries[0].currency, "USD");
    assert!(close(entries[0].total_value, 100.0));
    assert!(close(entries[0].total_value_in_base.unwrap(), 500.0)); // a taxa de 10/01, não a de hoje

    // Sem o par, o valor na base fica vazio em vez de errado.
    let ws2 = workspace(&db, "Ledger 2").await;
    let pf2 = portfolio(&db, ws2.id, "Main").await;
    let eur = asset_in(&db, "SAP", "acao_internacional", "EUR").await;
    trade(&db, pf2.id, eur.id, "compra", 1.0, 50.0, "2026-01-10").await;
    let without_rate = list_transactions_in(&db, pf2.id).await.unwrap();
    assert!(without_rate[0].total_value_in_base.is_none());
}

#[tokio::test]
async fn a_brl_position_with_a_usd_base_uses_the_rate_of_the_purchase_date() {
    let db = memory_db().await;
    let ws = workspace(&db, "Base").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let petr = asset(&db, "PETR4", "acao_br").await;
    // Comprou 10 por R$ 500 com o dólar a 5 (US$ 100); hoje vale R$ 600 com o dólar a 6 (US$ 100).
    trade(&db, pf.id, petr.id, "compra", 10.0, 500.0, "2026-01-10").await;
    seed_price(&db, "USDBRL", "2026-01-10", 5.0).await;
    seed_fx_rate(&db, "USDBRL", 6.0).await;
    seed_price(&db, "PETR4", "2026-10-16", 60.0).await;
    save_base_currency(&db, ws.id, "USD").await.unwrap();

    let (positions, _) = positions_in_base(&db, pf.id).await.unwrap();
    assert!(close(positions[0].average_buy_price.unwrap(), 10.0)); // R$ 50 ÷ 5
    assert!(close(positions[0].market_value.unwrap(), 100.0)); // R$ 600 ÷ 6
    assert!(close(positions[0].unrealized_pl.unwrap(), 0.0)); // a alta em reais foi só o câmbio
    // Em USD o ativo subiu 20% (R$ 500 → R$ 600) e o real perdeu o mesmo tanto: os efeitos se anulam.
    assert!(close(positions[0].unrealized_pl_price.unwrap() + positions[0].unrealized_pl_fx.unwrap(), 0.0));
    assert!(positions[0].unrealized_pl_price.unwrap() > 0.0);
    assert!(positions[0].unrealized_pl_fx.unwrap() < 0.0);
}

// Rentabilidade na moeda base. O TWR percorre os meses até hoje, então os
// casos compram no dia 1 do mês corrente (peso do fluxo > 0 em qualquer dia) e
// o fim do mês é "hoje" — uma linha só, sem depender da data da execução.
struct ProfitabilityScenario {
    portfolio_id: i32,
    workspace_id: i32,
    first_day: String,
    today: String,
    end_rate: f64,
}

async fn usd_stock_scenario(db: &sea_orm::DatabaseConnection) -> ProfitabilityScenario {
    use chrono::Datelike;
    let today = chrono::Utc::now().date_naive();
    let first_day = today.with_day(1).unwrap().format("%Y-%m-%d").to_string();
    let today = today.format("%Y-%m-%d").to_string();

    let ws = workspace(db, "Return").await;
    let pf = portfolio(db, ws.id, "Main").await;
    let usd = asset_in(db, "AAPL", "acao_internacional", "USD").await;
    trade(db, pf.id, usd.id, "compra", 2.0, 100.0, &first_day).await;
    seed_price(db, "AAPL", &today, 60.0).await;
    seed_price(db, "USDBRL", &first_day, 5.0).await;
    // No dia 1 o início e o fim do mês são a mesma data (uma linha por par e dia).
    let end_rate = if today == first_day { 5.0 } else { 6.0 };
    if today != first_day {
        seed_price(db, "USDBRL", &today, end_rate).await;
    }
    ProfitabilityScenario { portfolio_id: pf.id, workspace_id: ws.id, first_day, today, end_rate }
}

#[tokio::test]
async fn profitability_converts_flows_and_month_end_at_their_own_dates() {
    let db = memory_db().await;
    let s = usd_stock_scenario(&db).await;

    let rows = compute_profitability(&db, &[s.portfolio_id], None, "BRL").await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(close(rows[0].cf_total, 500.0)); // 100 USD × 5 no dia da compra
    assert!(close(rows[0].emv, 120.0 * s.end_rate)); // 2 × 60 USD na taxa do fim do mês
    assert!(rows[0].r_month_pct > 0.0);
}

#[tokio::test]
async fn profitability_in_usd_has_no_exchange_effect() {
    let db = memory_db().await;
    let s = usd_stock_scenario(&db).await;

    let rows = compute_profitability(&db, &[s.portfolio_id], None, "USD").await.unwrap();
    assert!(close(rows[0].cf_total, 100.0));
    assert!(close(rows[0].emv, 120.0)); // 2 × 60 USD
    // Só o preço andou (50 → 60): o retorno em USD é o do ativo, sem o câmbio
    // que soma ao retorno em BRL quando o dólar sobe no mês.
    let in_brl = compute_profitability(&db, &[s.portfolio_id], None, "BRL").await.unwrap();
    assert!(rows[0].r_month_pct <= in_brl[0].r_month_pct);
}

#[tokio::test]
async fn profitability_of_a_brl_portfolio_needs_no_exchange_history() {
    use chrono::Datelike;
    let db = memory_db().await;
    let today = chrono::Utc::now().date_naive();
    let first_day = today.with_day(1).unwrap().format("%Y-%m-%d").to_string();
    let ws = workspace(&db, "Return").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let petr = asset(&db, "PETR4", "acao_br").await;
    trade(&db, pf.id, petr.id, "compra", 10.0, 250.0, &first_day).await;
    seed_price(&db, "PETR4", &today.format("%Y-%m-%d").to_string(), 30.0).await;

    let rows = compute_profitability(&db, &[pf.id], None, "BRL").await.unwrap();
    assert!(close(rows[0].cf_total, 250.0));
    assert!(close(rows[0].emv, 300.0));
}

#[tokio::test]
async fn profitability_without_the_exchange_pair_names_the_pair() {
    use chrono::Datelike;
    let db = memory_db().await;
    let today = chrono::Utc::now().date_naive();
    let first_day = today.with_day(1).unwrap().format("%Y-%m-%d").to_string();
    let ws = workspace(&db, "Return").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let usd = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    trade(&db, pf.id, usd.id, "compra", 2.0, 100.0, &first_day).await;
    seed_price(&db, "AAPL", &today.format("%Y-%m-%d").to_string(), 60.0).await;

    let err = compute_profitability(&db, &[pf.id], None, "BRL").await.err().unwrap();
    assert!(err.to_string().contains("USD/BRL"), "mensagem: {err}");
}

#[tokio::test]
async fn comparison_hides_the_brl_benchmarks_outside_brl() {
    let db = memory_db().await;
    let s = usd_stock_scenario(&db).await;
    // Um benchmark que existiria em BRL: sem a regra, entraria na comparação.
    seed_price(&db, "^BVSP", &s.first_day, 100_000.0).await;
    seed_price(&db, "^BVSP", &s.today, 101_000.0).await;
    save_base_currency(&db, s.workspace_id, "USD").await.unwrap();

    let cmp = compare_with_benchmarks(&db, s.portfolio_id, "USD".to_string()).await.unwrap();
    assert_eq!(cmp.base_currency, "USD");
    assert!(cmp.benchmarks.is_empty());
    assert_eq!(cmp.portfolio.len(), 1);
}

// Moeda por conta (Fase 19)

async fn borrower_of(db: &sea_orm::DatabaseConnection, workspace_id: i32) -> i32 {
    create_borrower_in(
        db,
        CreateBorrowerRequest { workspace_id, nome: "Friend".to_string(), notes: None },
    )
    .await
    .unwrap()
    .id
}

fn loan_request(workspace_id: i32, borrower_id: i32, account: Option<i32>, principal: f64) -> CreateLoanRequest {
    CreateLoanRequest {
        workspace_id,
        borrower_id,
        bank_account_id: account,
        principal,
        taxa_mensal_percentual: None,
        data_inicio: "2026-02-01".to_string(),
        include_in_net_worth: true,
        notes: None,
        currency: None,
    }
}

#[tokio::test]
async fn net_worth_converts_each_account_and_loan_by_its_currency() {
    let db = memory_db().await;
    let ws = workspace(&db, "Abroad").await;
    let brl = bank_account(&db, ws.id, "Nubank").await;
    let usd = bank_account_in(&db, ws.id, "Chase", "USD").await;
    cash_in(&db, ws.id, brl.id, 1000.0, "2026-01-01").await;
    cash_in(&db, ws.id, usd.id, 400.0, "2026-01-01").await;
    seed_fx_rate(&db, "USDBRL", 5.0).await;
    let borrower = borrower_of(&db, ws.id).await;
    // 100 USD saem da conta em dólar: caixa USD 300 e a receber USD 100.
    create_loan_in(&db, loan_request(ws.id, borrower, Some(usd.id), 100.0)).await.unwrap();

    let in_brl = compute_net_worth(&db, ws.id).await.unwrap();
    assert_eq!(in_brl.base_currency, "BRL");
    assert!(close(in_brl.total_cash, 1000.0 + 300.0 * 5.0));
    assert!(close(in_brl.total_loans_receivable, 100.0 * 5.0));
    assert_eq!(in_brl.items_missing_fx, 0);

    save_base_currency(&db, ws.id, "USD").await.unwrap();
    let in_usd = compute_net_worth(&db, ws.id).await.unwrap();
    assert_eq!(in_usd.base_currency, "USD");
    assert!(close(in_usd.total_cash, 1000.0 / 5.0 + 300.0));
    assert!(close(in_usd.total_loans_receivable, 100.0));

    let accounts = list_bank_accounts_view(&db, ws.id).await.unwrap();
    let chase = accounts.iter().find(|a| a.id == usd.id).unwrap();
    assert_eq!(chase.currency, "USD");
    assert!(close(chase.balance, 300.0));
    assert!(close(chase.balance_in_base.unwrap(), 300.0));
}

#[tokio::test]
async fn net_worth_counts_a_usd_position_converted() {
    let db = memory_db().await;
    let ws = workspace(&db, "Abroad").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let usd = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    trade(&db, pf.id, usd.id, "compra", 2.0, 100.0, "2026-01-10").await;
    seed_fx_rate(&db, "USDBRL", 5.0).await;

    let summary = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(summary.total_market_value, 500.0)); // 100 USD × 5, não 100
}

#[tokio::test]
async fn an_account_without_an_exchange_rate_stays_out_of_the_total() {
    let db = memory_db().await;
    let ws = workspace(&db, "Abroad").await;
    let usd = bank_account_in(&db, ws.id, "Chase", "USD").await;
    cash_in(&db, ws.id, usd.id, 400.0, "2026-01-01").await;

    let summary = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(summary.total_cash, 0.0));
    assert_eq!(summary.items_missing_fx, 1);
    let accounts = list_bank_accounts_view(&db, ws.id).await.unwrap();
    assert!(accounts[0].balance_in_base.is_none());
    assert!(close(accounts[0].balance, 400.0)); // o saldo na moeda da conta segue certo
}

#[tokio::test]
async fn the_transaction_list_carries_the_account_currency_and_the_value_in_the_base() {
    use crate::commands::general_transaction::list_transactions_view;
    let db = memory_db().await;
    let ws = workspace(&db, "Abroad").await;
    let usd = bank_account_in(&db, ws.id, "Chase", "USD").await;
    let brl = bank_account_in(&db, ws.id, "Itaú", "BRL").await;
    cash_in(&db, ws.id, usd.id, 100.0, "2026-01-01").await;
    cash_in(&db, ws.id, brl.id, 70.0, "2026-01-02").await;

    // Sem câmbio: a moeda aparece, o equivalente não; a conta em BRL é a própria base.
    let before = list_transactions_view(&db, ws.id).await.unwrap();
    let in_usd = before.iter().find(|v| v.transaction.bank_account_id == usd.id).unwrap();
    assert_eq!(in_usd.currency, "USD");
    assert!(in_usd.valor_in_base.is_none());
    let in_brl = before.iter().find(|v| v.transaction.bank_account_id == brl.id).unwrap();
    assert_eq!(in_brl.currency, "BRL");
    assert!(close(in_brl.valor_in_base.unwrap(), 70.0));

    // US$ 100 a 5,00 = R$ 500 (a única taxa é a mais próxima do dia).
    seed_fx_rate(&db, "USDBRL", 5.0).await;
    let after = list_transactions_view(&db, ws.id).await.unwrap();
    let in_usd = after.iter().find(|v| v.transaction.bank_account_id == usd.id).unwrap();
    assert!(close(in_usd.valor_in_base.unwrap(), 500.0));
    assert!(close(in_usd.transaction.valor, 100.0)); // o valor original não muda
}

#[tokio::test]
async fn an_account_currency_only_changes_while_the_account_is_unused() {
    let db = memory_db().await;
    let ws = workspace(&db, "Abroad").await;
    let account = create_bank_account_in(
        &db,
        CreateBankAccountRequest {
            workspace_id: ws.id,
            nome: "Chase".to_string(),
            titular: "Me".to_string(),
            currency: Some("usd".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(account.currency, "USD");

    let update = |currency: &str| UpdateBankAccountRequest {
        bank_account_id: account.id,
        nome: "Chase".to_string(),
        titular: "Me".to_string(),
        currency: Some(currency.to_string()),
        mirrored_in_portfolio: None,
        auto_yield_pct_of_cdi: None,
        clear_auto_yield: false,
    };
    assert_eq!(update_bank_account_in(&db, update("EUR")).await.unwrap().currency, "EUR");
    assert!(update_bank_account_in(&db, update("VND")).await.is_err());

    cash_in(&db, ws.id, account.id, 10.0, "2026-01-01").await;
    assert!(update_bank_account_in(&db, update("GBP")).await.is_err());
    // Repetir a mesma moeda não conta como troca.
    assert_eq!(update_bank_account_in(&db, update("EUR")).await.unwrap().currency, "EUR");

    // A lista avisa a tela: a conta com lançamento está em uso.
    let accounts = list_bank_accounts_view(&db, ws.id).await.unwrap();
    assert!(accounts[0].in_use);
}

#[tokio::test]
async fn a_new_account_is_not_in_use_until_it_gets_a_transaction() {
    let db = memory_db().await;
    let ws = workspace(&db, "Abroad").await;
    let account = bank_account(&db, ws.id, "Nubank").await;
    assert!(!list_bank_accounts_view(&db, ws.id).await.unwrap()[0].in_use);

    cash_in(&db, ws.id, account.id, 10.0, "2026-01-01").await;
    assert!(list_bank_accounts_view(&db, ws.id).await.unwrap()[0].in_use);
}

#[tokio::test]
async fn a_transaction_cannot_move_between_accounts_of_different_currencies() {
    let db = memory_db().await;
    let ws = workspace(&db, "Abroad").await;
    let brl = bank_account(&db, ws.id, "Nubank").await;
    let brl2 = bank_account(&db, ws.id, "Itaú").await;
    let usd = bank_account_in(&db, ws.id, "Chase", "USD").await;
    let tx = cash_in(&db, ws.id, brl.id, 100.0, "2026-01-01").await;

    let request = |account_id: i32| UpdateGeneralTransactionRequest {
        general_transaction_id: tx.id,
        bank_account_id: account_id,
        transaction_type: "receita".to_string(),
        category_id: None,
        valor: 100.0,
        transaction_date: "2026-01-01".to_string(),
        notes: None,
    };
    assert!(update_general_transaction_in(&db, request(usd.id)).await.is_err());
    assert_eq!(update_general_transaction_in(&db, request(brl2.id)).await.unwrap().bank_account_id, brl2.id);
}

#[tokio::test]
async fn a_loan_takes_the_account_currency_and_repays_only_into_the_same_one() {
    let db = memory_db().await;
    let ws = workspace(&db, "Abroad").await;
    let brl = bank_account(&db, ws.id, "Nubank").await;
    let usd = bank_account_in(&db, ws.id, "Chase", "USD").await;
    let borrower = borrower_of(&db, ws.id).await;

    // Com conta, a moeda pedida é ignorada.
    let mut request = loan_request(ws.id, borrower, Some(usd.id), 100.0);
    request.currency = Some("BRL".to_string());
    let loan = create_loan_in(&db, request).await.unwrap();
    assert_eq!(loan.currency, "USD");

    let repay = |account_id: i32| RegisterLoanRepaymentRequest {
        loan_id: loan.id,
        data: "2026-03-01".to_string(),
        valor_recebido: 50.0,
        valor_baixado: 50.0,
        bank_account_id: Some(account_id),
        notes: None,
    };
    assert!(register_loan_repayment_in(&db, repay(brl.id)).await.is_err());
    assert!(register_loan_repayment_in(&db, repay(usd.id)).await.is_ok());

    // Sem conta, vale a moeda do pedido (e BRL por padrão).
    let mut no_account = loan_request(ws.id, borrower, None, 80.0);
    no_account.currency = Some("eur".to_string());
    assert_eq!(create_loan_in(&db, no_account).await.unwrap().currency, "EUR");
    assert_eq!(create_loan_in(&db, loan_request(ws.id, borrower, None, 80.0)).await.unwrap().currency, "BRL");
}

// ---------------------------------------------------------------------------
// Ingestão diária (Fase 33)
// ---------------------------------------------------------------------------

fn pair(a: &str, b: &str) -> (String, String) {
    (a.to_string(), b.to_string())
}

#[tokio::test]
async fn ingestion_targets_are_only_what_the_user_has() {
    use crate::entity::asset_favorites;
    use sea_orm::{ActiveModelTrait, Set};

    let db = memory_db().await;
    let ws = workspace(&db, "Ingestion").await;
    let pf = portfolio(&db, ws.id, "Main").await;

    let aapl = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    trade(&db, pf.id, aapl.id, "compra", 1.0, 100.0, "2026-01-02").await;
    // Favoritado sem lançamento: entra. Sem lançamento nem favorito: fica de fora.
    let petr = asset(&db, "PETR4", "acao_br").await;
    asset_favorites::ActiveModel {
        workspace_id: Set(ws.id),
        asset_id: Set(petr.id),
        created_at: Set("2026-01-01".to_string()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    asset(&db, "VALE3", "acao_br").await;
    // Sem cotação automática: nunca entra, mesmo com lançamento.
    let house = asset(&db, "CASA", "imovel").await;
    trade(&db, pf.id, house.id, "compra", 1.0, 500.0, "2026-01-03").await;

    bank_account_in(&db, ws.id, "Euro account", "EUR").await;
    bank_account_in(&db, ws.id, "Real account", "BRL").await;

    // Segundo workspace com outra base: o par é contra a base dele.
    let ws2 = workspace(&db, "Ingestion USD").await;
    save_base_currency(&db, ws2.id, "USD").await.unwrap();
    bank_account_in(&db, ws2.id, "Real in USD base", "BRL").await;

    let targets = crate::ingestion::collection_targets(&db).await.unwrap();
    assert_eq!(
        targets.assets.into_iter().collect::<Vec<_>>(),
        vec![pair("AAPL", "acao_internacional"), pair("PETR4", "acao_br")]
    );
    assert_eq!(
        targets.fx_pairs.into_iter().collect::<Vec<_>>(),
        vec![pair("BRL", "USD"), pair("EUR", "BRL"), pair("USD", "BRL")]
    );
}

#[tokio::test]
async fn fx_staleness_flags_only_the_pairs_the_workspace_uses() {
    let db = memory_db().await;
    let ws = workspace(&db, "Stale").await;
    bank_account_in(&db, ws.id, "Dollar account", "USD").await;
    bank_account_in(&db, ws.id, "Euro account", "EUR").await;
    seed_price(&db, "USDBRL", "2026-10-10", 5.0).await;
    seed_price(&db, "USDBRL", "2026-10-16", 5.1).await;
    // Par que ninguém usa não entra, mesmo velho.
    seed_price(&db, "GBPBRL", "2020-01-01", 6.0).await;

    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 18).unwrap();
    let stale = crate::ingestion::fx_staleness_in(&db, ws.id, today).await.unwrap();
    // USD/BRL tem fechamento de 2 dias atrás (vale); EUR/BRL nunca foi coletado.
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].pair, "EUR/BRL");
    assert!(stale[0].last_date.is_none());

    let much_later = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    let stale = crate::ingestion::fx_staleness_in(&db, ws.id, much_later).await.unwrap();
    assert_eq!(stale.iter().map(|s| s.pair.as_str()).collect::<Vec<_>>(), vec!["EUR/BRL", "USD/BRL"]);
}

#[tokio::test]
async fn a_currency_without_a_direct_pair_is_fresh_and_converts_through_the_usd_legs() {
    use crate::commands::currency::load_rates;

    let db = memory_db().await;
    let ws = workspace(&db, "Cruzada").await;
    bank_account_in(&db, ws.id, "Conta em guarani", "PYG").await;
    // O Yahoo não tem PYGBRL: só as pernas PYGUSD e USDBRL foram coletadas (0,000125 dólar por guarani, 5,00 reais por dólar)
    seed_price(&db, "PYGUSD", "2026-10-16", 0.000125).await;
    seed_price(&db, "USDBRL", "2026-10-16", 5.0).await;

    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 18).unwrap();
    let stale = crate::ingestion::fx_staleness_in(&db, ws.id, today).await.unwrap();
    assert!(stale.is_empty(), "as duas pernas têm fechamento de 2 dias atrás: {stale:?}");

    // 0,000125 × 5,00 = R$ 0,000625 por guarani
    let currencies: std::collections::HashSet<String> = ["PYG".to_string()].into_iter().collect();
    let rates = load_rates(&db, "BRL", &currencies).await.unwrap();
    assert!(close(rates.rate("PYG").unwrap(), 0.000625), "{:?}", rates.rate("PYG"));

    // sem a perna USDBRL a rota não fecha: volta a ser "nunca coletado"
    let db = memory_db().await;
    let ws = workspace(&db, "Perna só").await;
    bank_account_in(&db, ws.id, "Conta em guarani", "PYG").await;
    seed_price(&db, "PYGUSD", "2026-10-16", 0.000125).await;
    let stale = crate::ingestion::fx_staleness_in(&db, ws.id, today).await.unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].pair, "PYG/BRL");
    assert!(stale[0].last_date.is_none());
}

#[tokio::test]
async fn ingestion_with_nothing_owned_still_lists_no_targets() {
    let db = memory_db().await;
    workspace(&db, "Empty").await;
    let targets = crate::ingestion::collection_targets(&db).await.unwrap();
    assert!(targets.assets.is_empty());
    assert!(targets.fx_pairs.is_empty());
}

#[tokio::test]
async fn ingestion_yields_to_a_running_manual_collection() {
    use crate::entity::ingestion_runs;
    use std::sync::atomic::{AtomicBool, Ordering};

    let db = memory_db().await;
    let lock = AtomicBool::new(true);
    let err = crate::ingestion::run_ingestion(&db, &lock, "scheduled").await.unwrap_err();
    assert!(matches!(err, crate::error::AppError::CollectorBusy));
    assert!(ingestion_runs::Entity::find().all(&db).await.unwrap().is_empty());
    // Quem segurava o trinco continua segurando: a ingestão não o libera.
    assert!(lock.load(Ordering::SeqCst));
}

#[tokio::test]
async fn the_next_ingestion_waits_only_on_finished_runs() {
    use crate::entity::ingestion_runs;
    use sea_orm::{ActiveModelTrait, Set};

    let db = memory_db().await;
    assert!(crate::ingestion::last_finished_at(&db).await.unwrap().is_none());

    let add = |status: &str, finished: Option<&str>| {
        ingestion_runs::ActiveModel {
            trigger_kind: Set("scheduled".to_string()),
            started_at: Set("2026-10-17T00:00:00+00:00".to_string()),
            finished_at: Set(finished.map(str::to_string)),
            status: Set(status.to_string()),
            items_ok: Set(0),
            items_failed: Set(0),
            ..Default::default()
        }
        .insert(&db)
    };
    add("partial", Some("2026-10-15T10:00:00+00:00")).await.unwrap();
    // Rodada que falhou inteira ou nunca fechou não conta como "atualizado".
    add("error", Some("2026-10-17T10:00:00+00:00")).await.unwrap();
    add("running", None).await.unwrap();

    let last = crate::ingestion::last_finished_at(&db).await.unwrap().unwrap();
    assert_eq!(last.to_rfc3339(), "2026-10-15T10:00:00+00:00");

    // A conferência do laço: devida a partir de 24 h depois do fim da última rodada boa.
    let at = |s: &str| chrono::DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&chrono::Utc);
    assert!(!crate::ingestion::is_due_now(&db, at("2026-10-16T09:59:00+00:00")).await.unwrap());
    assert!(crate::ingestion::is_due_now(&db, at("2026-10-16T10:00:00+00:00")).await.unwrap());
}

#[tokio::test]
async fn a_database_without_runs_is_due_for_the_first_ingestion() {
    let db = memory_db().await;
    assert!(crate::ingestion::is_due_now(&db, chrono::Utc::now()).await.unwrap());
}

#[tokio::test]
async fn the_tax_is_only_paid_from_a_brl_account() {
    use crate::commands::tax::{mark_tax_paid_in, MarkTaxPaidRequest};
    use crate::entity::{general_transaction, tax_payments};

    let db = memory_db().await;
    let ws = workspace(&db, "Tax payment").await;
    let usd = bank_account_in(&db, ws.id, "Dollar account", "USD").await;
    let brl = bank_account(&db, ws.id, "Real account").await;
    let request = |account_id: i32| MarkTaxPaidRequest {
        workspace_id: ws.id,
        period: "2026-09".to_string(),
        amount: 150.0,
        paid_date: "2026-10-10".to_string(),
        bank_account_id: account_id,
        kind: "b3".to_string(),
    };

    let err = mark_tax_paid_in(&db, request(usd.id)).await.unwrap_err();
    assert!(matches!(err, crate::error::AppError::InvalidInput(_)));
    // A recusa não deixa pagamento nem despesa pela metade.
    assert!(tax_payments::Entity::find().all(&db).await.unwrap().is_empty());
    assert!(general_transaction::Entity::find().all(&db).await.unwrap().is_empty());

    let payment = mark_tax_paid_in(&db, request(brl.id)).await.unwrap();
    assert!(close(payment.amount, 150.0));
    let expense = general_transaction::Entity::find_by_id(payment.general_transaction_id.unwrap())
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(expense.transaction_type, "despesa");
    assert_eq!(expense.bank_account_id, brl.id);
}

#[tokio::test]
async fn crypto_tax_is_assessed_apart_and_its_darf_is_paid_apart_from_the_exchange_one() {
    use crate::commands::tax::{mark_tax_paid_in, MarkTaxPaidRequest};
    use crate::entity::{general_transaction, tax_payments};

    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let account = bank_account(&db, ws.id, "Real account").await;
    let btc = asset(&db, "BTC", "cripto").await;
    let bova = asset(&db, "BOVA11", "etf_br").await;

    // Fev: BTC vendido por 40 mil (acima dos 35 mil de isenção) com ganho de 30 mil: 15% = 4.500, DARF 4600.
    trade(&db, pf.id, btc.id, "compra", 1.0, 10000.0, "2026-01-02").await;
    trade(&db, pf.id, btc.id, "venda", 1.0, 40000.0, "2026-02-10").await;
    // No mesmo mês, um ETF com lucro de 1.000 na B3: DARF 6015, que não se mistura com o da cripto.
    trade(&db, pf.id, bova.id, "compra", 10.0, 1000.0, "2026-01-02").await;
    trade(&db, pf.id, bova.id, "venda", 10.0, 2000.0, "2026-02-12").await;

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert_eq!(assessment.months.len(), 1);
    assert_eq!(assessment.months[0].darf_code, 6015);
    assert!(close(assessment.months[0].gross_tax, 150.0), "só o ETF: {}", assessment.months[0].gross_tax);
    assert_eq!(assessment.crypto_months.len(), 1);
    let crypto = &assessment.crypto_months[0];
    assert_eq!((crypto.period.as_str(), crypto.darf_code), ("2026-02", 4600));
    assert!(!crypto.exempt && close(crypto.due, 4500.0), "{}", crypto.due);

    let pay = |kind: &str| MarkTaxPaidRequest {
        workspace_id: ws.id,
        period: "2026-02".to_string(),
        amount: if kind == "crypto" { 4500.0 } else { 149.9 },
        paid_date: "2026-03-31".to_string(),
        bank_account_id: account.id,
        kind: kind.to_string(),
    };
    let b3 = mark_tax_paid_in(&db, pay("b3")).await.unwrap();
    let crypto_payment = mark_tax_paid_in(&db, pay("crypto")).await.unwrap();
    assert_eq!((b3.kind.as_str(), crypto_payment.kind.as_str()), ("b3", "crypto"));
    assert_eq!(tax_payments::Entity::find().all(&db).await.unwrap().len(), 2, "o mesmo mês aceita um pagamento de cada tipo");

    // O mesmo tipo duas vezes no mesmo mês continua recusado, e um tipo desconhecido também.
    assert!(mark_tax_paid_in(&db, pay("crypto")).await.is_err());
    assert!(matches!(mark_tax_paid_in(&db, pay("fii")).await.unwrap_err(), crate::error::AppError::InvalidInput(_)));
    assert_eq!(tax_payments::Entity::find().all(&db).await.unwrap().len(), 2);

    let expense = general_transaction::Entity::find_by_id(crypto_payment.general_transaction_id.unwrap())
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(expense.notes.unwrap().contains("crypto"));
}

// ---------------------------------------------------------------------------
// Carteira: custo médio, vendas, taxas e eventos corporativos (Fase 34)
// ---------------------------------------------------------------------------

fn tx_request(
    portfolio_id: i32,
    asset_id: i32,
    kind: &str,
    quantity: f64,
    unit_price: f64,
    date: &str,
    fee: f64,
) -> crate::commands::transaction::CreateTransactionRequest {
    crate::commands::transaction::CreateTransactionRequest {
        portfolio_id,
        asset_id: Some(asset_id),
        custodia_id: None,
        transfer_to_custodia_id: None,
        transaction_type: kind.to_string(),
        payment_type: None,
        quantity: Some(quantity),
        unit_price: Some(unit_price),
        total_value: quantity * unit_price,
        fee: (fee > 0.0).then_some(fee),
        foreign_tax_withheld: None,
        foreign_tax_paid_on: None,
        loss_credit_used: None,
        transaction_date: date.to_string(),
        notes: None,
        fi_emissor: None,
        fi_indexador: None,
        fi_taxa_percentual: None,
        fi_data_vencimento: None,
        fi_liquidez: None,
    }
}

/// Desdobramento/grupamento: só a razão em `quantity`, sem preço nem valor.
fn corporate_request(
    portfolio_id: i32,
    asset_id: i32,
    kind: &str,
    ratio: f64,
    date: &str,
) -> crate::commands::transaction::CreateTransactionRequest {
    let mut request = tx_request(portfolio_id, asset_id, kind, ratio, 0.0, date, 0.0);
    request.unit_price = None;
    request.total_value = 0.0;
    request
}

async fn add(db: &sea_orm::DatabaseConnection, request: crate::commands::transaction::CreateTransactionRequest) {
    crate::commands::transaction::create_transaction_in(db, request).await.expect("create transaction");
}

#[tokio::test]
async fn a_realistic_portfolio_history_gives_the_hand_computed_position() {
    use crate::commands::transaction::{compute_positions, REVERSE_SPLIT, SPLIT};
    let _ = REVERSE_SPLIT;

    let db = memory_db().await;
    let ws = workspace(&db, "Realistic").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let petr = asset(&db, "PETR4", "acao_br").await;

    // 1) 100 @ 10 + taxa 5            custo 1005
    add(&db, tx_request(pf.id, petr.id, "compra", 100.0, 10.0, "2026-01-05", 5.0)).await;
    // 2) vende 40 @ 15 − taxa 3       custo baixado 40 × 10,05 = 402; recebe 597; ganho 195
    add(&db, tx_request(pf.id, petr.id, "venda", 40.0, 15.0, "2026-02-10", 3.0)).await;
    let positions = compute_positions(&db, pf.id).await.unwrap();
    assert!(close(positions[0].quantity, 60.0));
    assert!(close(positions[0].average_buy_price.unwrap(), 10.05), "a venda não muda o preço médio");
    assert!(close(positions[0].realized_pl, 195.0));

    // 3) desdobramento 2:1            120 ações, custo 603 → médio 5,025
    add(&db, corporate_request(pf.id, petr.id, SPLIT, 2.0, "2026-03-10")).await;
    // 4) compra 30 @ 6                150 ações, custo 783 → médio 5,22
    add(&db, tx_request(pf.id, petr.id, "compra", 30.0, 6.0, "2026-04-10", 0.0)).await;
    let positions = compute_positions(&db, pf.id).await.unwrap();
    assert!(close(positions[0].quantity, 150.0));
    assert!(close(positions[0].average_buy_price.unwrap(), 5.22));

    // 5) vende tudo @ 7               recebe 1050, custo 783, ganho 267 → acumulado 462
    add(&db, tx_request(pf.id, petr.id, "venda", 150.0, 7.0, "2026-05-10", 0.0)).await;
    let positions = compute_positions(&db, pf.id).await.unwrap();
    assert!(close(positions[0].quantity, 0.0));
    assert_eq!(positions[0].average_buy_price, None, "posição encerrada não tem preço médio");
    assert!(close(positions[0].realized_pl, 462.0));

    // 6) recompra depois de zerar: o médio recomeça do zero
    add(&db, tx_request(pf.id, petr.id, "compra", 10.0, 8.0, "2026-06-01", 0.0)).await;
    let positions = compute_positions(&db, pf.id).await.unwrap();
    assert!(close(positions[0].quantity, 10.0));
    assert!(close(positions[0].average_buy_price.unwrap(), 8.0));
    assert!(close(positions[0].realized_pl, 462.0));

    // O motor de IR enxerga o mesmo histórico: a soma dos ganhos das vendas
    // é o lucro realizado que a posição mostra.
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    let tax_gain: f64 = assessment.months.iter().flat_map(|m| m.sales.iter()).map(|s| s.gain).sum();
    assert!(close(tax_gain, 462.0), "IR {tax_gain} × posição 462");
    assert!(assessment.warnings.is_empty());
}

#[tokio::test]
async fn a_bonus_and_a_reverse_split_adjust_quantity_and_cost() {
    use crate::commands::transaction::{compute_positions, REVERSE_SPLIT};

    let db = memory_db().await;
    let ws = workspace(&db, "Corporate").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let vale = asset(&db, "VALE3", "acao_br").await;
    let bbas = asset(&db, "BBAS3", "acao_br").await;

    add(&db, tx_request(pf.id, vale.id, "compra", 100.0, 10.0, "2026-01-05", 0.0)).await;
    // Bonificação: 10 ações novas atribuídas a R$ 2 cada (custo 20).
    let mut bonus = tx_request(pf.id, vale.id, "bonificacao", 10.0, 2.0, "2026-02-05", 0.0);
    bonus.total_value = 20.0;
    add(&db, bonus).await;
    // Grupamento 10:1 em outro ativo.
    add(&db, tx_request(pf.id, bbas.id, "compra", 100.0, 1.0, "2026-01-05", 0.0)).await;
    add(&db, corporate_request(pf.id, bbas.id, REVERSE_SPLIT, 10.0, "2026-02-05")).await;

    let positions = compute_positions(&db, pf.id).await.unwrap();
    let by_ticker = |t: &str| positions.iter().find(|p| p.ticker == t).unwrap();
    assert!(close(by_ticker("VALE3").quantity, 110.0));
    assert!(close(by_ticker("VALE3").average_buy_price.unwrap(), 1020.0 / 110.0));
    assert!(close(by_ticker("BBAS3").quantity, 10.0));
    assert!(close(by_ticker("BBAS3").average_buy_price.unwrap(), 10.0));
}

#[tokio::test]
async fn invalid_transactions_are_rejected_before_they_reach_the_ledger() {
    let db = memory_db().await;
    let ws = workspace(&db, "Validation").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let petr = asset(&db, "PETR4", "acao_br").await;
    add(&db, tx_request(pf.id, petr.id, "compra", 100.0, 10.0, "2026-01-05", 0.0)).await;

    let rejected = |request| async {
        crate::commands::transaction::create_transaction_in(&db, request).await.is_err()
    };

    // Data fora do formato, quantidade zero e total que não bate com quantidade × preço.
    assert!(rejected(tx_request(pf.id, petr.id, "compra", 1.0, 10.0, "05/01/2026", 0.0)).await);
    assert!(rejected(tx_request(pf.id, petr.id, "compra", 1.0, 10.0, "05-01-26", 0.0)).await, "ano de 2 dígitos");
    assert!(rejected(tx_request(pf.id, petr.id, "compra", 0.0, 10.0, "2026-01-06", 0.0)).await);
    let mut off = tx_request(pf.id, petr.id, "compra", 10.0, 10.0, "2026-01-06", 0.0);
    off.total_value = 150.0;
    assert!(rejected(off).await);
    // Venda acima do saldo.
    assert!(rejected(tx_request(pf.id, petr.id, "venda", 101.0, 12.0, "2026-02-01", 0.0)).await);
    // Venda retroativa à compra: nesse dia ainda não havia ações.
    assert!(rejected(tx_request(pf.id, petr.id, "venda", 10.0, 12.0, "2026-01-01", 0.0)).await);
    // A venda do saldo todo vale.
    assert!(!rejected(tx_request(pf.id, petr.id, "venda", 100.0, 12.0, "2026-02-01", 0.0)).await);
}

#[tokio::test]
async fn a_backdated_sale_cannot_leave_a_later_sale_uncovered() {
    let db = memory_db().await;
    let ws = workspace(&db, "Backdated").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let petr = asset(&db, "PETR4", "acao_br").await;
    add(&db, tx_request(pf.id, petr.id, "compra", 100.0, 10.0, "2026-01-05", 0.0)).await;
    add(&db, tx_request(pf.id, petr.id, "venda", 80.0, 12.0, "2026-03-01", 0.0)).await;
    // Hoje há 20 de saldo; vender 30 em fevereiro deixaria a venda de março sem cobertura.
    let result = crate::commands::transaction::create_transaction_in(
        &db,
        tx_request(pf.id, petr.id, "venda", 30.0, 11.0, "2026-02-01", 0.0),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn the_position_in_another_currency_realizes_at_the_rate_of_each_sale() {
    use crate::commands::currency::positions_in_base;

    let db = memory_db().await;
    let ws = workspace(&db, "Dollar").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let aapl = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    seed_price(&db, "USDBRL", "2026-01-05", 5.0).await;
    seed_price(&db, "USDBRL", "2026-02-10", 6.0).await;
    seed_price(&db, "USDBRL", "2026-10-16", 6.0).await;

    // Compra 10 @ US$ 10 (dólar a 5 → R$ 500) e vende 5 @ US$ 20 (dólar a 6 → recebe R$ 600, custo R$ 250).
    add(&db, tx_request(pf.id, aapl.id, "compra", 10.0, 10.0, "2026-01-05", 0.0)).await;
    add(&db, tx_request(pf.id, aapl.id, "venda", 5.0, 20.0, "2026-02-10", 0.0)).await;

    let (positions, base) = positions_in_base(&db, pf.id).await.unwrap();
    assert_eq!(base, "BRL");
    assert!(close(positions[0].quantity, 5.0));
    assert!(close(positions[0].average_buy_price.unwrap(), 50.0), "custo na taxa da compra: R$ 50 por ação");
    assert!(close(positions[0].realized_pl, 350.0));
}

// ---------------------------------------------------------------------------
// Importação e exportação de lançamentos (Fase 34)
// ---------------------------------------------------------------------------

const IMPORT_CSV: &str = "date,type,ticker,asset_class,currency,quantity,unit_price,total_value,fee,custody,payment_type,notes\n\
2026-01-05,compra,PETR4,,,100,30.5,3050,4.9,XP,,\n\
2026-02-10,venda,PETR4,,,40,35,1400,,XP,,\n\
2026-03-01,compra,HGLG11,fii,,10,160,1600,,XP,,\n";

async fn preview_csv(
    db: &sea_orm::DatabaseConnection,
    portfolio_id: i32,
    text: &str,
) -> Vec<crate::commands::portfolio_io::ImportRow> {
    use crate::commands::portfolio_io::{parse_anchor_csv, preview_rows};
    preview_rows(db, portfolio_id, parse_anchor_csv(text)).await.unwrap()
}

fn commit_items(
    rows: &[crate::commands::portfolio_io::ImportRow],
) -> Vec<crate::commands::portfolio_io::CommitItem> {
    rows.iter()
        .filter_map(|r| r.proposed.clone())
        .map(|proposed| crate::commands::portfolio_io::CommitItem { proposed, asset_class: None })
        .collect()
}

#[tokio::test]
async fn importing_a_csv_creates_assets_custody_and_the_right_position() {
    use crate::commands::portfolio_io::{commit_rows, ImportStatus};
    use crate::commands::transaction::compute_positions;

    let db = memory_db().await;
    let ws = workspace(&db, "Import").await;
    let pf = portfolio(&db, ws.id, "Main").await;

    let rows = preview_csv(&db, pf.id, IMPORT_CSV).await;
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| r.status == ImportStatus::New), "{:?}", rows.iter().map(|r| &r.message).collect::<Vec<_>>());
    assert_eq!(rows[0].suggested_class.as_deref(), Some("acao_br"), "classe inferida do ticker");

    let result = commit_rows(&db, pf.id, commit_items(&rows)).await.unwrap();
    assert_eq!(result.created, 3);
    assert!(result.errors.is_empty());

    // Uma custódia criada pelo nome, os dois ativos novos e a posição da PETR4:
    // 100 @ 30,50 + taxa 4,90 → médio 30,549; vende 40 @ 35.
    let custodias = crate::entity::custodia::Entity::find().all(&db).await.unwrap();
    assert_eq!(custodias.len(), 1);
    assert_eq!(custodias[0].instituicao, "XP");
    let positions = compute_positions(&db, pf.id).await.unwrap();
    let petr = positions.iter().find(|p| p.ticker == "PETR4").unwrap();
    assert!(close(petr.quantity, 60.0));
    assert!(close(petr.average_buy_price.unwrap(), 30.549));
    assert!(close(petr.realized_pl, 1400.0 - 40.0 * 30.549));
    assert_eq!(positions.iter().find(|p| p.ticker == "HGLG11").unwrap().asset_class, "fii");
}

#[tokio::test]
async fn importing_the_same_file_twice_adds_nothing() {
    use crate::commands::portfolio_io::{commit_rows, ImportStatus};

    let db = memory_db().await;
    let ws = workspace(&db, "Import twice").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let rows = preview_csv(&db, pf.id, IMPORT_CSV).await;
    commit_rows(&db, pf.id, commit_items(&rows)).await.unwrap();

    let again = preview_csv(&db, pf.id, IMPORT_CSV).await;
    assert!(again.iter().all(|r| r.status == ImportStatus::Duplicate));
    let result = commit_rows(&db, pf.id, commit_items(&again)).await.unwrap();
    assert_eq!(result.created, 0);
    assert_eq!(result.skipped_duplicates, 3);
    assert_eq!(crate::entity::transactions::Entity::find().all(&db).await.unwrap().len(), 3);
}

#[tokio::test]
async fn two_identical_trades_on_the_same_day_are_not_collapsed() {
    use crate::commands::portfolio_io::{commit_rows, ImportStatus};

    let db = memory_db().await;
    let ws = workspace(&db, "Same day").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let csv = "date,type,ticker,quantity,unit_price,total_value\n\
2026-01-05,compra,PETR4,10,30,300\n\
2026-01-05,compra,PETR4,10,30,300\n";

    let rows = preview_csv(&db, pf.id, csv).await;
    assert!(rows.iter().all(|r| r.status == ImportStatus::New));
    assert_eq!(commit_rows(&db, pf.id, commit_items(&rows)).await.unwrap().created, 2);

    // Reimportar o arquivo: os dois já existem, nenhum é novo.
    let again = preview_csv(&db, pf.id, csv).await;
    assert!(again.iter().all(|r| r.status == ImportStatus::Duplicate));
}

#[tokio::test]
async fn an_unknown_ambiguous_ticker_asks_for_the_class_and_a_bad_row_cancels_everything() {
    use crate::commands::portfolio_io::{commit_rows, CommitItem, ImportStatus};

    let db = memory_db().await;
    let ws = workspace(&db, "Import errors").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let csv = "date,type,ticker,quantity,unit_price,total_value\n\
2026-01-05,compra,BOVA11,10,100,1000\n\
data-ruim,compra,PETR4,10,30,300\n";

    let rows = preview_csv(&db, pf.id, csv).await;
    assert_eq!(rows[0].status, ImportStatus::NeedsClass);
    assert_eq!(rows[1].status, ImportStatus::Error);
    assert_eq!(rows[1].line, 2);

    // Sem classe escolhida, a gravação recusa e não deixa nada pela metade.
    let result = commit_rows(&db, pf.id, commit_items(&rows)).await.unwrap();
    assert_eq!(result.created, 0);
    assert_eq!(result.errors.len(), 1);
    assert!(crate::entity::assets::Entity::find().all(&db).await.unwrap().is_empty());
    assert!(crate::entity::transactions::Entity::find().all(&db).await.unwrap().is_empty());

    // Com a classe escolhida na prévia, grava.
    let item = CommitItem { proposed: rows[0].proposed.clone().unwrap(), asset_class: Some("etf_br".to_string()) };
    let ok = commit_rows(&db, pf.id, vec![item]).await.unwrap();
    assert_eq!(ok.created, 1);
    assert_eq!(crate::entity::assets::Entity::find().all(&db).await.unwrap()[0].asset_class, "etf_br");
}

#[tokio::test]
async fn a_sale_beyond_the_balance_in_the_file_rolls_the_whole_import_back() {
    use crate::commands::portfolio_io::commit_rows;

    let db = memory_db().await;
    let ws = workspace(&db, "Import oversell").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let csv = "date,type,ticker,quantity,unit_price,total_value\n\
2026-01-05,compra,PETR4,10,30,300\n\
2026-02-05,venda,PETR4,20,35,700\n";
    let rows = preview_csv(&db, pf.id, csv).await;
    let result = commit_rows(&db, pf.id, commit_items(&rows)).await.unwrap();
    assert_eq!(result.created, 0);
    assert_eq!(result.errors[0].line, 2);
    assert!(crate::entity::transactions::Entity::find().all(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn exporting_and_importing_into_another_portfolio_reproduces_the_position() {
    use crate::commands::portfolio_io::{
        commit_rows, export_transactions_csv_text, parse_anchor_csv, preview_rows, proposed_from_portfolio,
    };
    use crate::commands::transaction::compute_positions;

    let db = memory_db().await;
    let ws = workspace(&db, "Round trip").await;
    let source = portfolio(&db, ws.id, "Source").await;
    let target = portfolio(&db, ws.id, "Target").await;
    let rows = preview_csv(&db, source.id, IMPORT_CSV).await;
    commit_rows(&db, source.id, commit_items(&rows)).await.unwrap();

    let exported = export_transactions_csv_text(&proposed_from_portfolio(&db, source.id).await.unwrap()).unwrap();
    let preview = preview_rows(&db, target.id, parse_anchor_csv(&exported)).await.unwrap();
    commit_rows(&db, target.id, commit_items(&preview)).await.unwrap();

    let key = |p: &crate::commands::transaction::PositionView| (p.ticker.clone(), p.quantity, p.realized_pl);
    let mut a: Vec<_> = compute_positions(&db, source.id).await.unwrap().iter().map(key).collect();
    let mut b: Vec<_> = compute_positions(&db, target.id).await.unwrap().iter().map(key).collect();
    a.sort_by(|x, y| x.0.cmp(&y.0));
    b.sort_by(|x, y| x.0.cmp(&y.0));
    assert_eq!(a.len(), 2);
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.0, y.0);
        assert!(close(x.1, y.1) && close(x.2, y.2), "{x:?} × {y:?}");
    }
}

const FUND_CSV: &str = "date,type,ticker,asset_class,currency,quantity,unit_price,total_value,fee,custody,payment_type,notes,loss_credit_used,fund_term,fund_structure,fund_estoque_option\n\
2025-01-02,compra,FUNDO1,fundo,,100,10,1000,,,,,,longo_prazo,fechado,opcao_8\n\
2025-05-30,come_cotas,FUNDO1,,,2.5,12,30,,,,,5,longo_prazo,fechado,opcao_8\n\
2025-08-01,amortizacao,FUNDO1,,,,15,600,,,,,,longo_prazo,fechado,opcao_8\n";

async fn fund_detail_of_ticker(db: &sea_orm::DatabaseConnection, ticker: &str) -> Option<crate::entity::fund_detail::Model> {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let asset = crate::entity::assets::Entity::find()
        .filter(crate::entity::assets::Column::Ticker.eq(ticker))
        .one(db)
        .await
        .unwrap()?;
    crate::entity::fund_detail::Entity::find()
        .filter(crate::entity::fund_detail::Column::AssetId.eq(asset.id))
        .one(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_fund_csv_round_trips_credit_and_fund_details_into_a_fresh_database() {
    use crate::commands::portfolio_io::{
        commit_rows, export_transactions_csv_text, parse_anchor_csv, preview_rows, proposed_from_portfolio,
    };
    use sea_orm::EntityTrait;

    let db = memory_db().await;
    let ws = workspace(&db, "Fundos").await;
    let source = portfolio(&db, ws.id, "Source").await;
    let rows = preview_csv(&db, source.id, FUND_CSV).await;
    let result = commit_rows(&db, source.id, commit_items(&rows)).await.unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let detail = fund_detail_of_ticker(&db, "FUNDO1").await.unwrap();
    assert_eq!((detail.term.as_str(), detail.structure.as_str()), ("longo_prazo", "fechado"));
    assert_eq!(detail.estoque_option.as_deref(), Some("opcao_8"));

    let exported = export_transactions_csv_text(&proposed_from_portfolio(&db, source.id).await.unwrap()).unwrap();
    assert!(exported.starts_with("date,type,ticker,asset_class,currency,quantity,unit_price,total_value,fee,custody,payment_type,notes,loss_credit_used,fund_term,fund_structure,fund_estoque_option\n"));

    let other = memory_db().await;
    let other_ws = workspace(&other, "Destino").await;
    let target = portfolio(&other, other_ws.id, "Target").await;
    let preview = preview_rows(&other, target.id, parse_anchor_csv(&exported)).await.unwrap();
    let result = commit_rows(&other, target.id, commit_items(&preview)).await.unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    let detail = fund_detail_of_ticker(&other, "FUNDO1").await.unwrap();
    assert_eq!((detail.term.as_str(), detail.structure.as_str()), ("longo_prazo", "fechado"));
    assert_eq!(detail.estoque_option.as_deref(), Some("opcao_8"));
    let txs = crate::entity::transactions::Entity::find().all(&other).await.unwrap();
    let credit: Vec<_> = txs.iter().filter(|t| t.transaction_type == "come_cotas").map(|t| t.loss_credit_used).collect();
    assert_eq!(credit, vec![Some(5.0)]);
    assert_eq!(txs.len(), 3);
}

#[tokio::test]
async fn importing_fund_columns_keeps_the_fund_details_the_user_already_set() {
    use crate::commands::portfolio_io::commit_rows;

    let db = memory_db().await;
    let ws = workspace(&db, "Fundos").await;
    let first = portfolio(&db, ws.id, "A").await;
    let seed = "date,type,ticker,asset_class,total_value,quantity,unit_price,fund_term,fund_structure\n\
                2025-01-02,compra,FUNDO1,fundo,1000,100,10,curto_prazo,aberto\n";
    let rows = preview_csv(&db, first.id, seed).await;
    commit_rows(&db, first.id, commit_items(&rows)).await.unwrap();

    let second = portfolio(&db, ws.id, "B").await;
    let rows = preview_csv(&db, second.id, FUND_CSV).await;
    commit_rows(&db, second.id, commit_items(&rows)).await.unwrap();
    let detail = fund_detail_of_ticker(&db, "FUNDO1").await.unwrap();
    assert_eq!((detail.term.as_str(), detail.structure.as_str()), ("curto_prazo", "aberto"));
    assert_eq!(detail.estoque_option, None);
}

#[tokio::test]
async fn bad_fund_columns_fail_the_import_with_the_line() {
    use crate::commands::portfolio_io::commit_rows;

    let db = memory_db().await;
    let ws = workspace(&db, "Fundos").await;
    let p = portfolio(&db, ws.id, "A").await;
    let bad = "date,type,ticker,asset_class,total_value,quantity,unit_price,fund_term,fund_structure\n\
               2025-01-02,compra,FUNDO1,fundo,1000,100,10,longo_prazo,semiaberto\n";
    let rows = preview_csv(&db, p.id, bad).await;
    let result = commit_rows(&db, p.id, commit_items(&rows)).await.unwrap();
    assert_eq!(result.created, 0);
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].message.contains("fund_structure"));
    assert!(fund_detail_of_ticker(&db, "FUNDO1").await.is_none());
}

// ---------------------------------------------------------------------------
// Arquivo portátil de um Workspace (Fase 34)
// ---------------------------------------------------------------------------

/// Um Workspace com carteira (split, venda, ativo em dólar), contas, transferência
/// entre moedas, empréstimo e moeda base própria.
async fn rich_workspace(db: &sea_orm::DatabaseConnection, name: &str) -> i32 {
    use crate::commands::transaction::SPLIT;

    let ws = workspace(db, name).await;
    save_base_currency(db, ws.id, "USD").await.unwrap();
    let pf = portfolio(db, ws.id, "Main").await;
    let petr = asset(db, "PETR4", "acao_br").await;
    let aapl = asset_in(db, "AAPL", "acao_internacional", "USD").await;
    add(db, tx_request(pf.id, petr.id, "compra", 100.0, 10.0, "2026-01-05", 5.0)).await;
    add(db, corporate_request(pf.id, petr.id, SPLIT, 2.0, "2026-02-05")).await;
    add(db, tx_request(pf.id, petr.id, "venda", 50.0, 6.0, "2026-03-05", 0.0)).await;
    add(db, tx_request(pf.id, aapl.id, "compra", 10.0, 100.0, "2026-01-06", 0.0)).await;

    let brl = bank_account(db, ws.id, "Nubank").await;
    let usd = bank_account_in(db, ws.id, "Chase", "USD").await;
    cash_in(db, ws.id, brl.id, 1000.0, "2026-01-01").await;
    create_transfer_in(db, transfer_request(ws.id, brl.id, usd.id, 500.0, 95.0)).await.unwrap();

    let borrower = create_borrower_in(
        db,
        CreateBorrowerRequest { workspace_id: ws.id, nome: "Maria".to_string(), notes: None },
    )
    .await
    .unwrap()
    .id;
    create_loan_in(db, loan_request(ws.id, borrower, Some(brl.id), 200.0)).await.unwrap();
    ws.id
}

/// Fotografia comparável de um Workspace: tudo que o usuário enxerga, sem ids.
async fn workspace_snapshot(db: &sea_orm::DatabaseConnection, workspace_id: i32) -> String {
    use crate::commands::transaction::compute_positions;

    let portfolios = crate::entity::portfolio::Entity::find()
        .filter(crate::entity::portfolio::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await
        .unwrap();
    let mut lines: Vec<String> = Vec::new();
    for pf in &portfolios {
        for p in compute_positions(db, pf.id).await.unwrap() {
            lines.push(format!(
                "pos {} {} {:.4} {:.4} {:.4}",
                pf.name,
                p.ticker,
                p.quantity,
                p.average_buy_price.unwrap_or(0.0),
                p.realized_pl
            ));
        }
    }
    for a in list_bank_accounts_view(db, workspace_id).await.unwrap() {
        lines.push(format!("account {} {} {:.2}", a.nome, a.currency, a.balance));
    }
    for l in list_loans_view(db, workspace_id).await.unwrap() {
        lines.push(format!("loan {} {:.2}", l.currency, l.saldo_a_receber));
    }
    lines.push(format!("base {}", base_currency(db, workspace_id).await.unwrap()));
    lines.sort();
    lines.join("\n")
}

#[tokio::test]
async fn a_workspace_exported_and_imported_into_the_same_database_becomes_an_equal_copy() {
    use crate::workspace_file::{export_workspace, import_workspace};

    let db = memory_db().await;
    let original = rich_workspace(&db, "Family").await;
    let assets_before = crate::entity::assets::Entity::find().all(&db).await.unwrap().len();

    let file = export_workspace(&db, original).await.unwrap();
    let json = serde_json::to_string(&file).unwrap();
    let summary = import_workspace(&db, serde_json::from_str(&json).unwrap()).await.unwrap();

    assert_eq!(summary.workspace_name, "Family (imported)");
    assert_ne!(summary.workspace_id as i32, original);
    assert_eq!(
        workspace_snapshot(&db, summary.workspace_id as i32).await,
        workspace_snapshot(&db, original).await
    );
    // Os ativos foram casados por ticker e classe, não duplicados.
    assert_eq!(crate::entity::assets::Entity::find().all(&db).await.unwrap().len(), assets_before);
    // A cópia não mexeu no original.
    assert!(workspace_snapshot(&db, original).await.contains("pos Main PETR4 150.0000"));

    // As duas pernas da transferência copiada apontam uma pra outra, não pro original.
    let copied: Vec<_> = crate::entity::general_transaction::Entity::find()
        .filter(crate::entity::general_transaction::Column::WorkspaceId.eq(summary.workspace_id as i32))
        .filter(crate::entity::general_transaction::Column::TransferId.is_not_null())
        .all(&db)
        .await
        .unwrap();
    assert_eq!(copied.len(), 2);
    let ids: std::collections::BTreeSet<i32> = copied.iter().map(|r| r.id).collect();
    assert!(copied.iter().all(|r| ids.contains(&r.transfer_id.unwrap())));
}

#[tokio::test]
async fn a_workspace_file_loads_into_a_fresh_database_with_its_assets() {
    use crate::workspace_file::{export_workspace, import_workspace};

    let source = memory_db().await;
    let original = rich_workspace(&source, "Family").await;
    let file = export_workspace(&source, original).await.unwrap();
    let json = serde_json::to_string(&file).unwrap();

    let destination = memory_db().await;
    workspace(&destination, "Family").await; // já existe um com esse nome
    let before = crate::entity::workspace::Entity::find().all(&destination).await.unwrap().len();
    let summary = import_workspace(&destination, serde_json::from_str(&json).unwrap()).await.unwrap();

    assert_eq!(summary.workspace_name, "Family (imported)");
    assert_eq!(
        workspace_snapshot(&destination, summary.workspace_id as i32).await,
        workspace_snapshot(&source, original).await
    );
    assert_eq!(crate::entity::workspace::Entity::find().all(&destination).await.unwrap().len(), before + 1);
    let tickers: std::collections::BTreeSet<String> = crate::entity::assets::Entity::find()
        .all(&destination)
        .await
        .unwrap()
        .into_iter()
        .map(|a| a.ticker)
        .collect();
    assert_eq!(tickers, ["AAPL".to_string(), "PETR4".to_string()].into_iter().collect());
}

#[tokio::test]
async fn a_workspace_file_never_leaks_another_workspace() {
    use crate::workspace_file::export_workspace;

    let db = memory_db().await;
    let mine = rich_workspace(&db, "Mine").await;
    let other = workspace(&db, "Other").await;
    bank_account(&db, other.id, "Secret bank").await;

    let file = export_workspace(&db, mine).await.unwrap();
    let json = serde_json::to_string(&file).unwrap();
    assert!(!json.contains("Secret bank"));
    assert!(!json.contains("Other"));
}

#[tokio::test]
async fn a_file_that_is_not_a_workspace_file_is_rejected() {
    use crate::workspace_file::{import_workspace, WorkspaceFile};

    let db = memory_db().await;
    let bogus: WorkspaceFile = serde_json::from_str(
        r#"{"format":"outra-coisa","version":1,"exported_at":"","schema_version":"","workspace":{},"tables":{},"assets":[],"asset_tables":{}}"#,
    )
    .unwrap();
    let before = crate::entity::workspace::Entity::find().all(&db).await.unwrap().len();
    assert!(import_workspace(&db, bogus).await.is_err());
    assert_eq!(crate::entity::workspace::Entity::find().all(&db).await.unwrap().len(), before);
}

// ---------------------------------------------------------------------------
// Fila de pendentes da importação (Fase 35)
// ---------------------------------------------------------------------------

async fn stage_csv(
    db: &sea_orm::DatabaseConnection,
    portfolio_id: i32,
    text: &str,
) -> crate::commands::portfolio_io::StageResult {
    use crate::commands::portfolio_io::{parse_anchor_csv, stage_parsed};
    stage_parsed(db, portfolio_id, "anchor_csv", parse_anchor_csv(text)).await.unwrap()
}

#[tokio::test]
async fn staging_keeps_new_rows_and_reports_unreadable_ones_apart() {
    use crate::commands::portfolio_io::list_import_pending_in;

    let db = memory_db().await;
    let ws = workspace(&db, "Stage").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let csv = format!("{IMPORT_CSV}05-01-26,compra,VALE3,,,1,1,1,,,,\n");

    let staged = stage_csv(&db, pf.id, &csv).await;
    assert_eq!((staged.staged, staged.duplicates, staged.errors.len()), (3, 0, 1));
    assert_eq!(staged.errors[0].line, 4);

    let pending = list_import_pending_in(&db, pf.id).await.unwrap();
    assert_eq!(pending.len(), 3);
    assert_eq!(pending[0].proposed.ticker, "PETR4", "em ordem de data");
    assert!(pending.iter().all(|p| p.needs_class), "nenhum ativo existe ainda");
    assert!(crate::entity::transactions::Entity::find().all(&db).await.unwrap().is_empty(), "fila não grava no ledger");
}

#[tokio::test]
async fn confirming_a_pending_item_writes_it_and_leaves_the_queue() {
    use crate::commands::portfolio_io::{confirm_import_pending_in, list_import_pending_in};
    use crate::commands::transaction::compute_positions;

    let db = memory_db().await;
    let ws = workspace(&db, "Confirm").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    stage_csv(&db, pf.id, IMPORT_CSV).await;

    let pending = list_import_pending_in(&db, pf.id).await.unwrap();
    let first = &pending[0];
    // Edita a quantidade antes de aprovar: o que vale é o editado.
    let mut edited = first.proposed.clone();
    edited.quantity = Some(50.0);
    edited.total_value = 1525.0;
    let result = confirm_import_pending_in(&db, pf.id, first.id, edited, first.asset_class.clone()).await.unwrap();
    assert_eq!((result.created, result.errors.len()), (1, 0));

    assert_eq!(list_import_pending_in(&db, pf.id).await.unwrap().len(), 2);
    let positions = compute_positions(&db, pf.id).await.unwrap();
    assert!(close(positions.iter().find(|p| p.ticker == "PETR4").unwrap().quantity, 50.0));

    // A edição mudou quantidade e total: reimportar o arquivo original não o traz de volta.
    let again = stage_csv(&db, pf.id, IMPORT_CSV).await;
    assert_eq!((again.staged, again.duplicates), (0, 3));
}

#[tokio::test]
async fn a_pending_sale_beyond_the_balance_stays_pending() {
    use crate::commands::portfolio_io::{confirm_import_pending_in, list_import_pending_in};

    let db = memory_db().await;
    let ws = workspace(&db, "Oversell pending").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    stage_csv(
        &db,
        pf.id,
        "date,type,ticker,quantity,unit_price,total_value\n2026-01-05,compra,PETR4,10,30,300\n2026-02-05,venda,PETR4,5,35,175\n",
    )
    .await;

    // Aprovar a venda antes da compra: recusada, o item continua na fila.
    let pending = list_import_pending_in(&db, pf.id).await.unwrap();
    let sale = pending.iter().find(|p| p.proposed.transaction_type == "venda").unwrap();
    let result =
        confirm_import_pending_in(&db, pf.id, sale.id, sale.proposed.clone(), Some("acao_br".to_string())).await.unwrap();
    assert_eq!(result.created, 0);
    assert!(!result.errors.is_empty());
    assert_eq!(list_import_pending_in(&db, pf.id).await.unwrap().len(), 2);
    assert!(crate::entity::transactions::Entity::find().all(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_discarded_item_is_gone_from_the_queue_and_never_comes_back() {
    use crate::commands::portfolio_io::{discard_import_pending_in, list_import_pending_in};

    let db = memory_db().await;
    let ws = workspace(&db, "Discard").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    stage_csv(&db, pf.id, IMPORT_CSV).await;
    let pending = list_import_pending_in(&db, pf.id).await.unwrap();
    discard_import_pending_in(&db, pf.id, pending[0].id).await.unwrap();
    assert_eq!(list_import_pending_in(&db, pf.id).await.unwrap().len(), 2);

    // Reimportar o mesmo arquivo: os 2 que ainda esperam e o descartado contam como duplicatas.
    let again = stage_csv(&db, pf.id, IMPORT_CSV).await;
    assert_eq!((again.staged, again.duplicates), (0, 3));
    assert_eq!(list_import_pending_in(&db, pf.id).await.unwrap().len(), 2);
    // Descartar duas vezes é recusado.
    assert!(discard_import_pending_in(&db, pf.id, pending[0].id).await.is_err());
}

#[tokio::test]
async fn an_approved_item_is_a_duplicate_on_reimport_through_the_ledger() {
    use crate::commands::portfolio_io::{confirm_import_pending_in, list_import_pending_in};

    let db = memory_db().await;
    let ws = workspace(&db, "Approved reimport").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    stage_csv(&db, pf.id, IMPORT_CSV).await;
    for item in list_import_pending_in(&db, pf.id).await.unwrap() {
        confirm_import_pending_in(&db, pf.id, item.id, item.proposed.clone(), item.asset_class.clone()).await.unwrap();
    }
    assert!(list_import_pending_in(&db, pf.id).await.unwrap().is_empty());

    let again = stage_csv(&db, pf.id, IMPORT_CSV).await;
    assert_eq!((again.staged, again.duplicates), (0, 3));
}

#[tokio::test]
async fn a_movement_sheet_stages_the_dividend_and_confirming_it_records_a_provento() {
    use crate::commands::portfolio_io::{
        confirm_import_pending_in, list_import_pending_in, parse_b3_movimentacao_rows, stage_parsed,
    };
    use crate::commands::proventos::proventos_summary_in;

    let db = memory_db().await;
    let ws = workspace(&db, "Movement").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let sheet: Vec<Vec<String>> = [
        ["Entrada/Saída", "Data", "Movimentação", "Produto", "Instituição", "Quantidade", "Preço unitário", "Valor da Operação"],
        ["Credito", "30/09/2026", "Dividendo", "ABCD3 - EMPRESA A S.A.", "CORRETORA", "100", "0.05", "5"],
        ["Credito", "22/09/2026", "Transferência - Liquidação", "ABCD3 - EMPRESA A S.A.", "CORRETORA", "100", "9", "900"],
    ]
    .iter()
    .map(|r| r.iter().map(|c| c.to_string()).collect())
    .collect();

    let rules = crate::commands::tax::load_rule_set(&db).await.unwrap();
    let staged = stage_parsed(&db, pf.id, "b3_movimentacao", parse_b3_movimentacao_rows(&sheet, &rules)).await.unwrap();
    assert_eq!((staged.staged, staged.duplicates, staged.errors.len()), (1, 0, 1));
    assert!(staged.errors[0].message.contains("liquidação"));

    let pending = list_import_pending_in(&db, pf.id).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending[0].needs_class, "o ativo ainda não existe");
    let result = confirm_import_pending_in(
        &db,
        pf.id,
        pending[0].id,
        pending[0].proposed.clone(),
        Some("acao_br".to_string()),
    )
    .await
    .unwrap();
    assert_eq!((result.created, result.errors.len()), (1, 0));

    let txs = crate::entity::transactions::Entity::find().all(&db).await.unwrap();
    assert_eq!(txs.len(), 1);
    assert_eq!(txs[0].transaction_type, "provento");
    assert_eq!(txs[0].payment_type.as_deref(), Some("dividendo"));
    assert!(close(txs[0].total_value, 5.0));
    let summary = proventos_summary_in(&db, pf.id).await.unwrap();
    assert!(close(summary.total_all_time, 5.0), "o resumo de proventos conta o lançamento importado");
    assert_eq!(summary.rows.len(), 1);

    // Reimportar a mesma planilha não traz o provento de volta.
    let again = stage_parsed(&db, pf.id, "b3_movimentacao", parse_b3_movimentacao_rows(&sheet, &rules)).await.unwrap();
    assert_eq!((again.staged, again.duplicates), (0, 1));
}

#[tokio::test]
async fn a_jcp_from_the_movement_sheet_is_booked_gross_and_reads_back_as_the_net_the_b3_paid() {
    use crate::commands::portfolio_io::{
        confirm_import_pending_in, list_import_pending_in, parse_b3_movimentacao_rows, stage_parsed,
    };
    use crate::commands::proventos::proventos_summary_in;

    let db = memory_db().await;
    let ws = workspace(&db, "JCP").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let header = ["Entrada/Saída", "Data", "Movimentação", "Produto", "Instituição", "Quantidade", "Preço unitário", "Valor da Operação"];
    fn jcp<'a>(date: &'a str, net: &'a str) -> [&'a str; 8] {
        ["Credito", date, "Juros Sobre Capital Próprio", "ABCD3 - EMPRESA A S.A.", "CORRETORA", "100", "0.1", net]
    }
    // A B3 informa o líquido: 8,50 em dez/2025 (IRRF de 15% → bruto 10,00) e 8,25 em jan/2026 (17,5% → bruto 10,00).
    let sheet: Vec<Vec<String>> = [header, jcp("15/12/2025", "8.50"), jcp("10/01/2026", "8.25")]
        .iter()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect();

    let rules = crate::commands::tax::load_rule_set(&db).await.unwrap();
    let staged = stage_parsed(&db, pf.id, "b3_movimentacao", parse_b3_movimentacao_rows(&sheet, &rules)).await.unwrap();
    assert_eq!((staged.staged, staged.errors.len()), (2, 0));
    for item in list_import_pending_in(&db, pf.id).await.unwrap() {
        assert_eq!(item.proposed.payment_type.as_deref(), Some("jscp"));
        confirm_import_pending_in(&db, pf.id, item.id, item.proposed.clone(), item.asset_class.clone()).await.unwrap();
    }

    let stored = crate::entity::transactions::Entity::find().all(&db).await.unwrap();
    assert!(stored.iter().all(|t| close(t.total_value, 10.0)), "o bruto é o que fica gravado");
    let summary = proventos_summary_in(&db, pf.id).await.unwrap();
    assert!(close(summary.total_all_time, 16.75), "o app desconta o IRRF da data e devolve o líquido da B3 (8,50 + 8,25)");
}

#[test]
fn the_tax_hint_follows_the_class_chosen_for_an_eleven_ticker() {
    use crate::commands::portfolio_io::class_tax_hint_text;
    assert_ne!(class_tax_hint_text("fii", "TAEE11"), class_tax_hint_text("acao_br", "TAEE11"));
    assert!(class_tax_hint_text("acao_br", "TAEE11").contains("never exempt"));
    assert!(class_tax_hint_text("etf_br", "WXYZ11").contains("never exempt"));
}

// ---------------------------------------------------------------------------
// Extrato bancário pela fila (Fase 35, entrega 3). Dados inventados.
// ---------------------------------------------------------------------------

const STATEMENT_CSV: &str = "Extrato Conta Corrente\nConta ;111\n\nData Lançamento;Histórico;Descrição;Valor;Saldo\n\
15/01/2025;Pix recebido;Pessoa A;1.000,00;0,00\n\
16/01/2025;Pix enviado;Loja B;-150,25;0,00\n\
16/01/2025;Aplicação;Cdb Porquinho Banco X;-300,00;0,00\n\
20/01/2025;Resgate;Cdb Porquinho Banco X;50,00;0,00\n";

const STATEMENT_OFX: &str = "<OFX><BANKTRANLIST>\n\
<STMTTRN>\n<DTPOSTED>20250115\n<TRNAMT>1000.00\n<FITID>1\n<NAME>Pessoa A\n<MEMO>Pix recebido: \"x\"\n</STMTTRN>\n\
<STMTTRN>\n<DTPOSTED>20250116\n<TRNAMT>-150.25\n<FITID>2\n<NAME>Loja B\n<MEMO>Pix enviado: \"x\"\n</STMTTRN>\n\
<STMTTRN>\n<DTPOSTED>20250116\n<TRNAMT>-300.00\n<FITID>3\n<NAME>Cdb Porquinho Banco X\n<MEMO>Aplicacao: \"x\"\n</STMTTRN>\n\
<STMTTRN>\n<DTPOSTED>20250120\n<TRNAMT>50.00\n<FITID>4\n<NAME>Cdb Porquinho Banco X\n<MEMO>Resgate: \"x\"\n</STMTTRN>\n\
</BANKTRANLIST></OFX>\n";

fn edited_from(item: &crate::commands::statement_io::StatementPendingView) -> crate::commands::statement_io::EditedBankItem {
    crate::commands::statement_io::EditedBankItem {
        transaction_date: item.proposed.transaction_date.clone(),
        transaction_type: item.suggested_type.clone(),
        valor: item.proposed.valor.abs(),
        notes: Some(item.proposed.description.clone()),
        category_id: None,
    }
}

async fn confirm_all_statement_items(db: &sea_orm::DatabaseConnection, workspace_id: i32) {
    use crate::commands::statement_io::{confirm_statement_pending_in, list_statement_pending_in};
    for item in list_statement_pending_in(db, workspace_id).await.unwrap() {
        confirm_statement_pending_in(db, workspace_id, item.id, edited_from(&item)).await.unwrap();
    }
}

#[tokio::test]
async fn a_statement_moves_balances_and_the_porquinho_legs_keep_the_total_cash() {
    use crate::commands::statement_io::{list_statement_pending_in, parse_inter_csv, stage_statement_parsed};

    let db = memory_db().await;
    let ws = workspace(&db, "Statement").await;
    let checking = crate::test_support::bank_account(&db, ws.id, "Checking").await;

    let staged = stage_statement_parsed(&db, ws.id, checking.id, None, true, "inter_csv", parse_inter_csv(STATEMENT_CSV))
        .await
        .unwrap();
    assert_eq!((staged.staged, staged.duplicates, staged.skipped.len()), (4, 0, 0));
    assert_eq!(staged.account_ref.as_deref(), Some("111"));
    let accounts = list_bank_accounts_view(&db, ws.id).await.unwrap();
    assert_eq!(accounts.len(), 2, "a conta Porquinho foi criada");
    assert!(accounts.iter().any(|a| a.nome == "Cdb Porquinho"));

    let pending = list_statement_pending_in(&db, ws.id).await.unwrap();
    assert_eq!(
        pending.iter().map(|p| p.suggested_type.as_str()).collect::<Vec<_>>(),
        vec!["receita", "despesa", "transferencia", "transferencia"]
    );
    assert!(list_bank_accounts_view(&db, ws.id).await.unwrap().iter().all(|a| close(a.balance, 0.0)), "fila não move saldo");

    confirm_all_statement_items(&db, ws.id).await;
    assert!(list_statement_pending_in(&db, ws.id).await.unwrap().is_empty());
    let accounts = list_bank_accounts_view(&db, ws.id).await.unwrap();
    let balance = |name: &str| accounts.iter().find(|a| a.nome == name).unwrap().balance;
    // Conta: +1000 −150,25 −300 (aplicação) +50 (resgate). Porquinho: +300 −50.
    assert!(close(balance("Checking"), 599.75));
    assert!(close(balance("Cdb Porquinho"), 250.0));
    assert!(close(accounts.iter().map(|a| a.balance).sum::<f64>(), 849.75), "transferir não muda o caixa total");
}

#[tokio::test]
async fn the_ofx_and_the_csv_of_the_same_statement_do_not_duplicate_each_other() {
    use crate::commands::statement_io::{parse_inter_csv, parse_ofx, stage_statement_parsed};

    let db = memory_db().await;
    let ws = workspace(&db, "Both formats").await;
    let checking = crate::test_support::bank_account(&db, ws.id, "Checking").await;

    let first = stage_statement_parsed(&db, ws.id, checking.id, None, true, "inter_ofx", parse_ofx(STATEMENT_OFX)).await.unwrap();
    assert_eq!(first.staged, 4);
    confirm_all_statement_items(&db, ws.id).await;
    let pocket = list_bank_accounts_view(&db, ws.id).await.unwrap().into_iter().find(|a| a.nome == "Cdb Porquinho").unwrap();

    let second =
        stage_statement_parsed(&db, ws.id, checking.id, Some(pocket.id), false, "inter_csv", parse_inter_csv(STATEMENT_CSV))
            .await
            .unwrap();
    assert_eq!((second.staged, second.duplicates), (0, 4), "o CSV do mesmo período não traz nada novo");
    assert_eq!(crate::entity::general_transaction::Entity::find().all(&db).await.unwrap().len(), 6, "2 comuns + 2 pares");
}

#[tokio::test]
async fn identical_entries_on_the_same_day_count_as_many_as_there_are() {
    use crate::commands::statement_io::{
        confirm_statement_pending_in, list_statement_pending_in, parse_inter_csv, stage_statement_parsed,
    };

    let db = memory_db().await;
    let ws = workspace(&db, "Twins").await;
    let checking = crate::test_support::bank_account(&db, ws.id, "Checking").await;
    let csv = "Data Lançamento;Histórico;Descrição;Valor;Saldo\n\
10/02/2025;Pix enviado;Loja B;-10,00;0,00\n10/02/2025;Pix enviado;Loja C;-10,00;0,00\n";

    assert_eq!(stage_statement_parsed(&db, ws.id, checking.id, None, false, "inter_csv", parse_inter_csv(csv)).await.unwrap().staged, 2);
    let pending = list_statement_pending_in(&db, ws.id).await.unwrap();
    confirm_statement_pending_in(&db, ws.id, pending[0].id, edited_from(&pending[0])).await.unwrap();

    // Um já está no ledger e o outro continua na fila: reimportar não cria nenhum.
    let again = stage_statement_parsed(&db, ws.id, checking.id, None, false, "inter_csv", parse_inter_csv(csv)).await.unwrap();
    assert_eq!((again.staged, again.duplicates), (0, 2));
}

#[tokio::test]
async fn editing_discarding_and_refusing_work_on_the_statement_queue() {
    use crate::commands::statement_io::{
        confirm_statement_pending_in, discard_statement_pending_in, list_statement_pending_in, parse_inter_csv,
        stage_statement_parsed,
    };

    let db = memory_db().await;
    let ws = workspace(&db, "Edit statement").await;
    let checking = crate::test_support::bank_account(&db, ws.id, "Checking").await;
    let category = {
        use sea_orm::{ActiveModelTrait, Set};
        crate::entity::general_transaction_category::ActiveModel {
            workspace_id: Set(ws.id),
            nome: Set("Mercado".to_string()),
            limite_mensal: Set(None),
            created_at: Set("2026-01-01T00:00:00Z".to_string()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap()
    };

    // Sem conta Porquinho, aplicação e resgate ficam de fora com o motivo.
    let staged = stage_statement_parsed(&db, ws.id, checking.id, None, false, "inter_csv", parse_inter_csv(STATEMENT_CSV)).await.unwrap();
    assert_eq!((staged.staged, staged.skipped.len()), (2, 2));
    assert!(staged.skipped[0].message.contains("porquinho"));

    let pending = list_statement_pending_in(&db, ws.id).await.unwrap();
    // Valor inválido é recusado e o item continua pendente.
    let mut bad = edited_from(&pending[0]);
    bad.valor = 0.0;
    assert!(confirm_statement_pending_in(&db, ws.id, pending[0].id, bad).await.is_err());
    assert_eq!(list_statement_pending_in(&db, ws.id).await.unwrap().len(), 2);

    // A despesa é editada: outro valor, categoria e nota.
    let mut edited = edited_from(&pending[1]);
    edited.valor = 99.0;
    edited.category_id = Some(category.id);
    edited.notes = Some("compra do mês".to_string());
    confirm_statement_pending_in(&db, ws.id, pending[1].id, edited).await.unwrap();
    let saved = crate::entity::general_transaction::Entity::find().all(&db).await.unwrap();
    assert_eq!((saved.len(), saved[0].transaction_type.as_str()), (1, "despesa"));
    assert!(close(saved[0].valor, 99.0) && saved[0].category_id == Some(category.id));
    assert_eq!(saved[0].notes.as_deref(), Some("compra do mês"));

    // Editar o valor não faz o original voltar numa reimportação (a marca `confirmed` fica na fila).
    let still_pending = stage_statement_parsed(&db, ws.id, checking.id, None, false, "inter_csv", parse_inter_csv(STATEMENT_CSV))
        .await
        .unwrap();
    assert_eq!((still_pending.staged, still_pending.duplicates), (0, 2));

    // Descartar tira da fila, e reimportar não traz de volta.
    discard_statement_pending_in(&db, ws.id, pending[0].id).await.unwrap();
    assert!(list_statement_pending_in(&db, ws.id).await.unwrap().is_empty());
    assert!(discard_statement_pending_in(&db, ws.id, pending[0].id).await.is_err(), "já descartado");
    let again = stage_statement_parsed(&db, ws.id, checking.id, None, false, "inter_csv", parse_inter_csv(STATEMENT_CSV)).await.unwrap();
    assert_eq!(again.staged, 0);
}

#[tokio::test]
async fn deleting_the_account_clears_its_pending_items() {
    use crate::commands::statement_io::{list_statement_pending_in, parse_inter_csv, stage_statement_parsed};

    let db = memory_db().await;
    let ws = workspace(&db, "Cascade").await;
    let checking = crate::test_support::bank_account(&db, ws.id, "Checking").await;
    stage_statement_parsed(&db, ws.id, checking.id, None, true, "inter_csv", parse_inter_csv(STATEMENT_CSV)).await.unwrap();
    assert_eq!(list_statement_pending_in(&db, ws.id).await.unwrap().len(), 4);
    delete_bank_account_in(&db, checking.id).await.unwrap();
    assert!(list_statement_pending_in(&db, ws.id).await.unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Rendimento do porquinho derivado da Movimentação (Fase 35, entrega 4). Dados inventados.
// ---------------------------------------------------------------------------

fn movement_sheet(lines: &[&[&str]]) -> Vec<Vec<String>> {
    let header = ["Entrada/Saída", "Data", "Movimentação", "Produto", "Instituição", "Quantidade", "Preço unitário", "Valor da Operação"];
    std::iter::once(header.as_slice())
        .chain(lines.iter().copied())
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect()
}

/// Conta e Porquinho com o extrato aprovado: aplicação de 100,00 e resgate de 50,31 (o que caiu líquido).
async fn pocket_with_statement(db: &sea_orm::DatabaseConnection) -> (i32, crate::entity::bank_account::Model, crate::entity::bank_account::Model) {
    use crate::commands::statement_io::{parse_inter_csv, stage_statement_parsed};
    let ws = workspace(db, "Yield").await;
    let checking = crate::test_support::bank_account(db, ws.id, "Checking").await;
    let csv = "Data Lançamento;Histórico;Descrição;Valor;Saldo\n\
01/03/2026;Aplicação;Cdb Porquinho Banco X;-100,00;0,00\n\
05/03/2026;Resgate;Cdb Porquinho Banco X;50,31;0,00\n";
    stage_statement_parsed(db, ws.id, checking.id, None, true, "inter_csv", parse_inter_csv(csv)).await.unwrap();
    confirm_all_statement_items(db, ws.id).await;
    let pocket = crate::entity::bank_account::Entity::find()
        .filter(crate::entity::bank_account::Column::Nome.eq("Cdb Porquinho"))
        .one(db)
        .await
        .unwrap()
        .unwrap();
    (ws.id, checking, pocket)
}

async fn account_balance(db: &sea_orm::DatabaseConnection, ws: i32, id: i32) -> f64 {
    list_bank_accounts_view(db, ws).await.unwrap().into_iter().find(|a| a.id == id).unwrap().balance
}

/// Aplicada 63 dias antes do resgate: já passou dos 30 dias do IOF, só o IR incide.
const CDB_SHEET: [&[&str]; 2] = [
    &["Credito", "01/01/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "10000", "0.01", "100"],
    &["Debito", "05/03/2026", "RESGATE ANTECIPADO/", "CDB - CDBAAA - BANCO", "BANCO", "5000", "0.0108", "50.40"],
];

#[tokio::test]
async fn a_cdb_yield_adds_gross_and_ir_and_the_pocket_ends_at_the_remaining_principal() {
    use crate::commands::fixed_income_io::{confirm_yield_pending_in, parse_fixed_income_rows, stage_yield_parsed, EditedYield};
    use crate::commands::statement_io::list_statement_pending_in;
    use crate::commands::tax::load_rule_set;

    let db = memory_db().await;
    let (ws, _checking, pocket) = pocket_with_statement(&db).await;
    // Só o fluxo do extrato: aplicou 100,00 e resgatou 50,31 → 49,69 (o rendimento ficou de fora).
    assert!(close(account_balance(&db, ws, pocket.id).await, 49.69));

    let rules = load_rule_set(&db).await.unwrap();
    let staged = stage_yield_parsed(&db, ws, pocket.id, "b3_fixed_income", parse_fixed_income_rows(&movement_sheet(&CDB_SHEET), &rules))
        .await
        .unwrap();
    assert_eq!((staged.staged, staged.duplicates, staged.skipped.len()), (1, 0, 1), "a aplicação é pulada");

    let pending = list_statement_pending_in(&db, ws).await.unwrap();
    assert_eq!(pending.len(), 1);
    let detail = pending[0].fixed_income.as_ref().unwrap();
    assert_eq!((pending[0].kind.as_str(), pending[0].suggested_type.as_str()), ("fixed_income_yield", "rendimento"));
    assert!(close(detail.gross_yield, 0.40) && close(detail.ir_amount, 0.09) && close(detail.net_yield, 0.31));
    // A checagem do dia: a B3 explica 50,31 (principal 50 + líquido 0,31) e o extrato mostra 50,31 saindo do porquinho.
    assert!(close(detail.day_derived_total, 50.31));
    assert!(close(detail.bank_total_that_day.unwrap(), 50.31));

    confirm_yield_pending_in(
        &db,
        ws,
        pending[0].id,
        EditedYield { transaction_date: "2026-03-05".to_string(), gross_yield: 0.40, iof_amount: 0.0, ir_amount: 0.09, notes: None },
    )
    .await
    .unwrap();

    // 49,69 + 0,40 (receita bruta) − 0,09 (IR retido) = 50,00: o principal que sobrou (10000 − 5000 unidades × 0,01).
    assert!(close(account_balance(&db, ws, pocket.id).await, 50.0));
    let lines = crate::entity::general_transaction::Entity::find()
        .filter(crate::entity::general_transaction::Column::BankAccountId.eq(pocket.id))
        .filter(crate::entity::general_transaction::Column::TransactionType.is_in(["receita", "despesa"]))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(lines.len(), 2);
    let ir = lines.iter().find(|l| l.transaction_type == "despesa").unwrap();
    let taxes = list_categories_view(&db, ws).await.unwrap().into_iter().find(|c| c.nome == "Taxes").unwrap();
    assert_eq!(ir.category_id, Some(taxes.id), "o IR retido cai na categoria Taxes");
    let gross = lines.iter().find(|l| l.transaction_type == "receita").unwrap();
    let yield_category = list_categories_view(&db, ws).await.unwrap().into_iter().find(|c| c.nome == "Yield").unwrap();
    assert_eq!(gross.category_id, Some(yield_category.id), "o rendimento bruto cai na categoria Yield");
    assert!(list_statement_pending_in(&db, ws).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_short_cdb_holding_records_gross_iof_and_ir_as_three_lines() {
    use crate::commands::fixed_income_io::{confirm_yield_pending_in, parse_fixed_income_rows, stage_yield_parsed, EditedYield};
    use crate::commands::statement_io::list_statement_pending_in;
    use crate::commands::tax::load_rule_set;

    let db = memory_db().await;
    let (ws, _checking, pocket) = pocket_with_statement(&db).await;
    // Aplicado em 01/03 e resgatado em 05/03 (4 dias): o IOF leva 86% do rendimento bruto de 0,50.
    let sheet = movement_sheet(&[
        &["Credito", "01/03/2026", "APLICAÇÃO", "CDB - CDBAAA", "BANCO", "10000", "0.01", "100"],
        &["Debito", "05/03/2026", "RESGATE ANTECIPADO/", "CDB - CDBAAA - BANCO", "BANCO", "5000", "0.0101", "50.50"],
    ]);
    let rules = load_rule_set(&db).await.unwrap();
    stage_yield_parsed(&db, ws, pocket.id, "b3_fixed_income", parse_fixed_income_rows(&sheet, &rules)).await.unwrap();

    let item = list_statement_pending_in(&db, ws).await.unwrap().remove(0);
    let detail = item.fixed_income.as_ref().unwrap();
    assert_eq!((detail.holding_days, detail.iof_basis.as_str()), (Some(4), "full"));
    assert!(close(detail.gross_yield, 0.50) && close(detail.iof_amount, 0.43) && close(detail.ir_amount, 0.02) && close(detail.net_yield, 0.05));

    confirm_yield_pending_in(
        &db,
        ws,
        item.id,
        EditedYield {
            transaction_date: "2026-03-05".to_string(),
            gross_yield: detail.gross_yield,
            iof_amount: detail.iof_amount,
            ir_amount: detail.ir_amount,
            notes: None,
        },
    )
    .await
    .unwrap();

    let lines = crate::entity::general_transaction::Entity::find()
        .filter(crate::entity::general_transaction::Column::BankAccountId.eq(pocket.id))
        .filter(crate::entity::general_transaction::Column::TransactionType.is_in(["receita", "despesa"]))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(lines.len(), 3, "receita bruta + IOF + IR");
    assert_eq!(lines.iter().filter(|l| l.transaction_type == "despesa").count(), 2);
    assert!(lines.iter().any(|l| l.notes.as_deref().is_some_and(|n| n.starts_with("IOF"))));
    // 49,69 do extrato + 0,50 − 0,43 − 0,02 = 49,74: só ficou o líquido de 0,05.
    assert!(close(account_balance(&db, ws, pocket.id).await, 49.74));

    // Passar do rendimento bruto somando IOF e IR é recusado.
    stage_yield_parsed(&db, ws, pocket.id, "b3_fixed_income", parse_fixed_income_rows(&movement_sheet(&CDB_SHEET), &rules)).await.unwrap();
    let next = list_statement_pending_in(&db, ws).await.unwrap().remove(0);
    let too_much = EditedYield { transaction_date: "2026-03-05".to_string(), gross_yield: 0.40, iof_amount: 0.30, ir_amount: 0.20, notes: None };
    assert!(confirm_yield_pending_in(&db, ws, next.id, too_much).await.is_err());
}

#[tokio::test]
async fn an_exempt_instrument_records_only_the_gross_line() {
    use crate::commands::fixed_income_io::{confirm_yield_pending_in, parse_fixed_income_rows, stage_yield_parsed, EditedYield};
    use crate::commands::statement_io::list_statement_pending_in;
    use crate::commands::tax::load_rule_set;

    let db = memory_db().await;
    let (ws, _checking, pocket) = pocket_with_statement(&db).await;
    let sheet = movement_sheet(&[
        &["Credito", "01/03/2026", "APLICAÇÃO", "LCI - LCIAAA", "BANCO", "10000", "0.01", "100"],
        &["Debito", "05/03/2026", "RESGATE", "LCI - LCIAAA - BANCO", "BANCO", "5000", "0.0108", "50.40"],
    ]);
    let rules = load_rule_set(&db).await.unwrap();
    stage_yield_parsed(&db, ws, pocket.id, "b3_fixed_income", parse_fixed_income_rows(&sheet, &rules)).await.unwrap();
    let item = list_statement_pending_in(&db, ws).await.unwrap().remove(0);
    let detail = item.fixed_income.as_ref().unwrap();
    assert_eq!((detail.instrument.as_str(), detail.ir_rate, detail.ir_amount), ("lci", 0.0, 0.0));

    confirm_yield_pending_in(
        &db,
        ws,
        item.id,
        EditedYield { transaction_date: "2026-03-05".to_string(), gross_yield: detail.gross_yield, iof_amount: 0.0, ir_amount: 0.0, notes: None },
    )
    .await
    .unwrap();
    let despesas = crate::entity::general_transaction::Entity::find()
        .filter(crate::entity::general_transaction::Column::BankAccountId.eq(pocket.id))
        .filter(crate::entity::general_transaction::Column::TransactionType.eq("despesa"))
        .all(&db)
        .await
        .unwrap();
    assert!(despesas.is_empty(), "isento: nenhuma linha de IR");
}

#[tokio::test]
async fn editing_the_tax_rate_changes_the_proposal_and_a_bad_ir_is_refused() {
    use crate::commands::fixed_income_io::{confirm_yield_pending_in, parse_fixed_income_rows, stage_yield_parsed, EditedYield};
    use crate::commands::statement_io::list_statement_pending_in;
    use crate::commands::tax::load_rule_set;
    use sea_orm::{ActiveModelTrait, Set};

    let db = memory_db().await;
    let (ws, _checking, pocket) = pocket_with_statement(&db).await;
    // O usuário edita a alíquota do CDB em Settings → Taxes (15% em vez de 22,5%).
    let row = crate::entity::tax_rules::Entity::find()
        .filter(crate::entity::tax_rules::Column::RuleKey.eq("fixed_income_ir_cdb"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut active: crate::entity::tax_rules::ActiveModel = row.into();
    active.value = Set(0.15);
    active.update(&db).await.unwrap();

    let rules = load_rule_set(&db).await.unwrap();
    stage_yield_parsed(&db, ws, pocket.id, "b3_fixed_income", parse_fixed_income_rows(&movement_sheet(&CDB_SHEET), &rules)).await.unwrap();
    let item = list_statement_pending_in(&db, ws).await.unwrap().remove(0);
    let detail = item.fixed_income.as_ref().unwrap();
    assert!(close(detail.ir_rate, 0.15) && close(detail.ir_amount, 0.06) && close(detail.net_yield, 0.34));

    for ir in [-0.01, 0.41] {
        let bad = EditedYield { transaction_date: "2026-03-05".to_string(), gross_yield: 0.40, iof_amount: 0.0, ir_amount: ir, notes: None };
        assert!(confirm_yield_pending_in(&db, ws, item.id, bad).await.is_err(), "IR {ir} fora de 0..=bruto");
    }
    assert_eq!(list_statement_pending_in(&db, ws).await.unwrap().len(), 1, "continua pendente");
}

#[tokio::test]
async fn reimporting_the_yields_never_duplicates_even_after_editing_the_gross() {
    use crate::commands::fixed_income_io::{confirm_yield_pending_in, parse_fixed_income_rows, stage_yield_parsed, EditedYield};
    use crate::commands::statement_io::list_statement_pending_in;
    use crate::commands::tax::load_rule_set;

    let db = memory_db().await;
    let (ws, _checking, pocket) = pocket_with_statement(&db).await;
    let rules = load_rule_set(&db).await.unwrap();
    let stage = || async {
        stage_yield_parsed(&db, ws, pocket.id, "b3_fixed_income", parse_fixed_income_rows(&movement_sheet(&CDB_SHEET), &rules))
            .await
            .unwrap()
    };
    assert_eq!(stage().await.staged, 1);
    assert_eq!((stage().await.staged, stage().await.duplicates), (0, 1), "ainda pendente");

    let item = list_statement_pending_in(&db, ws).await.unwrap().remove(0);
    // Aprova corrigindo o bruto (o banco pagou 0,41 em vez de 0,40): a marca `confirmed` segura a reimportação.
    confirm_yield_pending_in(
        &db,
        ws,
        item.id,
        EditedYield { transaction_date: "2026-03-05".to_string(), gross_yield: 0.41, iof_amount: 0.0, ir_amount: 0.09, notes: None },
    )
    .await
    .unwrap();
    let again = stage().await;
    assert_eq!((again.staged, again.duplicates), (0, 1));
}

#[tokio::test]
async fn the_fixed_income_rules_exist_after_the_migrations_and_the_cdb_defaults_to_22_5() {
    let db = memory_db().await;
    let rules = crate::entity::tax_rules::Entity::find()
        .filter(crate::entity::tax_rules::Column::RuleKey.like("fixed_income_ir_%"))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(rules.len(), 5);
    let rate = |key: &str| rules.iter().find(|r| r.rule_key == key).unwrap().value;
    assert!(close(rate("fixed_income_ir_cdb"), 0.225));
    for exempt in ["lci", "lca", "cri", "cra"] {
        assert!(close(rate(&format!("fixed_income_ir_{exempt}")), 0.0));
    }
}

#[tokio::test]
async fn the_migration_keeps_an_old_pending_item_and_gives_it_a_workspace() {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectOptions, ConnectionTrait, Database};

    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    // Aplica tudo até a migration que reconstrói a fila (sem ela), pelo nome, pra
    // não quebrar quando entrarem migrations depois dela.
    let before_rebuild = Migrator::migrations()
        .iter()
        .position(|m| m.name().contains("rebuild_import_pending_by_workspace"))
        .expect("a migration de reconstrução existe") as u32;
    Migrator::up(&db, Some(before_rebuild)).await.unwrap();

    let ws = workspace(&db, "Legacy").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    db.execute_unprepared(&format!(
        "INSERT INTO import_pending (portfolio_id, kind, source, payload, status, created_at) \
         VALUES ({}, 'transaction', 'anchor_csv', '{{}}', 'pending', '2026-01-01T00:00:00Z')",
        pf.id
    ))
    .await
    .unwrap();

    Migrator::up(&db, None).await.unwrap();
    let rows = crate::entity::import_pending::Entity::find().all(&db).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].workspace_id, rows[0].portfolio_id), (ws.id, Some(pf.id)));
    assert_eq!(rows[0].bank_account_id, None);
}

// ---------------------------------------------------------------------------
// Renda fixa na carteira (Fase 35, entrega 8): posição importada + marca por conta
// ---------------------------------------------------------------------------

/// Importa as linhas da Movimentação pelo caminho real (leitor → commit) e devolve o resultado do commit.
async fn import_movimentacao(
    db: &sea_orm::DatabaseConnection,
    portfolio_id: i32,
    lines: &[&[&str]],
) -> crate::commands::portfolio_io::CommitResult {
    use crate::commands::portfolio_io::{commit_rows, parse_b3_movimentacao_rows, CommitItem};
    let mut rows: Vec<Vec<String>> = vec![[
        "Entrada/Saída",
        "Data",
        "Movimentação",
        "Produto",
        "Instituição",
        "Quantidade",
        "Preço unitário",
        "Valor da Operação",
    ]
    .iter()
    .map(|c| c.to_string())
    .collect()];
    rows.extend(lines.iter().map(|l| l.iter().map(|c| c.to_string()).collect()));
    let rules = crate::commands::tax::load_rule_set(db).await.unwrap();
    let items: Vec<CommitItem> = parse_b3_movimentacao_rows(&rows, &rules)
        .into_iter()
        .map(|r| CommitItem { proposed: r.expect("linha lida"), asset_class: None })
        .collect();
    commit_rows(db, portfolio_id, items).await.unwrap()
}

const FI_APPLICATION: [&str; 8] =
    ["Debito", "02/03/2026", "APLICAÇÃO", "CDB - CDBAAA111 - BANCO X", "BANCO X", "100000", "0.01", "1000"];
const FI_REDEMPTION: [&str; 8] =
    ["Credito", "20/03/2026", "RESGATE", "CDB - CDBAAA111 - BANCO X", "BANCO X", "60000", "0.0102", "1020"];

async fn mark_mirrored(db: &sea_orm::DatabaseConnection, account: &crate::entity::bank_account::Model, on: bool) {
    update_bank_account_in(
        db,
        UpdateBankAccountRequest {
            bank_account_id: account.id,
            nome: account.nome.clone(),
            titular: account.titular.clone(),
            currency: None,
            mirrored_in_portfolio: Some(on),
            auto_yield_pct_of_cdi: None,
            clear_auto_yield: false,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn an_imported_fixed_income_position_is_not_counted_twice_when_the_account_is_marked() {
    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let pf = portfolio(&db, ws.id, "B3").await;
    let porquinho = bank_account(&db, ws.id, "Porquinho").await;
    cash_in(&db, ws.id, porquinho.id, 1000.0, "2026-03-02").await;

    let result = import_movimentacao(&db, pf.id, &[&FI_APPLICATION]).await;
    assert_eq!((result.created, result.errors.len()), (1, 0));

    // Sem marca: o mesmo dinheiro conta na carteira e no caixa, como antes de a conta ser marcada.
    let unmarked = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(unmarked.total_market_value, 1000.0), "posição a custo: {}", unmarked.total_market_value);
    assert!(close(unmarked.total_cash, 1000.0) && close(unmarked.cash_in_portfolio, 0.0));
    assert!(close(unmarked.net_worth, 2000.0));

    // Com a marca: o caixa cede o que a carteira já tem.
    mark_mirrored(&db, &porquinho, true).await;
    let marked = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(marked.total_market_value, 1000.0));
    assert!(close(marked.total_cash, 0.0) && close(marked.cash_in_portfolio, 1000.0));
    assert!(close(marked.net_worth, 1000.0), "o patrimônio não pode dobrar: {}", marked.net_worth);

    // Desmarcar volta ao que era.
    mark_mirrored(&db, &porquinho, false).await;
    assert!(close(compute_net_worth(&db, ws.id).await.unwrap().net_worth, 2000.0));
}

#[tokio::test]
async fn a_fixed_income_etf_is_a_class_of_its_own_outside_the_mirrored_cash_and_the_tax_assessment() {
    use crate::commands::portfolio_io::{class_tax_hint_text, commit_rows, CommitItem, ImportStatus};

    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let pf = portfolio(&db, ws.id, "B3").await;
    let porquinho = bank_account(&db, ws.id, "Porquinho").await;
    cash_in(&db, ws.id, porquinho.id, 1000.0, "2026-03-02").await;
    mark_mirrored(&db, &porquinho, true).await;

    // O final 11 pede a classe; escolhida a do ETF de renda fixa, o ativo nasce em BRL.
    let csv = "date,type,ticker,quantity,unit_price,total_value\n2026-03-05,compra,AUPO11,10,100,1000\n";
    let rows = preview_csv(&db, pf.id, csv).await;
    assert_eq!(rows[0].status, ImportStatus::NeedsClass);
    let item = CommitItem { proposed: rows[0].proposed.clone().unwrap(), asset_class: Some("etf_renda_fixa".to_string()) };
    assert_eq!(commit_rows(&db, pf.id, vec![item]).await.unwrap().created, 1);
    let asset = crate::entity::assets::Entity::find().all(&db).await.unwrap().remove(0);
    assert_eq!((asset.asset_class.as_str(), asset.currency.as_str()), ("etf_renda_fixa", "BRL"));

    // Fora do desconto de caixa: a renda fixa de verdade cede o caixa da conta marcada, o ETF não.
    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_market_value, 1000.0), "a posição conta: {}", nw.total_market_value);
    assert!(close(nw.cash_in_portfolio, 0.0) && close(nw.total_cash, 1000.0), "o caixa não cede nada ao ETF");

    // Fora da apuração, com o aviso próprio na revisão, e cotada como as outras classes da B3.
    assert!(class_tax_hint_text("etf_renda_fixa", "AUPO11").contains("withheld at source"));
    assert!(crate::commands::profitability::ASSET_CLASSES_WITH_AUTO_QUOTE.contains(&"etf_renda_fixa"));
    assert!(crate::domain::position_pricing::ASSET_CLASSES_WITH_AUTO_QUOTE.contains(&"etf_renda_fixa"));
}

#[tokio::test]
async fn a_partial_redemption_leaves_the_rest_in_the_position_and_the_yield_stays_out() {
    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let pf = portfolio(&db, ws.id, "B3").await;
    let porquinho = bank_account(&db, ws.id, "Porquinho").await;
    mark_mirrored(&db, &porquinho, true).await;
    // Saldo da conta depois do resgate: principal que ficou (400) + o que voltou da B3 e foi gasto, aqui 100 de folga.
    cash_in(&db, ws.id, porquinho.id, 500.0, "2026-03-02").await;

    let result = import_movimentacao(&db, pf.id, &[&FI_APPLICATION, &FI_REDEMPTION]).await;
    assert_eq!((result.created, result.errors.len()), (2, 0));

    // A venda é ao custo (600 de 1000): sobram 400 na posição, e os 1020 do resgate não viram ganho realizado.
    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.total_market_value, 400.0), "sobra a custo: {}", nw.total_market_value);
    // O caixa cede só o que a carteira cobre (400 de 500); os 100 restantes seguem como caixa.
    assert!(close(nw.cash_in_portfolio, 400.0) && close(nw.total_cash, 100.0));
    assert!(close(nw.net_worth, 500.0));
}

#[tokio::test]
async fn the_cash_never_goes_negative_when_the_portfolio_has_more_than_the_marked_balance() {
    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let pf = portfolio(&db, ws.id, "B3").await;
    let porquinho = bank_account(&db, ws.id, "Porquinho").await;
    mark_mirrored(&db, &porquinho, true).await;
    cash_in(&db, ws.id, porquinho.id, 300.0, "2026-03-02").await;

    import_movimentacao(&db, pf.id, &[&FI_APPLICATION]).await;

    // A carteira tem 1000 e a conta só 300: a conta cede 300, nunca mais.
    let nw = compute_net_worth(&db, ws.id).await.unwrap();
    assert!(close(nw.cash_in_portfolio, 300.0) && close(nw.total_cash, 0.0));
    assert!(close(nw.net_worth, 1000.0));
}

#[tokio::test]
async fn a_redemption_without_its_position_is_refused_instead_of_going_negative() {
    use crate::commands::portfolio_io::{commit_rows, parse_b3_movimentacao_rows, CommitItem};
    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let pf = portfolio(&db, ws.id, "B3").await;

    // Só o resgate, sem a aplicação (nem no arquivo nem na carteira): a venda passaria da posição.
    let header: Vec<String> = [
        "Entrada/Saída",
        "Data",
        "Movimentação",
        "Produto",
        "Instituição",
        "Quantidade",
        "Preço unitário",
        "Valor da Operação",
    ]
    .iter()
    .map(|c| c.to_string())
    .collect();
    let sheet = vec![header, FI_REDEMPTION.iter().map(|c| c.to_string()).collect()];
    let rules = crate::commands::tax::load_rule_set(&db).await.unwrap();
    // Sem aplicação no arquivo não há preço-base: o leitor pula a linha com o motivo, em vez de inventar um.
    let parsed = parse_b3_movimentacao_rows(&sheet, &rules);
    assert!(parsed[0].as_ref().unwrap_err().message.contains("sem preço-base"));

    // Com o preço-base vindo de outra aplicação do arquivo, a venda sai com aviso e a validação a recusa.
    let other_application = ["Debito", "01/03/2026", "APLICAÇÃO", "CDB - CDBZZZ999 - BANCO X", "BANCO X", "10", "0.01", "0.1"];
    let mut sheet = sheet;
    sheet.push(other_application.iter().map(|c| c.to_string()).collect());
    let items: Vec<CommitItem> = parse_b3_movimentacao_rows(&sheet, &rules)
        .into_iter()
        .map(|r| CommitItem { proposed: r.expect("linha lida"), asset_class: None })
        .filter(|item| item.proposed.transaction_type == "venda")
        .collect();
    assert_eq!(items.len(), 1);
    assert!(items[0].proposed.notes.as_deref().unwrap().contains("não está toda"));
    let result = commit_rows(&db, pf.id, items).await.unwrap();
    assert_eq!(result.created, 0);
    assert!(result.errors[0].message.contains("quantidade em carteira"), "{}", result.errors[0].message);
}

// ---------------------------------------------------------------------------
// Conciliação do saldo pelo saldo real do banco (Fase 35, entrega 9)
// ---------------------------------------------------------------------------

fn reconcile(account_id: i32, bank_balance: f64, date: &str) -> crate::commands::balance_reconciliation::ReconcileRequest {
    crate::commands::balance_reconciliation::ReconcileRequest {
        bank_account_id: account_id,
        bank_balance,
        date: date.to_string(),
        adjustment: false,
    }
}

#[tokio::test]
async fn a_balance_adjustment_books_the_difference_up_or_down_and_never_counts_as_yield() {
    use crate::commands::balance_reconciliation::{confirm_balance_reconciliation_in, ADJUSTMENT_NOTE_PREFIX};
    use crate::commands::bank_account::balance_on;
    let db = memory_db().await;
    let (_, account) = porquinho_with_flows(&db).await; // saldo do ledger em 31/03: 600
    let adjust = |balance: f64, date: &str| {
        let mut request = reconcile(account.id, balance, date);
        request.adjustment = true;
        request
    };

    // Para cima (saldo de abertura): receita sem categoria de rendimento.
    let up = confirm_balance_reconciliation_in(&db, adjust(1000.0, "2026-03-31")).await.unwrap();
    assert_eq!((up.transaction_type.as_str(), up.category_id), ("receita", None));
    assert!(close(up.valor, 400.0) && up.notes.as_deref().unwrap().starts_with(ADJUSTMENT_NOTE_PREFIX));
    assert!(close(balance_on(&db, account.id, "2026-03-31").await.unwrap(), 1000.0));

    // Para baixo (caixa que sobrou): despesa, que o modo de rendimento recusaria.
    let down = confirm_balance_reconciliation_in(&db, adjust(950.0, "2026-03-31")).await.unwrap();
    assert!(down.transaction_type == "despesa" && close(down.valor, 50.0));
    assert!(close(balance_on(&db, account.id, "2026-03-31").await.unwrap(), 950.0));
    assert!(confirm_balance_reconciliation_in(&db, reconcile(account.id, 900.0, "2026-03-31")).await.is_err(), "rendimento não vai para baixo");

    // Conciliar de novo no mesmo dia não duplica.
    let again = confirm_balance_reconciliation_in(&db, adjust(950.0, "2026-03-31")).await;
    assert!(matches!(again, Err(e) if e.to_string().contains("nothing to reconcile")));
}

/// Conta com 1.000 aplicados em 01/03 e 400 resgatados em 10/03 (saldo do ledger: 600, sem rendimento).
async fn porquinho_with_flows(db: &sea_orm::DatabaseConnection) -> (crate::entity::workspace::Model, crate::entity::bank_account::Model) {
    use crate::commands::general_transaction::{create_general_transaction_in, CreateGeneralTransactionRequest};
    let ws = workspace(db, "Family").await;
    let account = bank_account(db, ws.id, "Porquinho").await;
    cash_in(db, ws.id, account.id, 1000.0, "2026-03-01").await;
    create_general_transaction_in(
        db,
        CreateGeneralTransactionRequest {
            workspace_id: ws.id,
            bank_account_id: account.id,
            transaction_type: "despesa".to_string(),
            category_id: None,
            valor: 400.0,
            transaction_date: "2026-03-10".to_string(),
            notes: None,
        },
    )
    .await
    .unwrap();
    (ws, account)
}

#[tokio::test]
async fn reconciling_with_the_bank_balance_books_the_missing_yield_once() {
    use crate::commands::balance_reconciliation::{confirm_balance_reconciliation_in, preview_balance_reconciliation_in};
    use crate::commands::bank_account::balance_on;
    let db = memory_db().await;
    let (_, account) = porquinho_with_flows(&db).await;

    let preview = preview_balance_reconciliation_in(&db, reconcile(account.id, 603.25, "2026-03-31")).await.unwrap();
    assert!(close(preview.ledger_balance, 600.0) && close(preview.difference, 3.25));

    let booked = confirm_balance_reconciliation_in(&db, reconcile(account.id, 603.25, "2026-03-31")).await.unwrap();
    assert_eq!((booked.transaction_type.as_str(), booked.transaction_date.as_str()), ("receita", "2026-03-31"));
    assert!(close(booked.valor, 3.25));
    let yield_category = list_categories_view(&db, account.workspace_id).await.unwrap().into_iter().find(|c| c.nome == "Yield").unwrap();
    assert_eq!(booked.category_id, Some(yield_category.id), "a receita da conciliação cai na categoria Yield");
    assert!(close(balance_on(&db, account.id, "2026-03-31").await.unwrap(), 603.25), "o ledger passa a bater com o banco");

    // Conciliar de novo no mesmo dia não duplica: a diferença já é zero.
    let again = confirm_balance_reconciliation_in(&db, reconcile(account.id, 603.25, "2026-03-31")).await;
    assert!(again.unwrap_err().to_string().contains("nothing to reconcile"));
}

#[tokio::test]
async fn a_yield_already_approved_from_the_b3_file_is_not_counted_again() {
    use crate::commands::balance_reconciliation::preview_balance_reconciliation_in;
    let db = memory_db().await;
    let (ws, account) = porquinho_with_flows(&db).await;
    // Rendimento de um resgate que a B3 explicou e que o dono já aprovou: está no ledger.
    cash_in(&db, ws.id, account.id, 1.5, "2026-03-10").await;

    let preview = preview_balance_reconciliation_in(&db, reconcile(account.id, 603.25, "2026-03-31")).await.unwrap();
    assert!(close(preview.ledger_balance, 601.5) && close(preview.difference, 1.75));
}

#[tokio::test]
async fn the_ledger_balance_is_taken_at_the_chosen_date() {
    use crate::commands::balance_reconciliation::preview_balance_reconciliation_in;
    let db = memory_db().await;
    let (_, account) = porquinho_with_flows(&db).await;

    // Em 05/03 o resgate de 400 ainda não tinha acontecido.
    let preview = preview_balance_reconciliation_in(&db, reconcile(account.id, 1001.0, "2026-03-05")).await.unwrap();
    assert!(close(preview.ledger_balance, 1000.0) && close(preview.difference, 1.0));
}

#[tokio::test]
async fn a_bank_balance_below_the_ledger_a_future_date_or_a_bad_balance_are_refused() {
    use crate::commands::balance_reconciliation::confirm_balance_reconciliation_in;
    let db = memory_db().await;
    let (_, account) = porquinho_with_flows(&db).await;

    // Banco mostra menos que o ledger: falta um fluxo no extrato, não é rendimento.
    let lower = confirm_balance_reconciliation_in(&db, reconcile(account.id, 590.0, "2026-03-31")).await;
    assert!(lower.unwrap_err().to_string().contains("lower than the ledger"));
    let future = confirm_balance_reconciliation_in(&db, reconcile(account.id, 700.0, "2999-01-01")).await;
    assert!(future.unwrap_err().to_string().contains("future"));
    let negative = confirm_balance_reconciliation_in(&db, reconcile(account.id, -1.0, "2026-03-31")).await;
    assert!(negative.is_err());
    let bad_date = confirm_balance_reconciliation_in(&db, reconcile(account.id, 700.0, "31/03/2026")).await;
    assert!(bad_date.unwrap_err().to_string().contains("invalid date"));
    // Nada foi gravado em nenhum dos casos.
    assert_eq!(list_bank_accounts_view(&db, account.workspace_id).await.unwrap()[0].balance, 600.0);
}

// ---------------------------------------------------------------------------
// Estimativa do rendimento do porquinho por resgate (Fase 35, entrega 10)
// ---------------------------------------------------------------------------

/// Meta Selic de 10,10% (CDI de 10,00%) desde `from`, gravada agora para o app não ir ao BCB.
async fn seed_selic_meta(db: &sea_orm::DatabaseConnection, from: &str) {
    use sea_orm::{ActiveModelTrait, Set};
    crate::entity::selic_daily::ActiveModel {
        series_code: Set("meta".to_string()),
        reference_date: Set(from.to_string()),
        value_pct: Set(10.10),
        source: Set("test".to_string()),
        fetched_at: Set(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
}

async fn pocket_leg(db: &sea_orm::DatabaseConnection, ws: i32, account: i32, kind: &str, valor: f64, date: &str) {
    use sea_orm::{ActiveModelTrait, Set};
    crate::entity::general_transaction::ActiveModel {
        workspace_id: Set(ws),
        bank_account_id: Set(account),
        transaction_type: Set(kind.to_string()),
        category_id: Set(None),
        valor: Set(valor),
        transaction_date: Set(date.to_string()),
        notes: Set(None),
        created_at: Set("2026-01-01T00:00:00Z".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
}

fn estimate_request(account: i32, pct: f64) -> crate::commands::porquinho_estimate::EstimateRequest {
    crate::commands::porquinho_estimate::EstimateRequest { bank_account_id: account, pct_of_cdi: pct }
}

/// Porquinho com 1.000 aplicados em 03/03/2025 e 250,00 pagos num resgate em 03/06/2025.
async fn porquinho_for_estimate(db: &sea_orm::DatabaseConnection) -> (crate::entity::workspace::Model, crate::entity::bank_account::Model) {
    let ws = workspace(db, "Family").await;
    let account = bank_account(db, ws.id, "Porquinho").await;
    pocket_leg(db, ws.id, account.id, "transferencia_entrada", 1000.0, "2025-03-03").await;
    pocket_leg(db, ws.id, account.id, "transferencia_saida", 250.0, "2025-06-03").await;
    (ws, account)
}

#[tokio::test]
async fn the_estimate_queues_a_yield_for_the_redemption_and_approving_it_leaves_the_principal_in_the_account() {
    use crate::commands::bank_account::balance_on;
    use crate::commands::fixed_income_io::{confirm_yield_pending_in, EditedYield, YieldPayload};
    use crate::commands::porquinho_estimate::stage_porquinho_yield_estimate_in;
    let db = memory_db().await;
    let (ws, account) = porquinho_for_estimate(&db).await;
    seed_selic_meta(&db, "2024-01-01").await;

    let result = stage_porquinho_yield_estimate_in(&db, estimate_request(account.id, 1.0)).await.unwrap();
    assert_eq!((result.redemptions, result.staged, result.skipped_covered_day, result.duplicates), (1, 1, 0, 0));

    let queued = crate::entity::import_pending::Entity::find().all(&db).await.unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!((queued[0].kind.as_str(), queued[0].source.as_str()), ("fixed_income_yield", "porquinho_estimate"));
    let p = serde_json::from_str::<YieldPayload>(&queued[0].payload).unwrap().proposed;
    assert_eq!((p.base_price_source.as_str(), p.estimate_pct_of_cdi), ("estimate", Some(1.0)));
    assert_eq!(p.transaction_date, "2025-06-03");
    assert!(close(p.principal + p.net_yield, 250.0), "o pago fecha no centavo: {} + {}", p.principal, p.net_yield);
    // Três meses a 10% ao ano sobre cerca de 250: poucos reais de rendimento bruto, IR de 22,5% sem IOF.
    assert!(p.gross_yield > 3.0 && p.gross_yield < 9.0, "bruto {}", p.gross_yield);
    assert_eq!(p.iof_amount, 0.0);
    assert!(close(p.ir_amount, (p.gross_yield * 0.225 * 100.0).round() / 100.0) || (p.ir_amount - p.gross_yield * 0.225).abs() < 0.011);
    assert!(close(p.day_derived_total, 250.0));

    // Aprovar grava a receita bruta e o IR; o saldo da conta passa a ser o principal que sobrou.
    confirm_yield_pending_in(
        &db,
        ws.id,
        queued[0].id,
        EditedYield {
            transaction_date: p.transaction_date.clone(),
            gross_yield: p.gross_yield,
            iof_amount: p.iof_amount,
            ir_amount: p.ir_amount,
            notes: None,
        },
    )
    .await
    .unwrap();
    let left = balance_on(&db, account.id, "2025-12-31").await.unwrap();
    assert!((left - (1000.0 - p.principal)).abs() < 0.011, "saldo {left} contra principal que sobrou {}", 1000.0 - p.principal);
}

#[tokio::test]
async fn a_day_that_already_has_a_yield_is_skipped_and_running_twice_does_not_duplicate() {
    use crate::commands::porquinho_estimate::stage_porquinho_yield_estimate_in;
    let db = memory_db().await;
    let (ws, account) = porquinho_for_estimate(&db).await;
    seed_selic_meta(&db, "2024-01-01").await;

    // Rendimento já lançado no dia do resgate (por exemplo, o que a B3 explicou): não há o que estimar.
    cash_in(&db, ws.id, account.id, 4.0, "2025-06-03").await;
    let covered = stage_porquinho_yield_estimate_in(&db, estimate_request(account.id, 1.0)).await.unwrap();
    assert_eq!((covered.staged, covered.skipped_covered_day), (0, 1));
}

#[tokio::test]
async fn running_the_estimate_twice_leaves_one_item_in_the_queue() {
    use crate::commands::porquinho_estimate::stage_porquinho_yield_estimate_in;
    let db = memory_db().await;
    let (_, account) = porquinho_for_estimate(&db).await;
    seed_selic_meta(&db, "2024-01-01").await;

    assert_eq!(stage_porquinho_yield_estimate_in(&db, estimate_request(account.id, 1.0)).await.unwrap().staged, 1);
    let again = stage_porquinho_yield_estimate_in(&db, estimate_request(account.id, 1.0)).await.unwrap();
    assert_eq!((again.staged, again.skipped_covered_day), (0, 1), "o dia já tem item na fila");
    assert_eq!(crate::entity::import_pending::Entity::find().all(&db).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_reconciliation_yield_does_not_mark_its_day_as_covered() {
    use crate::commands::balance_reconciliation::{confirm_balance_reconciliation_in, ReconcileRequest};
    use crate::commands::porquinho_estimate::stage_porquinho_yield_estimate_in;
    let db = memory_db().await;
    let (_, account) = porquinho_for_estimate(&db).await;
    seed_selic_meta(&db, "2024-01-01").await;

    // Ledger em 03/06/2025: 1000 − 250 = 750. O banco mostra 760: a conciliação grava 10 nesse mesmo dia.
    confirm_balance_reconciliation_in(
        &db,
        ReconcileRequest { bank_account_id: account.id, bank_balance: 760.0, date: "2025-06-03".to_string(), adjustment: false },
    )
    .await
    .unwrap();
    let result = stage_porquinho_yield_estimate_in(&db, estimate_request(account.id, 1.0)).await.unwrap();
    assert_eq!((result.staged, result.skipped_covered_day), (1, 0));
}

#[tokio::test]
async fn a_selic_history_that_does_not_reach_the_first_application_is_refused() {
    use crate::commands::porquinho_estimate::stage_porquinho_yield_estimate_in;
    let db = memory_db().await;
    let (_, account) = porquinho_for_estimate(&db).await;
    seed_selic_meta(&db, "2025-05-01").await;

    let err = stage_porquinho_yield_estimate_in(&db, estimate_request(account.id, 1.0)).await.unwrap_err();
    assert!(err.to_string().contains("Selic history"), "{err}");
    assert!(crate::entity::import_pending::Entity::find().all(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn the_estimate_refuses_an_out_of_range_percentage_and_does_nothing_without_redemptions() {
    use crate::commands::porquinho_estimate::stage_porquinho_yield_estimate_in;
    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let empty = bank_account(&db, ws.id, "Porquinho").await;
    seed_selic_meta(&db, "2024-01-01").await;

    assert!(stage_porquinho_yield_estimate_in(&db, estimate_request(empty.id, 0.0)).await.is_err());
    assert!(stage_porquinho_yield_estimate_in(&db, estimate_request(empty.id, 5.0)).await.is_err());
    let none = stage_porquinho_yield_estimate_in(&db, estimate_request(empty.id, 1.0)).await.unwrap();
    assert_eq!((none.redemptions, none.staged), (0, 0));
}

// ---------------------------------------------------------------------------
// Estimativa automática do porquinho ao fechar a revisão do extrato (Fase 35, entrega 11)
// ---------------------------------------------------------------------------

async fn queued_yields(db: &sea_orm::DatabaseConnection) -> Vec<crate::entity::import_pending::Model> {
    crate::entity::import_pending::Entity::find()
        .filter(crate::entity::import_pending::Column::Kind.eq("fixed_income_yield"))
        .all(db)
        .await
        .unwrap()
}

/// Uma proposta de rendimento como a da B3, só com o que os testes olham.
fn b3_yield(date: &str, gross: f64) -> crate::commands::fixed_income_io::ProposedYield {
    crate::commands::fixed_income_io::ProposedYield {
        line: 1,
        transaction_date: date.to_string(),
        instrument: "cdb".to_string(),
        code: "CDBAAA".to_string(),
        quantity: 5000.0,
        principal: 50.0,
        gross_value: 50.0 + gross,
        gross_yield: gross,
        iof_amount: 0.0,
        holding_days: Some(4),
        iof_basis: "full".to_string(),
        ir_rate: 0.225,
        ir_amount: 0.0,
        net_yield: gross,
        base_unit_price: 0.01,
        base_price_source: "application".to_string(),
        day_derived_total: 50.0 + gross,
        estimate_pct_of_cdi: None,
    }
}

/// Importa o extrato de teste e aprova tudo menos o último item; devolve a conta Porquinho criada.
async fn statement_reviewed_up_to_the_last_item(
    db: &sea_orm::DatabaseConnection,
    ws: i32,
) -> (crate::entity::bank_account::Model, crate::commands::statement_io::StatementPendingView) {
    use crate::commands::statement_io::{confirm_statement_pending_in, list_statement_pending_in, parse_inter_csv, stage_statement_parsed};
    let checking = crate::test_support::bank_account(db, ws, "Checking").await;
    stage_statement_parsed(db, ws, checking.id, None, true, "inter_csv", parse_inter_csv(STATEMENT_CSV)).await.unwrap();
    let mut items = list_statement_pending_in(db, ws).await.unwrap();
    let last = items.pop().unwrap();
    for item in &items {
        confirm_statement_pending_in(db, ws, item.id, edited_from(item)).await.unwrap();
    }
    let pocket = list_bank_accounts_view(db, ws).await.unwrap().into_iter().find(|a| a.nome == "Cdb Porquinho").unwrap();
    let pocket = crate::entity::bank_account::Entity::find_by_id(pocket.id).one(db).await.unwrap().unwrap();
    (pocket, last)
}

#[tokio::test]
async fn finishing_the_statement_review_estimates_the_porquinho_yield_by_itself() {
    use crate::commands::statement_io::confirm_statement_pending_in;
    let db = memory_db().await;
    let ws = workspace(&db, "Auto").await;
    seed_selic_meta(&db, "2024-01-01").await;
    let (pocket, last) = statement_reviewed_up_to_the_last_item(&db, ws.id).await;

    // A primeira transferência de porquinho aprovada liga a estimativa automática a 100% do CDI.
    assert_eq!(pocket.auto_yield_pct_of_cdi, Some(1.0));
    // Com um item ainda na fila, nada é estimado: o extrato está pela metade.
    assert!(queued_yields(&db).await.is_empty());

    confirm_statement_pending_in(&db, ws.id, last.id, edited_from(&last)).await.unwrap();
    let queued = queued_yields(&db).await;
    assert_eq!(queued.len(), 1, "o resgate de 20/01 ganha uma estimativa");
    assert_eq!(queued[0].source, "porquinho_estimate");
    assert_eq!(queued[0].bank_account_id, Some(pocket.id));
}

#[tokio::test]
async fn an_account_without_the_mark_is_not_estimated() {
    use crate::commands::statement_io::confirm_statement_pending_in;
    let db = memory_db().await;
    let ws = workspace(&db, "Off").await;
    seed_selic_meta(&db, "2024-01-01").await;
    let (pocket, last) = statement_reviewed_up_to_the_last_item(&db, ws.id).await;

    update_bank_account_in(
        &db,
        UpdateBankAccountRequest {
            bank_account_id: pocket.id,
            nome: pocket.nome.clone(),
            titular: pocket.titular.clone(),
            currency: None,
            mirrored_in_portfolio: None,
            auto_yield_pct_of_cdi: None,
            clear_auto_yield: true,
        },
    )
    .await
    .unwrap();
    confirm_statement_pending_in(&db, ws.id, last.id, edited_from(&last)).await.unwrap();
    assert!(queued_yields(&db).await.is_empty());
}

#[tokio::test]
async fn a_failing_estimate_never_fails_the_approval() {
    use crate::commands::statement_io::confirm_statement_pending_in;
    let db = memory_db().await;
    let ws = workspace(&db, "NoSelic").await;
    // Sem Selic gravada, a estimativa não tem como rodar: a aprovação do último item segue valendo.
    let (_, last) = statement_reviewed_up_to_the_last_item(&db, ws.id).await;
    confirm_statement_pending_in(&db, ws.id, last.id, edited_from(&last)).await.unwrap();
    assert!(queued_yields(&db).await.is_empty());
    assert!(crate::commands::statement_io::list_statement_pending_in(&db, ws.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn the_b3_value_replaces_a_pending_estimate_of_the_same_day() {
    use crate::commands::fixed_income_io::stage_yield_parsed;
    use crate::commands::statement_io::confirm_statement_pending_in;
    let db = memory_db().await;
    let ws = workspace(&db, "B3 after").await;
    seed_selic_meta(&db, "2024-01-01").await;
    let (pocket, last) = statement_reviewed_up_to_the_last_item(&db, ws.id).await;
    confirm_statement_pending_in(&db, ws.id, last.id, edited_from(&last)).await.unwrap();
    assert_eq!(queued_yields(&db).await[0].source, "porquinho_estimate");

    let result = stage_yield_parsed(&db, ws.id, pocket.id, "b3_fixed_income", vec![Ok(b3_yield("2025-01-20", 0.04))]).await.unwrap();
    assert_eq!(result.staged, 1);
    let queued = queued_yields(&db).await;
    assert_eq!(queued.len(), 1, "a estimativa pendente saiu e ficou só a da B3");
    assert_eq!(queued[0].source, "b3_fixed_income");
}

#[tokio::test]
async fn the_b3_value_skips_a_day_whose_estimate_was_already_booked() {
    use crate::commands::fixed_income_io::{confirm_yield_pending_in, stage_yield_parsed, EditedYield, YieldPayload};
    use crate::commands::statement_io::confirm_statement_pending_in;
    let db = memory_db().await;
    let ws = workspace(&db, "B3 late").await;
    seed_selic_meta(&db, "2024-01-01").await;
    let (pocket, last) = statement_reviewed_up_to_the_last_item(&db, ws.id).await;
    confirm_statement_pending_in(&db, ws.id, last.id, edited_from(&last)).await.unwrap();
    let estimate = queued_yields(&db).await.remove(0);
    let p = serde_json::from_str::<YieldPayload>(&estimate.payload).unwrap().proposed;
    confirm_yield_pending_in(
        &db,
        ws.id,
        estimate.id,
        EditedYield {
            transaction_date: p.transaction_date.clone(),
            gross_yield: p.gross_yield,
            iof_amount: p.iof_amount,
            ir_amount: p.ir_amount,
            notes: None,
        },
    )
    .await
    .unwrap();

    // Importar a B3 depois do dia já ter estimativa lançada não conta o rendimento duas vezes.
    let result = stage_yield_parsed(&db, ws.id, pocket.id, "b3_fixed_income", vec![Ok(b3_yield("2025-01-20", 0.04))]).await.unwrap();
    assert_eq!((result.staged, result.skipped.len()), (0, 1));
    assert!(result.skipped[0].message.contains("estimado"), "{}", result.skipped[0].message);
}

#[tokio::test]
async fn the_automatic_percentage_is_validated_and_can_be_cleared() {
    let db = memory_db().await;
    let ws = workspace(&db, "Pct").await;
    let account = bank_account(&db, ws.id, "Porquinho").await;
    let update = |pct: Option<f64>, clear: bool| UpdateBankAccountRequest {
        bank_account_id: account.id,
        nome: account.nome.clone(),
        titular: account.titular.clone(),
        currency: None,
        mirrored_in_portfolio: None,
        auto_yield_pct_of_cdi: pct,
        clear_auto_yield: clear,
    };
    assert!(update_bank_account_in(&db, update(Some(5.0), false)).await.is_err());
    assert!(update_bank_account_in(&db, update(Some(0.0), false)).await.is_err());
    assert_eq!(update_bank_account_in(&db, update(Some(0.98), false)).await.unwrap().auto_yield_pct_of_cdi, Some(0.98));
    // Omitido não muda; desligar grava 0 (e não nulo: nulo é "ainda não decidido").
    assert_eq!(update_bank_account_in(&db, update(None, false)).await.unwrap().auto_yield_pct_of_cdi, Some(0.98));
    assert_eq!(update_bank_account_in(&db, update(None, true)).await.unwrap().auto_yield_pct_of_cdi, Some(0.0));
}

#[tokio::test]
async fn the_mirrored_flag_exists_with_a_false_default_and_travels_with_the_account_view() {
    let db = memory_db().await;
    let ws = workspace(&db, "Family").await;
    let account = bank_account(&db, ws.id, "Porquinho").await;
    assert!(!account.mirrored_in_portfolio);
    mark_mirrored(&db, &account, true).await;
    let views = list_bank_accounts_view(&db, ws.id).await.unwrap();
    assert!(views[0].mirrored_in_portfolio);
}

// ---------------------------------------------------------------------------
// Caixa da conta Corretora: aporte do banco e liquidação da B3 (Fase 35, entrega 12)
// ---------------------------------------------------------------------------

const BROKER_STATEMENT_CSV: &str = "Extrato Conta Corrente\nConta ;111\n\nData Lançamento;Histórico;Descrição;Valor;Saldo\n\
10/03/2025;Pix recebido;Pessoa A;2.000,00;0,00\n\
11/03/2025;Transferência enviada;Titular;-1.000,00;0,00\n\
20/03/2025;Transferência recebida;Titular;100,00;0,00\n";

fn settlement_sheet() -> Vec<Vec<String>> {
    let header = ["Entrada/Saída", "Data", "Movimentação", "Produto", "Instituição", "Quantidade", "Preço unitário", "Valor da Operação"];
    let body: [[&str; 8]; 4] = [
        ["Credito", "13/03/2025", "Transferência - Liquidação", "ABCD3 - EMPRESA S.A.", "CORRETORA", "100", "9", "900"],
        ["Debito", "21/03/2025", "Transferência - Liquidação", "ABCD3 - EMPRESA S.A.", "CORRETORA", "40", "10", "400"],
        ["Debito", "21/03/2025", "Transferência - Liquidação", "WXYZ4 - OUTRA S.A.", "CORRETORA", "5", "-", "-"],
        ["Credito", "25/03/2025", "Dividendo", "ABCD3 - EMPRESA S.A.", "CORRETORA", "100", "0.1", "10"],
    ];
    std::iter::once(header.to_vec()).chain(body.iter().map(|r| r.to_vec())).map(|r| r.into_iter().map(String::from).collect()).collect()
}

#[tokio::test]
async fn without_a_brokerage_account_the_bank_transfers_stay_income_and_expenses() {
    use crate::commands::statement_io::{list_statement_pending_in, parse_inter_csv, stage_statement_parsed};

    let db = memory_db().await;
    let ws = workspace(&db, "Plain").await;
    let checking = crate::test_support::bank_account(&db, ws.id, "Checking").await;
    let staged = stage_statement_parsed(&db, ws.id, checking.id, None, false, "inter_csv", parse_inter_csv(BROKER_STATEMENT_CSV)).await.unwrap();
    assert_eq!(staged.staged, 3);
    let kinds: Vec<String> = list_statement_pending_in(&db, ws.id).await.unwrap().iter().map(|p| p.suggested_type.clone()).collect();
    assert_eq!(kinds, vec!["receita", "despesa", "receita"]);
}

#[tokio::test]
async fn broker_transfers_and_b3_settlements_move_the_brokerage_cash_without_double_counting() {
    use crate::commands::broker_settlement_io::parse_b3_settlements;
    use crate::commands::statement_io::{list_statement_pending_in, parse_inter_csv, stage_statement_parsed, stage_statement_with_broker};

    let db = memory_db().await;
    let ws = workspace(&db, "Broker").await;
    let checking = crate::test_support::bank_account(&db, ws.id, "Checking").await;
    let broker = crate::test_support::bank_account(&db, ws.id, "Brokerage").await;

    // Com a conta Corretora, a transferência do banco vira aporte e saque dela.
    let parsed = parse_inter_csv(BROKER_STATEMENT_CSV);
    let staged = stage_statement_with_broker(&db, ws.id, checking.id, None, false, Some(broker.id), "inter_csv", parsed).await.unwrap();
    assert_eq!(staged.staged, 3);
    let pending = list_statement_pending_in(&db, ws.id).await.unwrap();
    assert_eq!(pending.iter().map(|p| p.suggested_type.as_str()).collect::<Vec<_>>(), vec!["receita", "transferencia", "transferencia"]);
    assert_eq!(pending[1].counterpart_account_id, Some(broker.id));

    // A liquidação vira lançamento da Corretora: compra sai, venda entra, e a do aluguel (sem valor) fica de fora.
    let rows = settlement_sheet();
    let settlements = stage_statement_parsed(&db, ws.id, broker.id, None, false, "b3_settlement", parse_b3_settlements(&rows)).await.unwrap();
    assert_eq!((settlements.staged, settlements.skipped.len()), (3, 1), "compra, venda e o dividendo; o aluguel sem valor fica de fora");
    assert!(settlements.skipped[0].message.contains("sem valor"));
    // Importar de novo não duplica.
    let again = stage_statement_parsed(&db, ws.id, broker.id, None, false, "b3_settlement", parse_b3_settlements(&rows)).await.unwrap();
    assert_eq!((again.staged, again.duplicates), (0, 3));

    confirm_all_statement_items(&db, ws.id).await;
    // Banco: +2.000 − 1.000 (aporte) + 100 (saque) = 1.100. Corretora: +1.000 − 100 − 900 (compra) + 400 (venda) + 10 (dividendo) = 410.
    assert!(close(account_balance(&db, ws.id, checking.id).await, 1100.0));
    assert!(close(account_balance(&db, ws.id, broker.id).await, 410.0));
    let total: f64 = list_bank_accounts_view(&db, ws.id).await.unwrap().iter().map(|a| a.balance).sum();
    assert!(close(total, 1510.0), "o aporte só muda o dinheiro de conta; compra, venda e provento é que mexem no total");
}

#[tokio::test]
async fn a_brokerage_account_has_to_be_another_account() {
    use crate::commands::statement_io::{parse_inter_csv, stage_statement_with_broker};

    let db = memory_db().await;
    let ws = workspace(&db, "Same").await;
    let checking = crate::test_support::bank_account(&db, ws.id, "Checking").await;
    let same = stage_statement_with_broker(&db, ws.id, checking.id, None, false, Some(checking.id), "inter_csv", parse_inter_csv(BROKER_STATEMENT_CSV)).await;
    assert!(matches!(same, Err(e) if e.to_string().contains("outra conta")));
}

// ---------------------------------------------------------------------------
// Comando do backtest (Fase 22, fatia 3)
// ---------------------------------------------------------------------------

async fn seed_prices(db: &sea_orm::DatabaseConnection, ticker: &str, adjusted: bool, start: chrono::NaiveDate, prices: &[f64]) {
    use crate::entity::{stock_adjusted_close, stock_price_history};
    use sea_orm::{ActiveModelTrait, Set};
    for (i, price) in prices.iter().enumerate() {
        let date = (start + chrono::Duration::days(i as i64)).to_string();
        if adjusted {
            stock_adjusted_close::ActiveModel {
                ticker: Set(ticker.to_string()),
                price_date: Set(date),
                adj_close_price: Set(*price),
                fetched_at: Set("2026-01-01T00:00:00Z".to_string()),
            }
            .insert(db)
            .await
            .unwrap();
        } else {
            stock_price_history::ActiveModel {
                ticker: Set(ticker.to_string()),
                price_date: Set(date),
                close_price: Set(*price),
                source: Set("test".to_string()),
                fetched_at: Set("2026-01-01T00:00:00Z".to_string()),
                ..Default::default()
            }
            .insert(db)
            .await
            .unwrap();
        }
    }
}

async fn seed_macro(db: &sea_orm::DatabaseConnection, code: &str, months: &[(&str, f64)]) {
    use sea_orm::{ActiveModelTrait, Set};
    for (year_month, value) in months {
        crate::entity::macro_index_monthly::ActiveModel {
            index_code: Set(code.to_string()),
            year_month: Set(year_month.to_string()),
            value_pct: Set(*value),
            source: Set("test".to_string()),
            fetched_at: Set("2026-01-01T00:00:00Z".to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn the_backtest_simulates_the_current_portfolio_and_says_what_it_left_out() {
    use crate::commands::backtest::{run_backtest_in, BacktestRequest, WeightOverride};
    use chrono::NaiveDate;

    let db = memory_db().await;
    let ws = workspace(&db, "Quant").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let a = asset(&db, "AAAA3", "acao_br").await;
    let b = asset(&db, "BBBB3", "acao_br").await;
    let fixed = asset(&db, "CDBX", "renda_fixa").await;
    let usd = asset_in(&db, "SPY", "etf_us", "USD").await;
    for (id, qty, total) in [(a.id, 10.0, 1000.0), (b.id, 10.0, 1000.0), (fixed.id, 1.0, 500.0), (usd.id, 1.0, 100.0)] {
        trade(&db, pf.id, id, "compra", qty, total, "2026-01-01").await;
    }
    let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    // A sobe 1% ao dia e tem ajuste; B fica parado e só tem o fechamento simples (sem ajuste de dividendos).
    let a_prices: Vec<f64> = (0..70).map(|i| 100.0 * 1.01_f64.powi(i)).collect();
    seed_prices(&db, "AAAA3", true, start, &a_prices).await;
    seed_prices(&db, "BBBB3", false, start, &vec![100.0; 70]).await;
    seed_macro(&db, "cdi", &[("2026-02", 1.0), ("2026-03", 1.0)]).await;
    seed_macro(&db, "ipca", &[("2026-02", 0.5)]).await;

    let request = |weights: Vec<WeightOverride>, rebalance: &str| BacktestRequest {
        portfolio_id: pf.id,
        weights,
        rebalance: rebalance.to_string(),
        initial: 1000.0,
        monthly_contribution: 0.0,
        start_date: None,
        brokerage_per_order: 0.0,
        apply_tax: false,
    };
    let hold = run_backtest_in(&db, request(vec![], "never")).await.unwrap();
    let monthly = run_backtest_in(&db, request(vec![], "monthly")).await.unwrap();

    // Só os dois ativos de ação entram; a renda fixa e o ativo em USD saem com aviso, e o B avisa que é só preço.
    assert_eq!(hold.assets.iter().map(|x| x.ticker.as_str()).collect::<Vec<_>>(), vec!["AAAA3", "BBBB3"]);
    assert!(hold.warnings.iter().any(|w| w.starts_with("CDBX") && w.contains("no price history to simulate")), "{:?}", hold.warnings);
    assert!(hold.warnings.iter().any(|w| w.starts_with("SPY") && w.contains("BRL")));
    assert!(hold.warnings.iter().any(|w| w.starts_with("BBBB3") && w.contains("price only")));
    assert!(hold.assets[1].price_only && !hold.assets[0].price_only);

    // Pesos iguais (valor de mercado igual), sem rebalancear o A deriva; rebalanceando mensalmente volta perto de 50%.
    assert!(close(hold.assets[0].target_weight, 0.5));
    assert!(hold.assets[0].final_weight > 0.6 && monthly.assets[0].final_weight < hold.assets[0].final_weight);
    assert_eq!((hold.metrics.rebalance_count, monthly.metrics.rebalance_count), (0, 2));
    assert!(hold.metrics.twr_pct > monthly.metrics.twr_pct, "rebalancear contra uma tendência de alta rende menos");

    // Curva mensal: início, fim de janeiro, de fevereiro e o dia 11/mar; CDI e IPCA encadeiam os meses publicados.
    assert_eq!(hold.months.first().unwrap().date, "2026-01-01");
    assert_eq!(hold.months.len(), 3, "início + fev + mar (janeiro é o mês do início)");
    let february = &hold.months[1];
    assert!(close(february.cdi_index.unwrap(), 1.01) && close(february.ipca_index.unwrap(), 1.005));
    assert!(hold.months[2].ipca_index.is_none(), "IPCA de março não existe");
    assert!(hold.months[2].ibov_index.is_none() && hold.benchmarks.iter().all(|x| x.code != "ibov"));
    assert!(hold.benchmarks.iter().any(|x| x.code == "cdi" && close(x.total_return_pct, 2.01)));
    assert!(hold.metrics.risk_free_pct > 0.0);

    // Pesos escolhidos: só o A entra, e os outros ativos com posição viram aviso.
    let only_a = run_backtest_in(&db, request(vec![WeightOverride { asset_id: a.id, weight: 1.0 }], "never")).await.unwrap();
    assert_eq!(only_a.assets.len(), 1);
    assert!(only_a.warnings.iter().any(|w| w.starts_with("BBBB3") && w.contains("no weight given")));

    // Custos: a corretagem tira do patrimônio e é devolvida nas métricas; o imposto liga com as regras do banco
    // (aqui as vendas do rebalanceamento são pequenas e ficam isentas, então não há DARF).
    let mut costly = request(vec![], "monthly");
    costly.brokerage_per_order = 1.0;
    costly.apply_tax = true;
    let costly = run_backtest_in(&db, costly).await.unwrap();
    assert!(costly.metrics.total_fees >= 2.0, "ao menos a compra inicial dos dois ativos: {}", costly.metrics.total_fees);
    assert!(costly.metrics.final_value < monthly.metrics.final_value && costly.metrics.twr_pct < monthly.metrics.twr_pct);
    assert!(close(costly.metrics.total_tax_paid, 0.0) && close(costly.metrics.tax_pending, 0.0), "{:?}", costly.metrics.total_tax_paid);
    assert!(close(monthly.metrics.total_fees, 0.0), "sem corretagem nada é cobrado");

    // Parâmetros inválidos.
    assert!(run_backtest_in(&db, request(vec![], "daily")).await.is_err());
    let mut negative = request(vec![], "never");
    negative.brokerage_per_order = -1.0;
    assert!(run_backtest_in(&db, negative).await.is_err());
    let mut broke = request(vec![], "never");
    broke.initial = 0.0;
    assert!(run_backtest_in(&db, broke).await.is_err());
}

#[tokio::test]
async fn the_backtest_refuses_a_portfolio_with_nothing_to_simulate() {
    use crate::commands::backtest::{run_backtest_in, BacktestRequest};

    let db = memory_db().await;
    let ws = workspace(&db, "Empty").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let request = BacktestRequest { portfolio_id: pf.id, weights: vec![], rebalance: "never".to_string(), initial: 1000.0, monthly_contribution: 0.0, start_date: None, brokerage_per_order: 0.0, apply_tax: false };
    let error = run_backtest_in(&db, request).await.err().expect("carteira vazia");
    assert!(error.to_string().contains("no asset to simulate"));
}

fn swap_request(portfolio_id: i32, from: i32, to: i32, date: &str, out: f64, into: f64, value: f64) -> crate::commands::transaction::CryptoSwapRequest {
    crate::commands::transaction::CryptoSwapRequest {
        portfolio_id,
        from_asset_id: from,
        to_asset_id: to,
        custodia_id: None,
        transaction_date: date.to_string(),
        quantity_out: out,
        quantity_in: into,
        value,
        fee: None,
        notes: None,
    }
}

#[tokio::test]
async fn a_crypto_swap_is_a_sale_of_what_goes_out_and_a_purchase_of_what_comes_in() {
    use crate::commands::transaction::record_crypto_swap_in;

    let db = memory_db().await;
    let ws = workspace(&db, "Swap").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let btc = asset(&db, "BTC", "cripto").await;
    let eth = asset(&db, "ETH", "cripto").await;
    trade(&db, pf.id, btc.id, "compra", 1.0, 10000.0, "2026-01-02").await;

    // Fev: 1 BTC (custo 10 mil) vira 20 ETH avaliados em 40 mil: venda acima dos 35 mil, ganho de 30 mil.
    let swap = record_crypto_swap_in(&db, swap_request(pf.id, btc.id, eth.id, "2026-02-10", 1.0, 20.0, 40000.0)).await.unwrap();
    assert_eq!((swap.sale.transaction_type.as_str(), swap.purchase.transaction_type.as_str()), ("venda", "compra"));
    assert_eq!((swap.sale.asset_id, swap.purchase.asset_id), (Some(btc.id), Some(eth.id)));
    assert!(close(swap.sale.total_value, 40000.0) && close(swap.purchase.total_value, 40000.0));
    assert!(close(swap.purchase.unit_price.unwrap(), 2000.0));
    assert_eq!(swap.sale.notes.as_deref(), Some("Swap BTC → ETH"));

    // O ETH entra com custo de 40 mil: vender os 20 em março por 50 mil dá ganho de 10 mil, e não de 50 mil.
    trade(&db, pf.id, eth.id, "venda", 20.0, 50000.0, "2026-03-10").await;
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert_eq!(assessment.crypto_months.len(), 2);
    let (february, march) = (&assessment.crypto_months[0], &assessment.crypto_months[1]);
    assert!(!february.exempt && close(february.gain, 30000.0) && close(february.due, 4500.0), "{} / {}", february.gain, february.due);
    assert!(close(march.gain, 10000.0) && close(march.due, 1500.0), "{} / {}", march.gain, march.due);
}

#[tokio::test]
async fn a_small_crypto_swap_is_exempt_and_its_fee_lowers_the_gain() {
    use crate::commands::transaction::record_crypto_swap_in;

    let db = memory_db().await;
    let ws = workspace(&db, "Swap").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let btc = asset(&db, "BTC", "cripto").await;
    let eth = asset(&db, "ETH", "cripto").await;
    trade(&db, pf.id, btc.id, "compra", 2.0, 20000.0, "2026-01-02").await;

    // Venda de 10 mil, abaixo dos 35 mil: isenta mesmo com ganho.
    record_crypto_swap_in(&db, swap_request(pf.id, btc.id, eth.id, "2026-02-10", 1.0, 5.0, 10000.0)).await.unwrap();
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert!(assessment.crypto_months[0].exempt && close(assessment.crypto_months[0].due, 0.0));

    // Com taxa de 100 num swap grande (50 mil, custo de 10 mil), a taxa sai do ganho: 40.000 − 100 = 39.900 em vez de 40.000.
    let mut big = swap_request(pf.id, btc.id, eth.id, "2026-03-10", 1.0, 25.0, 50000.0);
    big.fee = Some(100.0);
    record_crypto_swap_in(&db, big).await.unwrap();
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    let march = assessment.crypto_months.iter().find(|m| m.period == "2026-03").unwrap();
    assert!(close(march.gain, 39900.0), "{}", march.gain);
}

#[tokio::test]
async fn crypto_created_in_usd_is_assessed_in_reais_at_the_rate_of_each_day() {
    let db = memory_db().await;
    let ws = workspace(&db, "Dólar").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let btc = asset_in(&db, "BTC", "cripto", "USD").await;
    seed_price(&db, "USDBRL", "2026-01-02", 5.0).await;
    seed_price(&db, "USDBRL", "2026-02-10", 6.0).await;
    // Compra de US$ 10 mil a R$ 5 (R$ 50 mil) e venda de US$ 12 mil a R$ 6 (R$ 72 mil): ganho de R$ 22 mil, acima dos R$ 35 mil.
    trade(&db, pf.id, btc.id, "compra", 1.0, 10000.0, "2026-01-02").await;
    trade(&db, pf.id, btc.id, "venda", 1.0, 12000.0, "2026-02-10").await;

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    let february = &assessment.crypto_months[0];
    assert!(!february.exempt && close(february.gain, 22000.0) && close(february.due, 3300.0), "{} / {}", february.gain, february.due);
}

#[tokio::test]
async fn crypto_in_usd_uses_the_ptax_sell_quote_and_the_market_rate_only_where_it_is_missing() {
    let db = memory_db().await;
    let ws = workspace(&db, "Dólar PTAX").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let btc = asset_in(&db, "BTC", "cripto", "USD").await;
    // Mercado: R$ 5,50 em 02/01 e R$ 6,50 em 10/02. PTAX de venda: R$ 5,00 em 02/01 (compra 4,90) e R$ 6,00 em 10/02.
    seed_price(&db, "USDBRL", "2026-01-02", 5.5).await;
    seed_price(&db, "USDBRL", "2026-02-10", 6.5).await;
    seed_ptax(&db, "USD", "2026-01-02", 4.9, 5.0).await;
    seed_ptax(&db, "USD", "2026-02-10", 5.9, 6.0).await;
    // Compra de US$ 10 mil pela PTAX de venda de R$ 5 (R$ 50 mil) e venda de US$ 12 mil a R$ 6 (R$ 72 mil): ganho de R$ 22 mil
    // (pelo mercado seriam 78.000 − 55.000 = 23.000). A venda passa dos R$ 35 mil, então paga 15% de 22.000 = 3.300.
    // O caso sem PTAX nenhuma (só câmbio de mercado) é o do teste vizinho.
    trade(&db, pf.id, btc.id, "compra", 1.0, 10000.0, "2026-01-02").await;
    trade(&db, pf.id, btc.id, "venda", 1.0, 12000.0, "2026-02-10").await;

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    let february = &assessment.crypto_months[0];
    assert!(close(february.gain, 22000.0) && close(february.due, 3300.0), "{} / {}", february.gain, february.due);
}

#[tokio::test]
async fn usd_crypto_without_an_exchange_rate_still_only_warns() {
    let db = memory_db().await;
    let ws = workspace(&db, "Dólar").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let btc = asset_in(&db, "BTC", "cripto", "USD").await;
    trade(&db, pf.id, btc.id, "compra", 1.0, 10000.0, "2026-01-02").await;
    trade(&db, pf.id, btc.id, "venda", 1.0, 90000.0, "2026-02-10").await;

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert!(assessment.crypto_months.is_empty());
    assert!(assessment.warnings.iter().any(|w| matches!(w, crate::domain::tax::assessment::TaxWarning::CryptoSalesNonBrl { .. })));
}

#[tokio::test]
async fn foreign_assets_are_assessed_yearly_in_reais_with_the_foreign_withholding_as_credit() {
    let db = memory_db().await;
    let ws = workspace(&db, "Exterior").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let aapl = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    seed_price(&db, "USDBRL", "2025-01-10", 5.0).await;
    seed_price(&db, "USDBRL", "2025-06-10", 6.0).await;
    seed_price(&db, "USDBRL", "2025-09-10", 6.0).await;
    // Compra de US$ 1.000 a R$ 5 (custo R$ 5.000) e venda de US$ 1.200 a R$ 6 (R$ 7.200): ganho de R$ 2.200.
    trade(&db, pf.id, aapl.id, "compra", 10.0, 1000.0, "2025-01-10").await;
    trade(&db, pf.id, aapl.id, "venda", 10.0, 1200.0, "2025-06-10").await;
    // Provento de US$ 100 (R$ 600) com US$ 10 retidos lá fora (R$ 60): base 2.800, imposto 420, crédito 60.
    let mut dividend = tx_request(pf.id, aapl.id, "provento", 1.0, 100.0, "2025-09-10", 0.0);
    dividend.payment_type = Some("dividendo".to_string());
    dividend.foreign_tax_withheld = Some(10.0);
    add(&db, dividend).await;

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert_eq!(assessment.foreign_years.len(), 1);
    let y = &assessment.foreign_years[0];
    assert_eq!(y.year, 2025);
    assert!(close(y.capital_gain, 2200.0) && close(y.dividends, 600.0) && close(y.withheld, 60.0), "{y:?}");
    assert!(close(y.taxable_base, 2800.0) && close(y.tax_before_credit, 420.0), "{y:?}");
    assert!(close(y.credit_used, 60.0) && close(y.tax_due, 360.0), "{y:?}");
    assert!(
        !assessment.warnings.iter().any(|w| matches!(w, crate::domain::tax::assessment::TaxWarning::ForeignSales { .. })),
        "de 2024 em diante não há mais o aviso antigo"
    );
    // Sem PTAX gravada a conversão cai no câmbio de mercado, e o ano avisa que é aproximado.
    assert!(assessment.warnings.iter().any(
        |w| matches!(w, crate::domain::tax::assessment::TaxWarning::ForeignFxApproximate { year: 2025 })
    ));
    assert_eq!(y.sales[0].fx_source, crate::domain::ptax::RateSource::Market);
}

async fn seed_ptax(db: &sea_orm::DatabaseConnection, currency: &str, date: &str, buy: f64, sell: f64) {
    use sea_orm::{ActiveModelTrait, Set};
    crate::entity::ptax_daily::ActiveModel {
        currency: Set(currency.to_string()),
        reference_date: Set(date.to_string()),
        buy: Set(buy),
        sell: Set(sell),
        source: Set("test".to_string()),
        fetched_at: Set("2026-10-21T00:00:00Z".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert ptax");
}

#[tokio::test]
async fn the_stored_ptax_decides_the_foreign_assessment_and_the_withholding_uses_the_buy_quote() {
    let db = memory_db().await;
    let ws = workspace(&db, "PTAX").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let aapl = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    // O câmbio de mercado diz 5 e 6, mas a PTAX gravada manda: compra a 5,5 e venda a 6,5.
    seed_price(&db, "USDBRL", "2025-01-10", 5.0).await;
    seed_price(&db, "USDBRL", "2025-06-10", 6.0).await;
    seed_price(&db, "USDBRL", "2025-09-10", 6.0).await;
    seed_ptax(&db, "USD", "2025-01-10", 5.49, 5.5).await;
    seed_ptax(&db, "USD", "2025-06-10", 6.49, 6.5).await;
    seed_ptax(&db, "USD", "2025-09-10", 6.4, 6.5).await;
    trade(&db, pf.id, aapl.id, "compra", 10.0, 1000.0, "2025-01-10").await;
    trade(&db, pf.id, aapl.id, "venda", 10.0, 1200.0, "2025-06-10").await;
    let mut dividend = tx_request(pf.id, aapl.id, "provento", 1.0, 100.0, "2025-09-10", 0.0);
    dividend.payment_type = Some("dividendo".to_string());
    dividend.foreign_tax_withheld = Some(10.0);
    add(&db, dividend).await;

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    let y = &assessment.foreign_years[0];
    // custo 1.000 × 5,5 = 5.500; venda 1.200 × 6,5 = 7.800 → ganho 2.300; provento 650; retido 10 × 6,4 (compra) = 64
    assert!(close(y.capital_gain, 2300.0) && close(y.dividends, 650.0) && close(y.withheld, 64.0), "{y:?}");
    // base 2.950 × 15% = 442,50; crédito 64
    assert!(close(y.tax_before_credit, 442.5) && close(y.tax_due, 378.5), "{y:?}");
    assert_eq!(y.sales[0].fx_source, crate::domain::ptax::RateSource::Ptax);
    assert!(!assessment.warnings.iter().any(|w| matches!(
        w,
        crate::domain::tax::assessment::TaxWarning::ForeignFxApproximate { .. }
    )));
}

#[tokio::test]
async fn the_2023_update_of_a_foreign_asset_sets_its_cost_and_the_command_refuses_bad_values() {
    use crate::commands::asset::{set_asset_update_2023_in, SetAssetUpdate2023Request};
    use crate::commands::tax::foreign_holdings_in;
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Atualização 2023").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let aapl = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    seed_ptax(&db, "USD", "2023-05-10", 4.99, 5.0).await;
    // 31/12/2023 é domingo: vale a PTAX de venda do último dia útil, 29/12/2023
    seed_ptax(&db, "USD", "2023-12-29", 4.89, 4.9).await;
    seed_ptax(&db, "USD", "2024-06-10", 5.49, 5.5).await;
    trade(&db, pf.id, aapl.id, "compra", 10.0, 1000.0, "2023-05-10").await;
    trade(&db, pf.id, aapl.id, "venda", 10.0, 1200.0, "2024-06-10").await;
    let set = |value: Option<f64>| set_asset_update_2023_in(&db, SetAssetUpdate2023Request { asset_id: aapl.id, value });

    for bad in [Some(0.0), Some(-1.0), Some(f64::NAN)] {
        let result = set(bad).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))), "{result:?}");
    }
    let unknown = set_asset_update_2023_in(&db, SetAssetUpdate2023Request { asset_id: 9999, value: Some(1.0) }).await;
    assert!(matches!(unknown, Err(AppError::NotFound(_))), "{unknown:?}");

    // sem a atualização: custo 1.000 × 5,0 = 5.000; venda 1.200 × 5,5 = 6.600; ganho 1.600
    let y = &assess_workspace(&db, ws.id).await.unwrap().foreign_years[0];
    assert!(close(y.capital_gain, 1600.0), "{y:?}");

    // saldo de US$ 1.500 × 4,90 = R$ 7.350 vira o custo: ganho 6.600 − 7.350 = −750, no informe de 2023 e no de 2024
    assert_eq!(set(Some(1500.0)).await.unwrap().update_2023_value, Some(1500.0));
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert!(close(assessment.foreign_years[0].capital_gain, -750.0), "{:?}", assessment.foreign_years[0]);
    assert!(assessment.warnings.is_empty(), "{:?}", assessment.warnings);
    let rows = foreign_holdings_in(&db, ws.id).await.unwrap();
    let row = rows.iter().find(|r| r.holding.year == 2023).unwrap();
    assert!(close(row.holding.cost_brl, 7350.0) && row.update_2023_value == Some(1500.0), "{:?}", row.holding);

    // vazio apaga: volta ao custo das compras
    assert_eq!(set(None).await.unwrap().update_2023_value, None);
    let y = &assess_workspace(&db, ws.id).await.unwrap().foreign_years[0];
    assert!(close(y.capital_gain, 1600.0), "{y:?}");
}

#[tokio::test]
async fn the_foreign_holdings_report_has_the_cost_in_reais_per_asset_and_december() {
    use crate::commands::tax::{foreign_holdings_csv_text, foreign_holdings_in};

    let db = memory_db().await;
    let ws = workspace(&db, "Informe").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let aapl = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    let msft = asset_in(&db, "MSFT", "acao_internacional", "USD").await;
    // PTAX de venda: 5,5 em 2024 e 6,5 em 2025 (a de compra é irrelevante para o custo).
    seed_ptax(&db, "USD", "2024-03-15", 5.49, 5.5).await;
    seed_ptax(&db, "USD", "2025-03-14", 6.49, 6.5).await;
    // AAPL: compra em 2024 (US$ 1.000 → R$ 5.500) e de novo em 2025 (US$ 1.000 → R$ 6.500). MSFT: comprada e vendida em 2024.
    trade(&db, pf.id, aapl.id, "compra", 10.0, 1000.0, "2024-03-15").await;
    trade(&db, pf.id, aapl.id, "compra", 10.0, 1000.0, "2025-03-14").await;
    trade(&db, pf.id, msft.id, "compra", 5.0, 500.0, "2024-03-15").await;
    trade(&db, pf.id, msft.id, "venda", 5.0, 600.0, "2024-03-15").await;

    let rows = foreign_holdings_in(&db, ws.id).await.unwrap();
    let find = |year: i32, ticker: &str| rows.iter().find(|r| r.holding.year == year && r.holding.ticker == ticker);
    let y24 = find(2024, "AAPL").unwrap();
    assert!(close(y24.holding.quantity, 10.0) && close(y24.holding.cost_brl, 5500.0), "{:?}", y24.holding);
    let y25 = find(2025, "AAPL").unwrap();
    assert!(close(y25.holding.quantity, 20.0) && close(y25.holding.cost_brl, 12000.0), "{:?}", y25.holding);
    assert!(!y25.holding.incomplete && !y25.holding.fx_approximate);
    assert_eq!(y25.name, "AAPL");
    assert!(find(2024, "MSFT").is_none(), "vendida por inteiro no mesmo ano");

    let csv = foreign_holdings_csv_text(&rows).unwrap();
    let mut lines = csv.lines();
    assert_eq!(
        lines.next().unwrap(),
        "year,ticker,name,exchange,custodia,currency,quantity,cost_brl,fx_approximate,incomplete,dirpf_group,dirpf_code,dirpf_country"
    );
    // ativo sem grupo, código e país informados: as três colunas novas saem vazias
    assert!(csv.contains("2024,AAPL,AAPL,,,USD,10,5500.00,false,false,,,"), "{csv}");
}

#[tokio::test]
async fn the_group_code_and_country_of_the_ficha_come_from_the_asset_and_reach_the_holdings_csv() {
    use crate::commands::asset::{update_asset_dirpf_in, UpdateAssetDirpfRequest};
    use crate::commands::tax::{foreign_holdings_csv_text, foreign_holdings_in};
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Ficha").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let filled = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    let blank = asset_in(&db, "MSFT", "acao_internacional", "USD").await;
    // compra de 10 cotas por US$ 1.000 em 15/03/2024, PTAX de venda 5,5: custo de R$ 5.500,00 nos dois ativos
    seed_ptax(&db, "USD", "2024-03-15", 5.49, 5.5).await;
    for asset in [&filled, &blank] {
        trade(&db, pf.id, asset.id, "compra", 10.0, 1000.0, "2024-03-15").await;
    }
    let request = |group: &str, code: &str, country: &str| UpdateAssetDirpfRequest {
        asset_id: filled.id,
        group: Some(group.to_string()),
        code: Some(code.to_string()),
        country: Some(country.to_string()),
    };

    // valor inválido é recusado, com o nome do campo, e nada é gravado
    let bad = update_asset_dirpf_in(&db, request("3", "ab", "249")).await;
    assert!(matches!(&bad, Err(AppError::InvalidInput(m)) if m.contains("code")), "{bad:?}");
    let rows = foreign_holdings_in(&db, ws.id).await.unwrap();
    assert!(rows.iter().all(|r| r.dirpf_group.is_none()), "nada gravado");

    // "3" e "1" completam para "03" e "01"; "249" fica
    let saved = update_asset_dirpf_in(&db, request("3", "1", "249")).await.unwrap();
    assert_eq!(
        (saved.dirpf_group.as_deref(), saved.dirpf_code.as_deref(), saved.dirpf_country.as_deref()),
        (Some("03"), Some("01"), Some("249"))
    );
    let rows = foreign_holdings_in(&db, ws.id).await.unwrap();
    let csv = foreign_holdings_csv_text(&rows).unwrap();
    assert!(csv.contains("2024,AAPL,AAPL,,,USD,10,5500.00,false,false,03,01,249"), "{csv}");
    assert!(csv.contains("2024,MSFT,MSFT,,,USD,10,5500.00,false,false,,,"), "{csv}");

    // vazio apaga os três campos
    let cleared = UpdateAssetDirpfRequest { asset_id: filled.id, group: None, code: Some("  ".to_string()), country: None };
    let cleared = update_asset_dirpf_in(&db, cleared).await.unwrap();
    assert!(cleared.dirpf_group.is_none() && cleared.dirpf_code.is_none() && cleared.dirpf_country.is_none());
}

#[tokio::test]
async fn the_foreign_holdings_report_splits_the_position_by_custody_after_a_transfer() {
    use crate::commands::tax::{foreign_holdings_csv_text, foreign_holdings_in};
    use sea_orm::{ActiveModelTrait, Set};

    let db = memory_db().await;
    let ws = workspace(&db, "Custódias").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let aapl = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    let mut custodias = Vec::new();
    for name in ["Avenue", "Nomad"] {
        let row = crate::entity::custodia::ActiveModel {
            workspace_id: Set(ws.id),
            instituicao: Set(name.to_string()),
            titular: Set("Eu".to_string()),
            created_at: Set("2024-01-01T00:00:00Z".to_string()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        custodias.push(row);
    }
    seed_ptax(&db, "USD", "2024-03-15", 5.49, 5.5).await;

    // US$ 1.000 a R$ 5,50 = 5.500 na Avenue; em 2025 transfere 4 das 10 ações para a Nomad.
    let mut buy = tx_request(pf.id, aapl.id, "compra", 10.0, 100.0, "2024-03-15", 0.0);
    buy.custodia_id = Some(custodias[0].id);
    add(&db, buy).await;
    let mut transfer = tx_request(pf.id, aapl.id, "transferencia", 4.0, 0.0, "2025-02-10", 0.0);
    transfer.custodia_id = Some(custodias[0].id);
    transfer.transfer_to_custodia_id = Some(custodias[1].id);
    add(&db, transfer).await;

    let rows = foreign_holdings_in(&db, ws.id).await.unwrap();
    let find = |year: i32, custodia: i32| rows.iter().find(|r| r.holding.year == year && r.holding.custodia_id == Some(custodia));
    let y24 = find(2024, custodias[0].id).unwrap();
    assert!(close(y24.holding.quantity, 10.0) && close(y24.holding.cost_brl, 5500.0), "{:?}", y24.holding);
    assert_eq!(y24.custodia_label.as_deref(), Some("Avenue (Eu)"));
    assert!(find(2024, custodias[1].id).is_none());
    let (avenue, nomad) = (find(2025, custodias[0].id).unwrap(), find(2025, custodias[1].id).unwrap());
    assert!(close(avenue.holding.quantity, 6.0) && close(avenue.holding.cost_brl, 3300.0), "{:?}", avenue.holding);
    assert!(close(nomad.holding.quantity, 4.0) && close(nomad.holding.cost_brl, 2200.0), "{:?}", nomad.holding);
    assert_eq!(nomad.custodia_label.as_deref(), Some("Nomad (Eu)"));

    let csv = foreign_holdings_csv_text(&rows).unwrap();
    assert!(csv.contains("2025,AAPL,AAPL,,Nomad (Eu),USD,4,2200.00,false,false,,,"), "{csv}");
}

#[test]
fn the_holdings_csv_quotes_names_with_commas_and_keeps_two_decimals() {
    use crate::commands::tax::{foreign_holdings_csv_text, ForeignHoldingView};
    use crate::domain::tax::foreign::ForeignHolding;

    let rows = vec![ForeignHoldingView {
        holding: ForeignHolding {
            year: 2025,
            asset_id: 1,
            custodia_id: Some(3),
            ticker: "BRK.B".to_string(),
            currency: "USD".to_string(),
            quantity: 2.5,
            cost_brl: 1234.5,
            fx_approximate: true,
            incomplete: false,
        },
        name: "Berkshire Hathaway, Inc. \"B\"".to_string(),
        exchange: Some("NYSE".to_string()),
        custodia_label: Some("Avenue (Eu)".to_string()),
        dirpf_group: Some("03".to_string()),
        dirpf_code: Some("01".to_string()),
        dirpf_country: Some("249".to_string()),
        update_2023_value: None,
    }];
    let csv = foreign_holdings_csv_text(&rows).unwrap();
    assert!(
        csv.contains("2025,BRK.B,\"Berkshire Hathaway, Inc. \"\"B\"\"\",NYSE,Avenue (Eu),USD,2.5,1234.50,true,false,03,01,249"),
        "{csv}"
    );
}

#[tokio::test]
async fn an_offshore_entity_is_taxed_on_the_ptax_of_31_12_and_an_open_year_waits() {
    use crate::commands::offshore::{
        create_offshore_entity_in, list_offshore_entities_in, update_offshore_entity_in, upsert_offshore_result_in,
        CreateOffshoreEntityRequest, OffshoreEntityRequest, UpdateOffshoreEntityRequest, UpsertOffshoreResultRequest,
    };
    use crate::domain::tax::assessment::TaxWarning;
    use chrono::{Datelike, Utc};

    let db = memory_db().await;
    let ws = workspace(&db, "Offshore").await;
    seed_ptax(&db, "USD", "2024-12-31", 4.9, 5.0).await;
    let entity_request = |currency: &str, percent: f64| OffshoreEntityRequest {
        name: " Holding LLC ".to_string(),
        country: "US".to_string(),
        currency: currency.to_string(),
        ownership_percent: percent,
        regime: None,
        notes: None,
    };

    let entity = create_offshore_entity_in(
        &db,
        CreateOffshoreEntityRequest { workspace_id: ws.id, entity: entity_request("usd", 40.0) },
    )
    .await
    .unwrap();
    assert_eq!(entity.name, "Holding LLC", "o nome é aparado");
    assert_eq!(entity.currency, "USD", "a moeda vai em maiúsculas");

    let result = |year: i32, profit: f64, tax: f64| UpsertOffshoreResultRequest {
        entity_id: entity.id,
        year,
        profit,
        tax_paid_abroad: tax,
    };
    upsert_offshore_result_in(&db, result(2024, 500.0, 0.0)).await.unwrap();
    // o mesmo ano de novo corrige em vez de duplicar
    upsert_offshore_result_in(&db, result(2024, 1_000.0, 100.0)).await.unwrap();
    // o ano corrente ainda não fechou
    upsert_offshore_result_in(&db, result(Utc::now().year(), 800.0, 0.0)).await.unwrap();

    let views = list_offshore_entities_in(&db, ws.id).await.unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].results.len(), 2, "{:?}", views[0].results);

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert_eq!(assessment.offshore_years.len(), 1);
    let y = &assessment.offshore_years[0];
    // 40% de US$ 1.000 × 5,0 = 2.000; imposto 300; crédito 100 × 40% × 4,9 (compra) = 196; devido 104
    assert_eq!(y.year, 2024);
    assert!(close(y.profit_share_brl, 2000.0) && close(y.tax_before_credit, 300.0), "{y:?}");
    assert!(close(y.credit_used, 196.0) && close(y.tax_due, 104.0), "{y:?}");
    assert_eq!(y.lines[0].fx_source, crate::domain::ptax::RateSource::Ptax);
    assert!(assessment
        .warnings
        .iter()
        .any(|w| matches!(w, TaxWarning::OffshoreYearOpen { year } if *year == Utc::now().year())));

    // trocar a participação muda a apuração
    update_offshore_entity_in(
        &db,
        UpdateOffshoreEntityRequest { id: entity.id, entity: entity_request("USD", 100.0) },
    )
    .await
    .unwrap();
    let y = &assess_workspace(&db, ws.id).await.unwrap().offshore_years[0];
    assert!(close(y.profit_share_brl, 5000.0) && close(y.tax_before_credit, 750.0), "{y:?}");
}

#[tokio::test]
async fn offshore_distributions_reduce_the_dividend_credit_and_the_commands_refuse_bad_input() {
    use crate::commands::offshore::{
        add_offshore_distribution_in, create_offshore_entity_in, offshore_credits_in, upsert_offshore_result_in,
        AddOffshoreDistributionRequest, CreateOffshoreEntityRequest, OffshoreEntityRequest, UpsertOffshoreResultRequest,
    };
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Distribuições").await;
    seed_ptax(&db, "USD", "2025-12-31", 5.9, 6.0).await;
    let entity = create_offshore_entity_in(
        &db,
        CreateOffshoreEntityRequest {
            workspace_id: ws.id,
            entity: OffshoreEntityRequest {
                name: "Holding LLC".to_string(),
                country: "US".to_string(),
                currency: "USD".to_string(),
                ownership_percent: 50.0,
                regime: None,
                notes: None,
            },
        },
    )
    .await
    .unwrap();
    // 50% de US$ 2.000 × 6,00 (PTAX de venda de 31/12/2025) = R$ 6.000 de lucro tributado
    upsert_offshore_result_in(
        &db,
        UpsertOffshoreResultRequest { entity_id: entity.id, year: 2025, profit: 2000.0, tax_paid_abroad: 0.0 },
    )
    .await
    .unwrap();
    let request = |year: i32, received_on: &str, amount: f64, entity_id: i32| AddOffshoreDistributionRequest {
        entity_id,
        origin_year: year,
        received_on: received_on.to_string(),
        amount,
    };

    // recusas: antes de 31/12 do ano de origem, valor zero ou negativo, data inválida, ano de origem anterior a 2023 (2023 vale
    // desde a Sessão 233, lucro de 2023 ou antes: art. 6º I), entidade inexistente
    for bad in [
        request(2025, "2025-12-30", 100.0, entity.id),
        request(2025, "2026-03-10", 0.0, entity.id),
        request(2025, "2026-03-10", -5.0, entity.id),
        request(2025, "10/03/2026", 100.0, entity.id),
        request(2022, "2026-03-10", 100.0, entity.id),
    ] {
        let result = add_offshore_distribution_in(&db, bad).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))), "{result:?}");
    }
    let unknown = add_offshore_distribution_in(&db, request(2025, "2026-03-10", 100.0, 9999)).await;
    assert!(matches!(unknown, Err(AppError::NotFound(_))), "{unknown:?}");

    // US$ 500 recebidos em 10/03/2026 reduzem 500 × 6,00 = R$ 3.000; no próprio 31/12/2025 pode-se receber (31/12 vale)
    add_offshore_distribution_in(&db, request(2025, "2026-03-10", 500.0, entity.id)).await.unwrap();
    let credits = offshore_credits_in(&db, ws.id).await.unwrap();
    let balance = |as_of: i32| {
        credits.rows.iter().find(|r| r.origin_year == 2025 && r.as_of_year == as_of).map(|r| r.balance_brl)
    };
    assert!(close(balance(2025).unwrap(), 6000.0) && close(balance(2026).unwrap(), 3000.0), "{:?}", credits.rows);
    assert_eq!(credits.distributions.len(), 1);
    assert!(close(credits.distributions[0].reduction_brl.unwrap(), 3000.0));
}

#[tokio::test]
async fn an_active_offshore_entity_is_taxed_on_the_distributions_it_pays_and_not_on_31_12() {
    use crate::commands::offshore::{
        add_offshore_distribution_in, create_offshore_entity_in, offshore_credits_in, upsert_offshore_result_in,
        AddOffshoreDistributionRequest, CreateOffshoreEntityRequest, OffshoreEntityRequest, UpsertOffshoreResultRequest,
    };
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Ativa").await;
    seed_ptax(&db, "USD", "2024-12-31", 4.9, 5.0).await;
    seed_ptax(&db, "USD", "2025-03-10", 5.4, 5.5).await;
    let create = |regime: Option<&str>| {
        create_offshore_entity_in(
            &db,
            CreateOffshoreEntityRequest {
                workspace_id: ws.id,
                entity: OffshoreEntityRequest {
                    name: "Operadora LLC".to_string(),
                    country: "US".to_string(),
                    currency: "USD".to_string(),
                    ownership_percent: 100.0,
                    regime: regime.map(str::to_string),
                    notes: None,
                },
            },
        )
    };
    assert!(matches!(create(Some("passiva")).await, Err(AppError::InvalidInput(_))), "regime desconhecido");
    let entity = create(Some("ativa")).await.unwrap();
    assert_eq!(entity.regime, "ativa");

    // lucro de US$ 1.000 em 2024: uma entidade ativa não é tributada em 31/12
    upsert_offshore_result_in(
        &db,
        UpsertOffshoreResultRequest { entity_id: entity.id, year: 2024, profit: 1000.0, tax_paid_abroad: 0.0 },
    )
    .await
    .unwrap();
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert!(assessment.offshore_years.is_empty() && assessment.offshore_active_years.is_empty());

    let request = |received_on: &str, amount: f64| AddOffshoreDistributionRequest {
        entity_id: entity.id,
        origin_year: 0,
        received_on: received_on.to_string(),
        amount,
    };
    // antes de 2024 a lei não vale; valor e data inválidos também são recusados
    for bad in [request("2023-12-20", 100.0), request("2025-03-10", 0.0), request("10/03/2025", 100.0)] {
        let result = add_offshore_distribution_in(&db, bad).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))), "{result:?}");
    }
    // sem ano de origem; o ano do recebimento vale. US$ 1.000 × 5,50 (PTAX de venda de 10/03/2025) = R$ 5.500; imposto 825
    let saved = add_offshore_distribution_in(&db, request("2025-03-10", 1000.0)).await.unwrap();
    assert_eq!(saved.origin_year, 2025);
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert!(assessment.offshore_years.is_empty());
    let year = &assessment.offshore_active_years[0];
    assert_eq!(year.year, 2025);
    assert!(close(year.taxable_base, 5500.0) && close(year.tax_due, 825.0), "{year:?}");

    // não vira crédito de dividendo a receber
    let credits = offshore_credits_in(&db, ws.id).await.unwrap();
    assert!(credits.rows.is_empty() && credits.distributions.is_empty(), "{:?}", credits.rows);
}

#[tokio::test]
async fn the_offshore_commands_refuse_invalid_input() {
    use crate::commands::offshore::{
        create_offshore_entity_in, upsert_offshore_result_in, CreateOffshoreEntityRequest, OffshoreEntityRequest,
        UpsertOffshoreResultRequest,
    };
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Offshore").await;
    let create = |name: &str, currency: &str, percent: f64| {
        create_offshore_entity_in(
            &db,
            CreateOffshoreEntityRequest {
                workspace_id: ws.id,
                entity: OffshoreEntityRequest {
                    name: name.to_string(),
                    country: "US".to_string(),
                    currency: currency.to_string(),
                    ownership_percent: percent,
                    regime: None,
                    notes: None,
                },
            },
        )
    };
    assert!(matches!(create("  ", "USD", 50.0).await, Err(AppError::InvalidInput(_))));
    assert!(matches!(create("A", "US", 50.0).await, Err(AppError::InvalidInput(_))), "moeda de 3 letras");
    assert!(matches!(create("A", "U5D", 50.0).await, Err(AppError::InvalidInput(_))));
    assert!(matches!(create("A", "USD", 0.0).await, Err(AppError::InvalidInput(_))), "participação acima de zero");
    assert!(matches!(create("A", "USD", 100.5).await, Err(AppError::InvalidInput(_))), "participação até 100");
    let entity = create("A", "USD", 100.0).await.unwrap();

    let result = |entity_id: i32, year: i32, profit: f64, tax: f64| {
        upsert_offshore_result_in(&db, UpsertOffshoreResultRequest { entity_id, year, profit, tax_paid_abroad: tax })
    };
    assert!(matches!(result(entity.id, 2023, 1.0, 0.0).await, Err(AppError::InvalidInput(_))), "só de 2024 em diante");
    assert!(matches!(result(entity.id, 2025, f64::NAN, 0.0).await, Err(AppError::InvalidInput(_))));
    assert!(matches!(result(entity.id, 2025, 1.0, -1.0).await, Err(AppError::InvalidInput(_))));
    assert!(matches!(result(9999, 2025, 1.0, 0.0).await, Err(AppError::NotFound(_))));
    assert!(result(entity.id, 2025, -50.0, 0.0).await.is_ok(), "prejuízo é válido");
}

#[tokio::test]
async fn the_2023_update_of_an_offshore_entity_gives_a_dividend_credit_that_distributions_reduce() {
    use crate::commands::offshore::{
        add_offshore_distribution_in, create_offshore_entity_in, offshore_credits_in, set_offshore_update_2023_in,
        AddOffshoreDistributionRequest, CreateOffshoreEntityRequest, OffshoreEntityRequest, SetOffshoreUpdate2023Request,
    };
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Atualização controlada").await;
    // 31/12/2023 é domingo: vale a PTAX de venda do último dia útil, 29/12/2023
    seed_ptax(&db, "USD", "2023-12-29", 4.79, 4.8).await;
    let entity = create_offshore_entity_in(
        &db,
        CreateOffshoreEntityRequest {
            workspace_id: ws.id,
            entity: OffshoreEntityRequest {
                name: "Holding LLC".to_string(),
                country: "US".to_string(),
                currency: "USD".to_string(),
                ownership_percent: 50.0,
                regime: None,
                notes: None,
            },
        },
    )
    .await
    .unwrap();
    let set = |equity: Option<f64>, profits: Option<f64>| {
        set_offshore_update_2023_in(&db, SetOffshoreUpdate2023Request { entity_id: entity.id, equity, profits })
    };
    let distribute = |origin_year: i32, received_on: &str, amount: f64| {
        add_offshore_distribution_in(
            &db,
            AddOffshoreDistributionRequest { entity_id: entity.id, origin_year, received_on: received_on.to_string(), amount },
        )
    };

    // valores inválidos e entidade desconhecida
    for (equity, profits) in [
        (Some(0.0), None),
        (Some(-1.0), None),
        (Some(f64::NAN), None),
        (Some(100.0), Some(-1.0)),
        (Some(100.0), Some(f64::NAN)),
        (Some(100.0), Some(200.0)),
    ] {
        let result = set(equity, profits).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))), "{equity:?} {profits:?}: {result:?}");
    }
    let unknown =
        set_offshore_update_2023_in(&db, SetOffshoreUpdate2023Request { entity_id: 9999, equity: Some(1.0), profits: None }).await;
    assert!(matches!(unknown, Err(AppError::NotFound(_))), "{unknown:?}");

    // atualização sem parcela de lucros: não há crédito (e a distribuição de 2023 seria tributada, art. 6º I; outro teste)
    set(Some(10_000.0), None).await.unwrap();

    // PL de US$ 10.000 × 4,80 = R$ 48.000; lucros de US$ 2.000 × 4,80 = R$ 9.600; custo adicional R$ 38.400
    let saved = set(Some(10_000.0), Some(2_000.0)).await.unwrap();
    assert_eq!((saved.update_2023_equity, saved.update_2023_profits), (Some(10_000.0), Some(2_000.0)));
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert!(assessment.warnings.is_empty(), "{:?}", assessment.warnings);
    let row = &assessment.offshore_updates_2023[0];
    assert!(close(row.equity_brl, 48_000.0) && close(row.profits_brl, 9_600.0) && close(row.additional_cost_brl, 38_400.0), "{row:?}");

    // a distribuição de lucro de 2023 só vale a partir de 31/12/2023; US$ 500 × 4,80 = R$ 2.400 reduzem o crédito
    let early = distribute(2023, "2023-12-20", 500.0).await;
    assert!(matches!(early, Err(AppError::InvalidInput(_))), "{early:?}");
    assert_eq!(distribute(2023, "2024-03-10", 500.0).await.unwrap().origin_year, 2023);
    let credits = offshore_credits_in(&db, ws.id).await.unwrap();
    let credit = credits.rows.iter().find(|r| r.origin_year == 2023 && r.as_of_year == 2024).unwrap();
    assert!(close(credit.taxed_brl, 9_600.0) && close(credit.distributed_brl, 2_400.0) && close(credit.balance_brl, 7_200.0), "{credit:?}");
    assert!(close(credits.distributions[0].reduction_brl.unwrap(), 2_400.0));
    assert_eq!(credits.updates.len(), 1);

    // apagar tira a atualização e o crédito (a distribuição fica, sem o que reduzir)
    let cleared = set(None, Some(5.0)).await.unwrap();
    assert_eq!((cleared.update_2023_equity, cleared.update_2023_profits), (None, None));
    let credits = offshore_credits_in(&db, ws.id).await.unwrap();
    assert!(credits.rows.is_empty() && credits.updates.is_empty(), "{:?}", credits.rows);
    assert_eq!(credits.distributions[0].reduction_brl, None);
}

#[tokio::test]
async fn the_2023_profits_of_an_ordinary_entity_are_taxed_on_receipt_and_the_credit_covers_part() {
    use crate::commands::offshore::{
        add_offshore_distribution_in, create_offshore_entity_in, offshore_credits_in, set_offshore_update_2023_in,
        AddOffshoreDistributionRequest, CreateOffshoreEntityRequest, OffshoreEntityRequest, SetOffshoreUpdate2023Request,
    };
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Comum com lucro de 2023").await;
    seed_ptax(&db, "USD", "2023-12-29", 4.79, 4.8).await;
    seed_ptax(&db, "USD", "2024-06-10", 4.9, 5.0).await;
    let entity = create_offshore_entity_in(
        &db,
        CreateOffshoreEntityRequest {
            workspace_id: ws.id,
            entity: OffshoreEntityRequest {
                name: "Holding LLC".to_string(),
                country: "US".to_string(),
                currency: "USD".to_string(),
                ownership_percent: 100.0,
                regime: None,
                notes: None,
            },
        },
    )
    .await
    .unwrap();
    let distribute = |origin_year: i32, received_on: &str, amount: f64| {
        add_offshore_distribution_in(
            &db,
            AddOffshoreDistributionRequest { entity_id: entity.id, origin_year, received_on: received_on.to_string(), amount },
        )
    };

    // a lei vale para recebimento a partir de 2024; ano de origem entre 2024 e 2100 ou 2023 (lucro de 2023 ou antes)
    let early = distribute(2023, "2023-12-20", 100.0).await;
    assert!(matches!(early, Err(AppError::InvalidInput(_))), "{early:?}");
    let old = distribute(2022, "2024-06-10", 100.0).await;
    assert!(matches!(old, Err(AppError::InvalidInput(_))), "{old:?}");

    // sem atualização: US$ 1.000 × 5,00 = R$ 5.000, imposto R$ 750 (art. 6º I)
    assert_eq!(distribute(2023, "2024-06-10", 1_000.0).await.unwrap().origin_year, 2023);
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    let year = &assessment.offshore_active_years[0];
    assert!(close(year.taxable_base, 5_000.0) && close(year.tax_due, 750.0), "{year:?}");

    // com a atualização (lucros de US$ 400): cobertos 400, tributáveis 600 × 5,00 = R$ 3.000, imposto R$ 450
    set_offshore_update_2023_in(&db, SetOffshoreUpdate2023Request { entity_id: entity.id, equity: Some(10_000.0), profits: Some(400.0) })
        .await
        .unwrap();
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    let year = &assessment.offshore_active_years[0];
    assert!(close(year.taxable_base, 3_000.0) && close(year.tax_due, 450.0), "{year:?}");
    assert!(close(year.lines[0].covered_amount, 400.0), "{year:?}");

    // o crédito de 2023 segue reduzido pela mesma distribuição (1.000 × 4,80 = 4.800 sobre 400 × 4,80 = 1.920: saldo zero)
    let credits = offshore_credits_in(&db, ws.id).await.unwrap();
    let credit = credits.rows.iter().find(|r| r.origin_year == 2023 && r.as_of_year == 2024).unwrap();
    assert!(close(credit.taxed_brl, 1_920.0) && close(credit.balance_brl, 0.0) && close(credit.over_distributed_brl, 2_880.0), "{credit:?}");

    // lucro de 2024 da entidade comum (já tributado em 31/12) não gera imposto na distribuição
    assert_eq!(distribute(2024, "2024-12-31", 500.0).await.unwrap().origin_year, 2024);
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert!(close(assessment.offshore_active_years[0].tax_due, 450.0), "{:?}", assessment.offshore_active_years);
}

#[tokio::test]
async fn selling_a_stake_in_a_controlled_entity_taxes_the_gain_over_the_cost_of_the_capital_applied() {
    use crate::commands::offshore::{
        add_offshore_capital_event_in, create_offshore_entity_in, list_offshore_entities_in, AddOffshoreCapitalEventRequest,
        CreateOffshoreEntityRequest, OffshoreEntityRequest,
    };
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Venda de participação").await;
    seed_ptax(&db, "USD", "2024-03-01", 4.9, 5.0).await;
    seed_ptax(&db, "USD", "2025-02-10", 5.9, 6.0).await;
    let entity = create_offshore_entity_in(
        &db,
        CreateOffshoreEntityRequest {
            workspace_id: ws.id,
            entity: OffshoreEntityRequest {
                name: "Holding LLC".to_string(),
                country: "US".to_string(),
                currency: "USD".to_string(),
                ownership_percent: 100.0,
                regime: None,
                notes: None,
            },
        },
    )
    .await
    .unwrap();
    let add = |entity_id: i32, kind: &str, date: &str, amount: f64, brl_cost: Option<f64>, fraction: Option<f64>| {
        add_offshore_capital_event_in(
            &db,
            AddOffshoreCapitalEventRequest {
                entity_id,
                kind: kind.to_string(),
                event_date: date.to_string(),
                amount,
                brl_cost,
                fraction,
            },
        )
    };

    // recusas: entidade inexistente, tipo, valor, data, venda sem fração ou com fração fora de (0, 1], custo em reais inválido,
    // e venda ou devolução anterior a 2024 (a lei)
    let unknown = add(9999, "aporte", "2024-03-01", 100.0, None, None).await;
    assert!(matches!(unknown, Err(AppError::NotFound(_))), "{unknown:?}");
    for (kind, date, amount, cost, fraction) in [
        ("compra", "2024-03-01", 100.0, None, None),
        ("aporte", "2024-03-01", 0.0, None, None),
        ("aporte", "2024-03-01", f64::NAN, None, None),
        ("aporte", "01/03/2024", 100.0, None, None),
        ("aporte", "2024-03-01", 100.0, Some(-1.0), None),
        ("venda", "2025-02-10", 100.0, None, None),
        ("venda", "2025-02-10", 100.0, None, Some(0.0)),
        ("venda", "2025-02-10", 100.0, None, Some(1.5)),
        ("venda", "2023-12-01", 100.0, None, Some(0.5)),
        ("devolucao", "2023-12-01", 100.0, None, None),
    ] {
        let result = add(entity.id, kind, date, amount, cost, fraction).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))), "{kind} {date} {amount}: {result:?}");
    }

    // aporte de US$ 1.000 a 5,00 (custo R$ 5.000) e venda de 50% por US$ 800 a 6,00: recebido R$ 4.800, custo vendido
    // R$ 2.500, ganho R$ 2.300, imposto de 15% = R$ 345. Os campos que não são do tipo são descartados.
    let contribution = add(entity.id, "aporte", "2024-03-01", 1_000.0, None, Some(0.5)).await.unwrap();
    assert_eq!(contribution.fraction, None, "fração é só da venda");
    let sale = add(entity.id, "venda", "2025-02-10", 800.0, Some(99.0), Some(0.5)).await.unwrap();
    assert_eq!((sale.brl_cost, sale.fraction), (None, Some(0.5)), "custo em reais é só do aporte");

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert!(assessment.warnings.is_empty(), "{:?}", assessment.warnings);
    let month = &assessment.offshore_capital_months[0];
    assert_eq!(month.period, "2025-02");
    assert!(close(month.gain, 2_300.0) && close(month.tax, 345.0) && close(month.due, 345.0), "{month:?}");
    assert_eq!(month.darf_code, 4600);
    let position = &assessment.offshore_capital_positions[0];
    assert!(close(position.capital, 500.0) && close(position.cost_brl, 2_500.0), "{position:?}");

    // a lista da entidade traz os dois eventos, do mais antigo ao mais novo
    let views = list_offshore_entities_in(&db, ws.id).await.unwrap();
    let kinds: Vec<&str> = views[0].capital_events.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds, vec!["aporte", "venda"]);
}

#[tokio::test]
async fn the_2023_credit_covers_the_distributions_of_an_active_entity_marked_as_2023_profits() {
    use crate::commands::offshore::{
        add_offshore_distribution_in, create_offshore_entity_in, offshore_credits_in, set_offshore_update_2023_in,
        AddOffshoreDistributionRequest, CreateOffshoreEntityRequest, OffshoreEntityRequest, SetOffshoreUpdate2023Request,
    };
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Ativa com atualização").await;
    seed_ptax(&db, "USD", "2023-12-29", 4.79, 4.8).await;
    seed_ptax(&db, "USD", "2024-06-10", 4.9, 5.0).await;
    let entity = create_offshore_entity_in(
        &db,
        CreateOffshoreEntityRequest {
            workspace_id: ws.id,
            entity: OffshoreEntityRequest {
                name: "Operadora LLC".to_string(),
                country: "US".to_string(),
                currency: "USD".to_string(),
                ownership_percent: 100.0,
                regime: Some("ativa".to_string()),
                notes: None,
            },
        },
    )
    .await
    .unwrap();
    let set = |equity: Option<f64>, profits: Option<f64>| {
        set_offshore_update_2023_in(&db, SetOffshoreUpdate2023Request { entity_id: entity.id, equity, profits })
    };
    let distribute = |origin_year: i32, amount: f64| {
        add_offshore_distribution_in(
            &db,
            AddOffshoreDistributionRequest { entity_id: entity.id, origin_year, received_on: "2024-06-10".to_string(), amount },
        )
    };

    // marcar "lucro de 2023" exige a atualização com parcela de lucros
    let refused = distribute(2023, 100.0).await;
    assert!(matches!(refused, Err(AppError::InvalidInput(_))), "sem atualização: {refused:?}");
    set(Some(10_000.0), None).await.unwrap();
    let refused = distribute(2023, 100.0).await;
    assert!(matches!(refused, Err(AppError::InvalidInput(_))), "sem parcela de lucros: {refused:?}");

    // crédito de US$ 2.000. Marcada de US$ 2.500: cobertos 2.000, tributáveis 500 × 5,00 = R$ 2.500. Desmarcada de US$ 100
    // (grava o ano do recebimento): R$ 500. Base R$ 3.000, imposto R$ 450
    set(Some(10_000.0), Some(2_000.0)).await.unwrap();
    assert_eq!(distribute(2023, 2_500.0).await.unwrap().origin_year, 2023);
    assert_eq!(distribute(0, 100.0).await.unwrap().origin_year, 2024);
    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    assert!(assessment.warnings.is_empty(), "{:?}", assessment.warnings);
    let year = &assessment.offshore_active_years[0];
    assert_eq!(year.year, 2024);
    assert!(close(year.taxable_base, 3_000.0) && close(year.tax_due, 450.0), "{year:?}");
    assert!(close(year.lines[0].covered_amount, 2_000.0) && close(year.lines[1].covered_amount, 0.0), "{year:?}");

    // a ativa tem o crédito de 2023 na ficha; só a distribuição marcada o reduz (2.500 × 4,80 = 12.000 > 9.600: saldo zero)
    let credits = offshore_credits_in(&db, ws.id).await.unwrap();
    assert_eq!(credits.distributions.len(), 1, "{:?}", credits.distributions);
    let credit = credits.rows.iter().find(|r| r.origin_year == 2023 && r.as_of_year == 2024).unwrap();
    assert!(close(credit.taxed_brl, 9_600.0) && close(credit.balance_brl, 0.0) && close(credit.over_distributed_brl, 2_400.0), "{credit:?}");
}

#[tokio::test]
async fn a_fund_estimate_uses_the_position_the_quota_values_and_the_stored_rate() {
    use crate::commands::fund::{funds_overview_in, set_fund_term_in, upsert_fund_quota_in, UpsertFundQuotaRequest};
    use crate::commands::transaction::compute_positions;
    use crate::domain::tax::come_cotas::{BasisSource, EstimateStatus};

    let db = memory_db().await;
    let ws = workspace(&db, "Fundos").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let fund = asset_in(&db, "FUNDO1", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
    let quota = |date: &str, value: f64| UpsertFundQuotaRequest { asset_id: fund.id, quota_date: date.to_string(), value };

    // 100 cotas a R$ 10,00 em jan/2025
    add(&db, tx_request(pf.id, fund.id, "compra", 100.0, 10.0, "2025-01-10", 0.0)).await;

    // sem tipo e sem cota: o fundo aparece, sem estimativa
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    assert_eq!(overview.funds.len(), 1);
    assert!(close(overview.funds[0].quantity, 100.0) && overview.funds[0].term.is_none());
    assert_eq!(overview.estimates[0].status, EstimateStatus::NoTerm);

    set_fund_term_in(&db, fund.id, "longo_prazo").await.unwrap();
    assert_eq!(funds_overview_in(&db, ws.id, today).await.unwrap().estimates[0].status, EstimateStatus::NoQuota);

    // última data de come-cotas: 29/05/2026 (cota de 28/05 a 11,00); hoje a cota vale 12,00
    upsert_fund_quota_in(&db, quota("2026-05-28", 11.0), today).await.unwrap();
    upsert_fund_quota_in(&db, quota("2026-10-20", 11.5), today).await.unwrap();
    // o mesmo dia de novo corrige em vez de duplicar
    upsert_fund_quota_in(&db, quota("2026-10-20", 12.0), today).await.unwrap();

    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    assert_eq!(overview.funds[0].quotas.len(), 2, "{:?}", overview.funds[0].quotas);
    let e = &overview.estimates[0];
    // base 11,00 (a cota do último come-cotas supera o custo de 10,00): ganho 100, imposto 15, 1,25 cota
    assert_eq!(e.status, EstimateStatus::Estimated);
    assert_eq!(e.basis_source, Some(BasisSource::LastComeCotas));
    assert!(close(e.gain, 100.0) && close(e.rate, 0.15) && close(e.tax, 15.0), "{e:?}");
    assert!(close(e.quotas_deducted, 1.25), "{e:?}");
    assert_eq!(e.reference_date.to_string(), "2026-11-30");

    // curto prazo: 20%
    set_fund_term_in(&db, fund.id, "curto_prazo").await.unwrap();
    let e = &funds_overview_in(&db, ws.id, today).await.unwrap().estimates[0];
    assert!(close(e.rate, 0.20) && close(e.tax, 20.0), "{e:?}");

    // a posição vale o último valor de cota lançado
    let positions = compute_positions(&db, pf.id).await.unwrap();
    let position = positions.iter().find(|p| p.ticker == "FUNDO1").unwrap();
    assert_eq!(position.price_source, "manual_quota");
    assert!(
        close(position.current_price.unwrap(), 12.0) && close(position.market_value.unwrap(), 1200.0),
        "{:?} {:?}",
        position.current_price,
        position.market_value
    );
}

#[tokio::test]
async fn the_cvm_quota_fills_the_fund_prices_the_position_and_never_overwrites_a_manual_quota() {
    use crate::commands::fund::{
        funds_overview_with, refresh_fund_quotas_with, set_fund_cvm_in, set_fund_term_in, upsert_fund_quota_in,
        SetFundCvmRequest, UpsertFundQuotaRequest,
    };
    use crate::commands::transaction::compute_positions;
    use crate::entity::fund_quota_value;
    use crate::error::AppError;
    use crate::finance_api::fund_quota_store::{CvmStatus, Session};
    use crate::finance_core::cvm_inf_diario::tests::{csv_of, FakeZips, CLASS_CNPJ};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use std::sync::atomic::Ordering;

    let db = memory_db().await;
    let ws = workspace(&db, "CVM").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let fund = asset_in(&db, "FUNDO2", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
    let session = Session::new();
    add(&db, tx_request(pf.id, fund.id, "compra", 100.0, 10.0, "2026-09-10", 0.0)).await;
    set_fund_term_in(&db, fund.id, "curto_prazo").await.unwrap();

    // sem CNPJ a CVM não entra: nada a buscar e o estado diz por quê
    let zips = FakeZips::new(&[
        (
            (2026, 9),
            csv_of(&["CLASSES - FIF;00.017.024/0001-53;;2026-09-30;1119426.37;44.500000000000;1221524.87;0.00;0.00;1"]),
        ),
        (
            (2026, 10),
            csv_of(&[
                "CLASSES - FIF;00.017.024/0001-53;;2026-10-01;1119426.37;44.990762600000;1221524.87;0.00;2902.97;1",
                "CLASSES - FIF;00.017.024/0001-53;;2026-10-02;1120004.10;45.009590600000;1222036.06;0.00;0.00;1",
            ]),
        ),
    ]);
    refresh_fund_quotas_with(&db, &session, &zips, ws.id, today, false).await.unwrap();
    assert_eq!(zips.calls.load(Ordering::SeqCst), 0);
    let overview = funds_overview_with(&db, &session, ws.id, today).await.unwrap();
    assert_eq!(overview.funds[0].cvm_status, CvmStatus::NoCnpj);
    assert!(overview.funds[0].quotas.is_empty());

    // CNPJ inválido é recusado; o válido é guardado pontuado
    let bad = SetFundCvmRequest { asset_id: fund.id, cnpj: Some("123".into()), subclass: None };
    assert!(matches!(set_fund_cvm_in(&db, &session, bad).await, Err(AppError::InvalidInput(_))));
    let set = SetFundCvmRequest { asset_id: fund.id, cnpj: Some("00017024000153".into()), subclass: None };
    set_fund_cvm_in(&db, &session, set).await.unwrap();

    // uma cota do usuário no dia 02/10 (valor bem diferente do da CVM)
    let manual = UpsertFundQuotaRequest { asset_id: fund.id, quota_date: "2026-10-02".into(), value: 99.0 };
    upsert_fund_quota_in(&db, manual, today).await.unwrap();

    refresh_fund_quotas_with(&db, &session, &zips, ws.id, today, false).await.unwrap();
    assert_eq!(zips.calls.load(Ordering::SeqCst), 2, "setembro e outubro, uma vez cada");
    let overview = funds_overview_with(&db, &session, ws.id, today).await.unwrap();
    let view = &overview.funds[0];
    assert_eq!(view.cnpj.as_deref(), Some(CLASS_CNPJ));
    assert_eq!(view.cvm_status, CvmStatus::Ok);
    assert_eq!(view.last_cvm_date.as_deref(), Some("2026-10-01"), "o dia 02/10 é do usuário");
    let by_date = |date: &str| view.quotas.iter().find(|q| q.quota_date == date).unwrap();
    assert!(close(by_date("2026-09-30").value, 44.5) && by_date("2026-09-30").source == "cvm");
    assert!(close(by_date("2026-10-01").value, 44.9907626) && by_date("2026-10-01").source == "cvm");
    assert!(close(by_date("2026-10-02").value, 99.0) && by_date("2026-10-02").source == "manual");

    // a cota mais recente é a do usuário: a posição diz isso
    let position = |positions: Vec<crate::commands::transaction::PositionView>| {
        positions.into_iter().find(|p| p.ticker == "FUNDO2").unwrap()
    };
    let current = position(compute_positions(&db, pf.id).await.unwrap());
    assert_eq!(current.price_source, "manual_quota");
    assert!(close(current.current_price.unwrap(), 99.0));

    // sem a cota do usuário, a da CVM volta no dia 02/10 e passa a ser o preço da posição
    fund_quota_value::Entity::delete_many()
        .filter(fund_quota_value::Column::AssetId.eq(fund.id))
        .filter(fund_quota_value::Column::Source.eq("manual"))
        .exec(&db)
        .await
        .unwrap();
    refresh_fund_quotas_with(&db, &session, &zips, ws.id, today, true).await.unwrap();
    let current = position(compute_positions(&db, pf.id).await.unwrap());
    assert_eq!(current.price_source, "cvm_quota");
    assert!(close(current.current_price.unwrap(), 45.0095906), "{:?}", current.current_price);
    assert!(close(current.market_value.unwrap(), 4500.95906), "{:?}", current.market_value);

    // corrigir à mão uma cota da CVM a torna do usuário
    let fix = UpsertFundQuotaRequest { asset_id: fund.id, quota_date: "2026-10-02".into(), value: 46.0 };
    upsert_fund_quota_in(&db, fix, today).await.unwrap();
    refresh_fund_quotas_with(&db, &session, &zips, ws.id, today, true).await.unwrap();
    let overview = funds_overview_with(&db, &session, ws.id, today).await.unwrap();
    let fixed = overview.funds[0].quotas.iter().find(|q| q.quota_date == "2026-10-02").unwrap();
    assert!(close(fixed.value, 46.0) && fixed.source == "manual");

    // trocar o CNPJ descarta o que veio da CVM e mantém o do usuário
    let other = SetFundCvmRequest { asset_id: fund.id, cnpj: Some("97.929.213/0001-34".into()), subclass: None };
    set_fund_cvm_in(&db, &session, other).await.unwrap();
    let overview = funds_overview_with(&db, &session, ws.id, today).await.unwrap();
    assert_eq!(overview.funds[0].quotas.len(), 1, "{:?}", overview.funds[0].quotas);
    assert_eq!(overview.funds[0].quotas[0].source, "manual");
}

#[tokio::test]
async fn a_cnpj_with_only_subclasses_asks_for_one_and_a_cvm_outage_rests_the_network() {
    use crate::commands::fund::{funds_overview_with, refresh_fund_quotas_with, set_fund_cvm_in, set_fund_term_in, SetFundCvmRequest};
    use crate::error::AppError;
    use crate::finance_api::fund_quota_store::{CvmStatus, Session};
    use crate::finance_core::cvm_inf_diario::tests::{csv_of, FakeZips};
    use std::sync::atomic::Ordering;

    let db = memory_db().await;
    let ws = workspace(&db, "Subclasses").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let fund = asset_in(&db, "FUNDO3", "fundo", "BRL").await;
    let stock = asset_in(&db, "PETR4", "acao_br", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
    let session = Session::new();
    add(&db, tx_request(pf.id, fund.id, "compra", 10.0, 70.0, "2026-10-01", 0.0)).await;

    let request = |subclass: Option<&str>| SetFundCvmRequest {
        asset_id: fund.id,
        cnpj: Some("00.888.897/0001-31".into()),
        subclass: subclass.map(str::to_string),
    };
    // só fundo recebe CNPJ da CVM
    let on_stock = SetFundCvmRequest { asset_id: stock.id, cnpj: Some("00.888.897/0001-31".into()), subclass: None };
    assert!(matches!(set_fund_cvm_in(&db, &session, on_stock).await, Err(AppError::InvalidInput(_))));
    // a subclasse mora em `fund_details`, que só existe depois do tipo do fundo
    assert!(matches!(set_fund_cvm_in(&db, &session, request(Some("RBMFN1747320951"))).await, Err(AppError::InvalidInput(_))));
    set_fund_cvm_in(&db, &session, request(None)).await.unwrap();

    let zips = FakeZips::new(&[(
        (2026, 10),
        csv_of(&[
            "CLASSES - FIF;00.888.897/0001-31;RBMFN1747320951;2026-10-01;47114895.69;74.383553275471;47091275.92;74000.00;17000.00;1",
            "CLASSES - FIF;00.888.897/0001-31;MZMRC1747322915;2026-10-01;700260258.28;74.040455402933;700168039.98;971319.87;6732106.10;10909",
        ]),
    )]);
    refresh_fund_quotas_with(&db, &session, &zips, ws.id, today, false).await.unwrap();
    let overview = funds_overview_with(&db, &session, ws.id, today).await.unwrap();
    assert_eq!(
        overview.funds[0].cvm_status,
        CvmStatus::NeedsSubclass { options: vec!["MZMRC1747322915".into(), "RBMFN1747320951".into()] }
    );
    assert!(overview.funds[0].quotas.is_empty());

    // escolhida a subclasse (depois do tipo), a cota dela entra
    set_fund_term_in(&db, fund.id, "longo_prazo").await.unwrap();
    set_fund_cvm_in(&db, &session, request(Some("RBMFN1747320951"))).await.unwrap();
    refresh_fund_quotas_with(&db, &session, &zips, ws.id, today, false).await.unwrap();
    let overview = funds_overview_with(&db, &session, ws.id, today).await.unwrap();
    assert_eq!(overview.funds[0].cvm_status, CvmStatus::Ok);
    assert_eq!(overview.funds[0].cvm_subclass.as_deref(), Some("RBMFN1747320951"));
    assert!(close(overview.funds[0].quotas[0].value, 74.383553275471));

    // uma CVM fora do ar vira erro no fundo e deixa a rede de molho: a segunda tentativa nem chama o fetcher
    let down = FakeZips::down();
    set_fund_cvm_in(&db, &session, request(Some("MZMRC1747322915"))).await.unwrap();
    assert!(refresh_fund_quotas_with(&db, &session, &down, ws.id, today, true).await.is_err());
    assert_eq!(down.calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        funds_overview_with(&db, &session, ws.id, today).await.unwrap().funds[0].cvm_status,
        CvmStatus::Error { .. }
    ));
    assert!(refresh_fund_quotas_with(&db, &session, &down, ws.id, today, true).await.is_err());
    assert_eq!(down.calls.load(Ordering::SeqCst), 1, "em espera depois da falha");
}

#[tokio::test]
async fn a_come_cotas_entry_takes_quotas_at_the_average_cost_and_is_recorded_for_the_estimate() {
    use crate::commands::fund::{funds_overview_in, set_fund_term_in, upsert_fund_quota_in, UpsertFundQuotaRequest};
    use crate::commands::transaction::compute_positions;
    use crate::domain::tax::come_cotas::BasisSource;

    let db = memory_db().await;
    let ws = workspace(&db, "Come-cotas").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let fund = asset_in(&db, "FUNDO1", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();

    // 100 cotas a R$ 10,00; o fundo desconta 5 cotas a R$ 12,00 (imposto de R$ 60,00) em 29/05/2026
    add(&db, tx_request(pf.id, fund.id, "compra", 100.0, 10.0, "2025-01-10", 0.0)).await;
    add(&db, tx_request(pf.id, fund.id, "come_cotas", 5.0, 12.0, "2026-05-29", 0.0)).await;
    set_fund_term_in(&db, fund.id, "longo_prazo").await.unwrap();
    upsert_fund_quota_in(
        &db,
        UpsertFundQuotaRequest { asset_id: fund.id, quota_date: "2026-10-20".to_string(), value: 13.0 },
        today,
    )
    .await
    .unwrap();

    // a posição perde as cotas, o custo médio por cota não muda e o custo consumido vira prejuízo realizado
    let positions = compute_positions(&db, pf.id).await.unwrap();
    let position = positions.iter().find(|p| p.ticker == "FUNDO1").unwrap();
    assert!(close(position.quantity, 95.0), "{}", position.quantity);
    assert!(close(position.average_buy_price.unwrap(), 10.0), "{:?}", position.average_buy_price);
    assert!(close(position.realized_pl, -50.0), "{}", position.realized_pl);

    // a coluna de quantidade corrente do livro desconta as cotas
    let entries = list_transactions_in(&db, pf.id).await.unwrap();
    let row = entries.iter().find(|e| e.transaction_type == "come_cotas").unwrap();
    assert!(close(row.running_quantity.unwrap(), 95.0), "{:?}", row.running_quantity);
    assert!(close(row.total_value, 60.0));

    // a estimativa usa o valor de cota do desconto registrado como base e não avisa que faltou registro
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    assert_eq!(overview.funds[0].deductions.len(), 1);
    assert!(close(overview.funds[0].deductions[0].tax, 60.0));
    let e = &overview.estimates[0];
    assert_eq!(e.basis_source, Some(BasisSource::LastComeCotas));
    assert!(close(e.basis_per_quota.unwrap(), 12.0), "{e:?}");
    // cota atual 13,00 contra base 12,00: ganho de 1,00 por cota em 95 cotas
    assert!(close(e.gain, 95.0) && close(e.tax, 14.25), "{e:?}");
    assert_eq!(e.last_reference_unrecorded, None);
}

#[tokio::test]
async fn a_fund_redemption_estimates_the_iof_and_the_ir_with_the_come_cotas_credit() {
    use crate::commands::fund::{funds_overview_in, set_fund_term_in};

    let db = memory_db().await;
    let ws = workspace(&db, "Resgate").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let fund = asset_in(&db, "FUNDO1", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();

    // 100 cotas a R$ 10,00; come-cotas de 2,5 cotas a R$ 12,00; resgate das 97,5 cotas restantes a R$ 15,00
    add(&db, tx_request(pf.id, fund.id, "compra", 100.0, 10.0, "2024-01-02", 0.0)).await;
    add(&db, tx_request(pf.id, fund.id, "come_cotas", 2.5, 12.0, "2024-05-31", 0.0)).await;
    add(&db, tx_request(pf.id, fund.id, "venda", 97.5, 15.0, "2026-03-01", 0.0)).await;

    // sem o tipo do fundo não há estimativa de resgate
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    assert!(overview.funds[0].redemptions.is_empty());

    set_fund_term_in(&db, fund.id, "longo_prazo").await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let redemptions = &overview.funds[0].redemptions;
    assert_eq!(redemptions.len(), 1);
    let r = &redemptions[0];
    // 789 dias (15%): rendimento 487,50 + 5 já tributados = 492,50; IR 73,875 − 30 de come-cotas = 43,875
    assert!(close(r.gross, 1462.5) && close(r.cost, 975.0), "{r:?}");
    assert!(close(r.iof, 0.0) && close(r.ir, 43.875) && close(r.come_cotas_credit, 30.0), "{r:?}");
    assert!(close(r.net, 1418.625), "{r:?}");
    assert!(close(r.uncovered_quotas, 0.0) && close(r.excess_come_cotas, 0.0));

    // fundo de ações: 15% sem come-cotas nem IOF (o come-cotas registrado não muda a regra do resgate)
    set_fund_term_in(&db, fund.id, "acoes").await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    assert!(close(overview.funds[0].redemptions[0].iof, 0.0));
}

#[tokio::test]
async fn a_fall_after_a_come_cotas_is_credited_at_the_updated_cost_through_the_overview() {
    use crate::commands::fund::{funds_overview_in, set_fund_administrator_in, set_fund_term_in};

    let db = memory_db().await;
    let ws = workspace(&db, "Perda legal").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let loser = asset_in(&db, "FUNDOA", "fundo", "BRL").await;
    let winner = asset_in(&db, "FUNDOB", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();

    // A: 100 cotas a 10,00; come-cotas de 2,5 cotas a 12,00 em 31/03/2025 (30 pagos); resgate de 97,5 a 9,00 em 01/06/2025.
    // Perda legal 97,5 × (12 − 9) = 292,50 (pelo custo original seriam 92,50); o come-cotas pago vira excesso de 30.
    // B: 100 cotas a 10,00 resgatadas a 12,00 em 01/09/2025 (243 dias, 20%): ganho 200, IR 40. O crédito de A zera o IR de B
    // (gasta 200) e sobram 292,50 − 200 = 92,50 no grupo XP.
    add(&db, tx_request(pf.id, loser.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    add(&db, tx_request(pf.id, loser.id, "come_cotas", 2.5, 12.0, "2025-03-31", 0.0)).await;
    add(&db, tx_request(pf.id, loser.id, "venda", 97.5, 9.0, "2025-06-01", 0.0)).await;
    add(&db, tx_request(pf.id, winner.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    add(&db, tx_request(pf.id, winner.id, "venda", 100.0, 12.0, "2025-09-01", 0.0)).await;
    for fund in [&loser, &winner] {
        set_fund_term_in(&db, fund.id, "longo_prazo").await.unwrap();
        set_fund_administrator_in(&db, fund.id, Some("XP")).await.unwrap();
    }

    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let a = &overview.funds.iter().find(|f| f.ticker == "FUNDOA").unwrap().redemptions[0];
    let b = &overview.funds.iter().find(|f| f.ticker == "FUNDOB").unwrap().redemptions[0];
    assert!(close(a.loss_created, 292.5) && close(a.ir, 0.0) && close(a.excess_come_cotas, 30.0), "{a:?}");
    assert!(close(b.ir_before_compensation, 40.0) && close(b.loss_credit_used, 200.0) && close(b.ir, 0.0), "{b:?}");
    assert_eq!(overview.loss_credits.len(), 1, "{:?}", overview.loss_credits);
    assert!(close(overview.loss_credits[0].amount, 92.5), "{:?}", overview.loss_credits[0]);
}

#[tokio::test]
async fn leaving_an_administrator_with_a_loss_balance_warns_until_the_record_guarantee_ends() {
    use crate::commands::fund::{funds_overview_in, set_fund_administrator_in, set_fund_term_in};

    let db = memory_db().await;
    let ws = workspace(&db, "Guarda").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let loser = asset_in(&db, "FUNDOA", "fundo", "BRL").await;
    let other = asset_in(&db, "FUNDOB", "fundo", "BRL").await;
    let day = |y, m, d| chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap();

    // A: 100 cotas a 10,00 resgatadas a 8,00 em 01/06/2025 (perda de 200, saldo 200 do grupo XP/longo prazo)
    add(&db, tx_request(pf.id, loser.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    add(&db, tx_request(pf.id, loser.id, "venda", 100.0, 8.0, "2025-06-01", 0.0)).await;
    set_fund_term_in(&db, loser.id, "longo_prazo").await.unwrap();
    set_fund_administrator_in(&db, loser.id, Some("XP")).await.unwrap();

    // todos os fundos lançados da XP estão zerados: a guarda vai até 31/12/2026 (o ano seguinte ao do resgate)
    let overview = funds_overview_in(&db, ws.id, day(2026, 10, 22)).await.unwrap();
    assert_eq!(overview.record_notes.len(), 1, "{:?}", overview.record_notes);
    let note = &overview.record_notes[0];
    assert_eq!((note.redeemed_on, note.guaranteed_until, note.expired), (day(2025, 6, 1), day(2026, 12, 31), false));
    assert!(close(note.balance, 200.0), "{note:?}");

    // depois de 31/12/2026 a garantia terminou
    let overview = funds_overview_in(&db, ws.id, day(2027, 1, 5)).await.unwrap();
    assert!(overview.record_notes[0].expired);

    // um segundo fundo da XP, de outro tipo, ainda com cotas: não saiu por completo, sem aviso
    add(&db, tx_request(pf.id, other.id, "compra", 50.0, 10.0, "2025-02-01", 0.0)).await;
    set_fund_term_in(&db, other.id, "curto_prazo").await.unwrap();
    set_fund_administrator_in(&db, other.id, Some(" xp")).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, day(2026, 10, 22)).await.unwrap();
    assert!(overview.record_notes.is_empty(), "{:?}", overview.record_notes);
}

#[tokio::test]
async fn a_loss_in_one_fund_offsets_the_later_ir_of_another_fund_of_the_same_administrator() {
    use crate::commands::fund::{funds_overview_in, set_fund_administrator_in, set_fund_term_in};
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Compensação").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let loser = asset_in(&db, "FUNDOA", "fundo", "BRL").await;
    let winner = asset_in(&db, "FUNDOB", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();

    // A: 100 cotas a 10,00 resgatadas a 8,00 em 01/06/2025 (perda de 200). B: 100 cotas a 10,00 resgatadas a 12,00 em
    // 01/09/2025 (243 dias, 20%: ganho de 200, IR de 40)
    add(&db, tx_request(pf.id, loser.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    add(&db, tx_request(pf.id, loser.id, "venda", 100.0, 8.0, "2025-06-01", 0.0)).await;
    add(&db, tx_request(pf.id, winner.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    add(&db, tx_request(pf.id, winner.id, "venda", 100.0, 12.0, "2025-09-01", 0.0)).await;

    // a administradora só se informa depois do tipo do fundo
    let early = set_fund_administrator_in(&db, loser.id, Some("XP")).await;
    assert!(matches!(early, Err(AppError::InvalidInput(_))), "sem o tipo do fundo");
    set_fund_term_in(&db, loser.id, "longo_prazo").await.unwrap();
    set_fund_term_in(&db, winner.id, "longo_prazo").await.unwrap();

    // sem administradora cada fundo é um grupo: nada se compensa e o saldo da perda fica no fundo A
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let b = &overview.funds.iter().find(|f| f.ticker == "FUNDOB").unwrap().redemptions[0];
    assert!(close(b.ir, 40.0) && close(b.loss_credit_used, 0.0), "{b:?}");
    assert_eq!(overview.loss_credits.len(), 1);
    assert!(close(overview.loss_credits[0].amount, 200.0) && overview.loss_credits[0].tickers == ["FUNDOA"]);

    // mesma administradora (sem diferença de caixa nem de espaços): a perda zera o IR do outro fundo
    set_fund_administrator_in(&db, loser.id, Some("XP")).await.unwrap();
    set_fund_administrator_in(&db, winner.id, Some("  xp ")).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let a = &overview.funds.iter().find(|f| f.ticker == "FUNDOA").unwrap().redemptions[0];
    let b = &overview.funds.iter().find(|f| f.ticker == "FUNDOB").unwrap().redemptions[0];
    assert!(close(a.loss_created, 200.0), "{a:?}");
    assert!(close(b.ir, 0.0) && close(b.ir_before_compensation, 40.0) && close(b.loss_credit_used, 200.0), "{b:?}");
    assert!(close(b.net, 1200.0), "{b:?}");
    assert!(overview.loss_credits.is_empty(), "o crédito foi todo usado: {:?}", overview.loss_credits);
    let administrator_of = |ticker: &str| {
        overview.funds.iter().find(|f| f.ticker == ticker).unwrap().administrator.clone()
    };
    assert_eq!(administrator_of("FUNDOA").as_deref(), Some("XP"));
    assert_eq!(administrator_of("FUNDOB").as_deref(), Some("xp"), "guarda o texto sem espaços sobrando");

    // apagar a administradora volta ao fundo isolado
    set_fund_administrator_in(&db, winner.id, None).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let b = &overview.funds.iter().find(|f| f.ticker == "FUNDOB").unwrap().redemptions[0];
    assert!(close(b.ir, 40.0), "{b:?}");
}

#[tokio::test]
async fn the_loss_credit_reduces_the_next_come_cotas_estimate_of_the_same_group() {
    use crate::commands::fund::{funds_overview_in, set_fund_administrator_in, set_fund_term_in, upsert_fund_quota_in, UpsertFundQuotaRequest};

    let db = memory_db().await;
    let ws = workspace(&db, "Compensação come-cotas").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let loser = asset_in(&db, "FUNDOA", "fundo", "BRL").await;
    let holder = asset_in(&db, "FUNDOC", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();

    // A perde 200 em 01/06/2025; C tem 100 cotas a 10,00 compradas em 01/07/2026, cota a 12,00 (ganho 200, 15% = 30)
    add(&db, tx_request(pf.id, loser.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    add(&db, tx_request(pf.id, loser.id, "venda", 100.0, 8.0, "2025-06-01", 0.0)).await;
    add(&db, tx_request(pf.id, holder.id, "compra", 100.0, 10.0, "2026-07-01", 0.0)).await;
    for id in [loser.id, holder.id] {
        set_fund_term_in(&db, id, "longo_prazo").await.unwrap();
    }
    upsert_fund_quota_in(
        &db,
        UpsertFundQuotaRequest { asset_id: holder.id, quota_date: "2026-10-20".to_string(), value: 12.0 },
        today,
    )
    .await
    .unwrap();

    let estimate_of = |overview: &crate::commands::fund::FundsOverview| {
        overview.estimates.iter().find(|e| e.asset_id == holder.id).unwrap().clone()
    };

    // grupos separados: o crédito não chega ao come-cotas de C
    let separate = estimate_of(&funds_overview_in(&db, ws.id, today).await.unwrap());
    assert!(close(separate.tax, 30.0) && close(separate.loss_credit_used, 0.0), "{separate:?}");

    // mesmo grupo: a perda de 200 zera o ganho de 200 da estimativa, e o saldo some
    set_fund_administrator_in(&db, loser.id, Some("XP")).await.unwrap();
    set_fund_administrator_in(&db, holder.id, Some("XP")).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let grouped = estimate_of(&overview);
    assert!(close(grouped.loss_credit_used, 200.0) && close(grouped.tax, 0.0), "{grouped:?}");
    assert!(close(grouped.tax_before_compensation, 30.0) && close(grouped.quotas_deducted, 0.0), "{grouped:?}");
    // o saldo exibido é o disponível antes do come-cotas estimado (que ainda não aconteceu)
    assert!(close(overview.loss_credits[0].amount, 200.0), "{:?}", overview.loss_credits);
}

#[tokio::test]
async fn a_recorded_come_cotas_after_a_loss_in_the_same_group_gets_a_credit_note() {
    use crate::commands::fund::{funds_overview_in, set_fund_administrator_in, set_fund_term_in};

    let db = memory_db().await;
    let ws = workspace(&db, "Aviso do crédito").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let loser = asset_in(&db, "FUNDOA", "fundo", "BRL").await;
    let holder = asset_in(&db, "FUNDOC", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();

    // A perde 200 em 01/06/2025. C tem 100 cotas a 10,00 e um come-cotas registrado de 5 cotas a 12,00 em 30/11/2025
    // (depois da perda); outro, de 1 cota, em 01/05/2025 (antes da perda)
    add(&db, tx_request(pf.id, loser.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    add(&db, tx_request(pf.id, loser.id, "venda", 100.0, 8.0, "2025-06-01", 0.0)).await;
    add(&db, tx_request(pf.id, holder.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    add(&db, tx_request(pf.id, holder.id, "come_cotas", 1.0, 11.0, "2025-05-01", 0.0)).await;
    add(&db, tx_request(pf.id, holder.id, "come_cotas", 5.0, 12.0, "2025-11-30", 0.0)).await;
    for id in [loser.id, holder.id] {
        set_fund_term_in(&db, id, "longo_prazo").await.unwrap();
    }
    let notes_of = |overview: &crate::commands::fund::FundsOverview, ticker: &str| {
        overview.funds.iter().find(|f| f.ticker == ticker).unwrap().credit_notes.clone()
    };

    // grupos separados: o saldo da perda é do fundo A, então o come-cotas de C não avisa
    let separate = funds_overview_in(&db, ws.id, today).await.unwrap();
    assert!(notes_of(&separate, "FUNDOC").is_empty(), "{:?}", notes_of(&separate, "FUNDOC"));

    // mesmo grupo: só o come-cotas de 30/11/2025 avisa, com o saldo de 200; o de 01/05/2025 veio antes da perda
    set_fund_administrator_in(&db, loser.id, Some("XP")).await.unwrap();
    set_fund_administrator_in(&db, holder.id, Some("XP")).await.unwrap();
    let grouped = funds_overview_in(&db, ws.id, today).await.unwrap();
    let notes = notes_of(&grouped, "FUNDOC");
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].date.to_string(), "2025-11-30");
    assert!(close(notes[0].balance, 200.0), "{notes:?}");
    assert!(notes_of(&grouped, "FUNDOA").is_empty());
    // o aviso não gasta crédito: o saldo do grupo segue inteiro
    assert!(close(grouped.loss_credits[0].amount, 200.0), "{:?}", grouped.loss_credits);
}

#[tokio::test]
async fn a_loss_credit_used_on_a_recorded_come_cotas_comes_off_the_group_balance() {
    use crate::commands::fund::{funds_overview_in, set_fund_administrator_in, set_fund_term_in};
    use crate::commands::transaction::create_transaction_in;

    let db = memory_db().await;
    let ws = workspace(&db, "Crédito usado").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let loser = asset_in(&db, "FUNDOA", "fundo", "BRL").await;
    let holder = asset_in(&db, "FUNDOC", "fundo", "BRL").await;
    let stock = asset_in(&db, "PETR4", "acao_br", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();

    // A perde 200 em 01/06/2025. C tem 100 cotas a 10,00 e um come-cotas de 5 cotas a 12,00 em 30/11/2025 em que o usuário
    // informou 150 de rendimento coberto pelo crédito
    add(&db, tx_request(pf.id, loser.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    add(&db, tx_request(pf.id, loser.id, "venda", 100.0, 8.0, "2025-06-01", 0.0)).await;
    add(&db, tx_request(pf.id, holder.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    let mut come_cotas = tx_request(pf.id, holder.id, "come_cotas", 5.0, 12.0, "2025-11-30", 0.0);
    come_cotas.loss_credit_used = Some(150.0);
    add(&db, come_cotas).await;
    for id in [loser.id, holder.id] {
        set_fund_term_in(&db, id, "longo_prazo").await.unwrap();
        set_fund_administrator_in(&db, id, Some("XP")).await.unwrap();
    }

    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let holder_view = overview.funds.iter().find(|f| f.ticker == "FUNDOC").unwrap();
    // o valor chega à linha do come-cotas registrado, e como foi informado e cabe no saldo, não há aviso
    assert_eq!(holder_view.deductions[0].loss_credit_used, Some(150.0));
    assert!(holder_view.credit_notes.is_empty(), "{:?}", holder_view.credit_notes);
    // o saldo do grupo caiu de 200 para 50
    assert_eq!(overview.loss_credits.len(), 1);
    assert!(close(overview.loss_credits[0].amount, 50.0), "{:?}", overview.loss_credits);

    // um valor acima do saldo avisa (saldo 200, informado 300 num segundo fundo do grupo sem saldo restante)
    let extra = asset_in(&db, "FUNDOD", "fundo", "BRL").await;
    add(&db, tx_request(pf.id, extra.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    let mut over = tx_request(pf.id, extra.id, "come_cotas", 5.0, 12.0, "2025-12-15", 0.0);
    over.loss_credit_used = Some(300.0);
    add(&db, over).await;
    set_fund_term_in(&db, extra.id, "longo_prazo").await.unwrap();
    set_fund_administrator_in(&db, extra.id, Some("XP")).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let notes = &overview.funds.iter().find(|f| f.ticker == "FUNDOD").unwrap().credit_notes;
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(close(notes[0].balance, 50.0) && notes[0].informed == Some(300.0), "{notes:?}");
    assert!(overview.loss_credits.is_empty(), "o saldo foi todo usado: {:?}", overview.loss_credits);

    // o campo só vale para come_cotas e não pode ser negativo
    let mut on_purchase = tx_request(pf.id, loser.id, "compra", 1.0, 10.0, "2026-01-02", 0.0);
    on_purchase.loss_credit_used = Some(1.0);
    assert!(create_transaction_in(&db, on_purchase).await.is_err(), "fora do come_cotas");
    let mut negative = tx_request(pf.id, holder.id, "come_cotas", 1.0, 12.0, "2026-01-02", 0.0);
    negative.loss_credit_used = Some(-1.0);
    assert!(create_transaction_in(&db, negative).await.is_err(), "negativo");
    let mut not_finite = tx_request(pf.id, holder.id, "come_cotas", 1.0, 12.0, "2026-01-02", 0.0);
    not_finite.loss_credit_used = Some(f64::NAN);
    assert!(create_transaction_in(&db, not_finite).await.is_err(), "não finito");
    // (o guard de ativo de fundo vale como antes; a ação só serve para provar que o campo não abre exceção)
    let mut on_stock = tx_request(pf.id, stock.id, "come_cotas", 1.0, 12.0, "2026-01-02", 0.0);
    on_stock.loss_credit_used = Some(1.0);
    assert!(create_transaction_in(&db, on_stock).await.is_err(), "ativo que não é fundo");
}

/// Amortização de fundo fechado: o valor recebido e a cota do dia, sem quantidade nem taxa.
fn amortization_request(
    portfolio_id: i32,
    asset_id: i32,
    amount: f64,
    quota_value: f64,
    date: &str,
) -> crate::commands::transaction::CreateTransactionRequest {
    let mut request = tx_request(portfolio_id, asset_id, "amortizacao", 1.0, quota_value, date, 0.0);
    request.quantity = None;
    request.total_value = amount;
    request
}

#[tokio::test]
async fn an_amortization_returns_capital_without_removing_quotas_and_is_taxed_as_a_share_of_the_fund() {
    use crate::commands::fund::{funds_overview_in, set_fund_term_in};
    use crate::commands::transaction::{compute_positions, create_transaction_in};
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Amortização").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let fund = asset_in(&db, "FUNDOFIP", "fundo", "BRL").await;
    let stock = asset_in(&db, "PETR4", "acao_br", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();

    add(&db, tx_request(pf.id, fund.id, "compra", 100.0, 10.0, "2025-01-01", 0.0)).await;
    add(&db, tx_request(pf.id, stock.id, "compra", 10.0, 30.0, "2025-01-01", 0.0)).await;

    // só vale para fundo, sem quantidade nem taxa, com cota e valor positivos, com cotas na data e dentro do valor da posição
    let on_stock = create_transaction_in(&db, amortization_request(pf.id, stock.id, 100.0, 30.0, "2026-03-01")).await;
    assert!(matches!(on_stock, Err(AppError::InvalidGuard(_))), "ativo que não é fundo");
    let mut with_quantity = amortization_request(pf.id, fund.id, 600.0, 12.0, "2026-03-01");
    with_quantity.quantity = Some(50.0);
    assert!(matches!(create_transaction_in(&db, with_quantity).await, Err(AppError::InvalidGuard(_))), "com quantidade");
    let mut with_fee = amortization_request(pf.id, fund.id, 600.0, 12.0, "2026-03-01");
    with_fee.fee = Some(1.0);
    assert!(matches!(create_transaction_in(&db, with_fee).await, Err(AppError::InvalidGuard(_))), "com taxa");
    let no_quota = create_transaction_in(&db, amortization_request(pf.id, fund.id, 600.0, 0.0, "2026-03-01")).await;
    assert!(matches!(no_quota, Err(AppError::InvalidGuard(_))), "sem valor da cota");
    let no_amount = create_transaction_in(&db, amortization_request(pf.id, fund.id, 0.0, 12.0, "2026-03-01")).await;
    assert!(matches!(no_amount, Err(AppError::InvalidGuard(_))), "sem valor recebido");
    let too_much = create_transaction_in(&db, amortization_request(pf.id, fund.id, 1_300.0, 12.0, "2026-03-01")).await;
    assert!(matches!(too_much, Err(AppError::InvalidGuard(_))), "passa do valor da posição (100 × 12 = 1.200)");
    let before_buying = create_transaction_in(&db, amortization_request(pf.id, fund.id, 100.0, 12.0, "2024-12-31")).await;
    assert!(matches!(before_buying, Err(AppError::InvalidGuard(_))), "sem cotas na data");

    // 600 de 1.200: as 100 cotas ficam, o custo cai pela metade (5,00 por cota) e o ganho de 100 vira resultado realizado
    add(&db, amortization_request(pf.id, fund.id, 600.0, 12.0, "2026-03-01")).await;
    let positions = compute_positions(&db, pf.id).await.unwrap();
    let position = positions.iter().find(|p| p.ticker == "FUNDOFIP").unwrap();
    assert!(close(position.quantity, 100.0), "{}", position.quantity);
    assert!(close(position.average_buy_price.unwrap(), 5.0), "{:?}", position.average_buy_price);
    assert!(close(position.realized_pl, 100.0), "{}", position.realized_pl);

    // o resgate estimado mostra a amortização: 424 dias (17,5%) sobre 100 de rendimento, e as cotas continuam em carteira
    set_fund_term_in(&db, fund.id, "longo_prazo").await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let r = &overview.funds[0].redemptions[0];
    assert!(r.is_amortization && close(r.gross, 600.0) && close(r.quotas, 50.0), "{r:?}");
    assert!(close(r.ir, 17.5) && close(r.net, 582.5), "{r:?}");
    assert!(close(overview.funds[0].quantity, 100.0));
}

#[tokio::test]
async fn the_2023_stock_of_a_closed_fund_needs_the_type_the_option_and_the_quota_of_31_12_2023() {
    use crate::commands::fund::{
        funds_overview_in, set_fund_estoque_in, set_fund_structure_in, set_fund_term_in, upsert_fund_quota_in,
        EstoqueStatus, UpsertFundQuotaRequest,
    };
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Estoque").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let fund = asset_in(&db, "FUNDOE", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
    let quota = |date: &str, value: f64| UpsertFundQuotaRequest {
        asset_id: fund.id,
        quota_date: date.to_string(),
        value,
    };

    // 100 cotas a 10,00 em 01/06/2022 e resgate de todas a 15,00 em 01/03/2026
    add(&db, tx_request(pf.id, fund.id, "compra", 100.0, 10.0, "2022-06-01", 0.0)).await;
    add(&db, tx_request(pf.id, fund.id, "venda", 100.0, 15.0, "2026-03-01", 0.0)).await;

    // estrutura e opção só depois do tipo; valores desconhecidos são recusados
    assert!(matches!(set_fund_structure_in(&db, fund.id, "fechado").await, Err(AppError::InvalidInput(_))));
    assert!(matches!(set_fund_estoque_in(&db, fund.id, Some("geral")).await, Err(AppError::InvalidInput(_))));
    set_fund_term_in(&db, fund.id, "longo_prazo").await.unwrap();
    assert!(matches!(set_fund_structure_in(&db, fund.id, "mista").await, Err(AppError::InvalidInput(_))));
    assert!(matches!(set_fund_estoque_in(&db, fund.id, Some("tudo")).await, Err(AppError::InvalidInput(_))));

    // fundo aberto não tem estoque, mesmo com a opção marcada
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    assert!(matches!(overview.funds[0].estoque, EstoqueStatus::NotApplicable));
    assert_eq!(overview.funds[0].structure, "aberto");
    set_fund_estoque_in(&db, fund.id, Some("geral")).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    assert!(matches!(overview.funds[0].estoque, EstoqueStatus::NotApplicable));

    // fechado sem opção: nada é estimado; com opção e sem a cota de 2023 (uma de 21 dias antes não serve): pede a cota
    set_fund_structure_in(&db, fund.id, "fechado").await.unwrap();
    set_fund_estoque_in(&db, fund.id, None).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    assert!(matches!(overview.funds[0].estoque, EstoqueStatus::NotTaxed));
    // sem o estoque o resgate segue sem crédito: rendimento 500 a 15% = 75
    assert!(close(overview.funds[0].redemptions[0].ir, 75.0), "{:?}", overview.funds[0].redemptions[0]);

    set_fund_estoque_in(&db, fund.id, Some("geral")).await.unwrap();
    upsert_fund_quota_in(&db, quota("2023-12-10", 13.0), today).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    assert!(matches!(overview.funds[0].estoque, EstoqueStatus::NeedsQuota), "a cota de 10/12 está longe demais");

    // com a cota de 29/12/2023 a 14,00: ganho de 400 e imposto de 60 a 15%, que vira crédito no resgate
    upsert_fund_quota_in(&db, quota("2023-12-29", 14.0), today).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let EstoqueStatus::Estimated { estimate } = &overview.funds[0].estoque else {
        panic!("esperava a estimativa do estoque");
    };
    assert!(close(estimate.base, 400.0) && close(estimate.tax, 60.0) && close(estimate.rate, 0.15), "{estimate:?}");
    // nominal 500 + 400 já tributados = 900; IR devido 135 − 60 pagos = 75: a 15% o estoque só antecipa o mesmo imposto,
    // então o IR final não muda, mas agora 60 dele já foram pagos
    let r = &overview.funds[0].redemptions[0];
    assert!(close(r.ir, 75.0) && close(r.come_cotas_credit, 60.0), "{r:?}");

    // a opção de 8%: imposto de 32 e IR final de 103
    set_fund_estoque_in(&db, fund.id, Some("opcao_8")).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let EstoqueStatus::Estimated { estimate } = &overview.funds[0].estoque else {
        panic!("esperava a estimativa do estoque");
    };
    assert!(close(estimate.tax, 32.0) && close(estimate.rate, 0.08), "{estimate:?}");
    assert!(close(overview.funds[0].redemptions[0].ir, 103.0), "{:?}", overview.funds[0].redemptions[0]);
    assert_eq!(overview.funds[0].estoque_option.as_deref(), Some("opcao_8"));
}

#[tokio::test]
async fn the_cvm_quota_of_december_2023_feeds_the_stock_only_of_funds_that_have_one() {
    use crate::commands::fund::{
        funds_overview_with, refresh_fund_quotas_with, set_fund_cvm_in, set_fund_estoque_in, set_fund_structure_in,
        set_fund_term_in, EstoqueStatus, SetFundCvmRequest,
    };
    use crate::finance_api::fund_quota_store::Session;
    use crate::finance_core::cvm_inf_diario::tests::{csv_of, FakeZips};

    let db = memory_db().await;
    let ws = workspace(&db, "Estoque CVM").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
    // 100 cotas a 10,00 em 01/06/2022 (antes de 31/12/2023); a cota da CVM em 29/12/2023 é 14,00
    let zips = |dec23: bool| {
        // A janela de 730 dias pede todo mês de 2024-10 a 2026-10: os que não importam vêm só com o cabeçalho.
        let mut months: Vec<((i32, u32), String)> = (0..25_u32)
            .map(|i| ((2024 + ((9 + i) / 12) as i32, (9 + i) % 12 + 1), csv_of(&[])))
            .filter(|(month, _)| *month != (2026, 10))
            .collect();
        months.push((
            (2026, 10),
            csv_of(&["CLASSES - FIF;00.017.024/0001-53;;2026-10-01;1119426.37;44.990762600000;1221524.87;0.00;0.00;1"]),
        ));
        if dec23 {
            months.push((
                (2023, 12),
                csv_of(&["CLASSES - FIF;00.017.024/0001-53;;2023-12-29;1119426.37;14.000000000000;1221524.87;0.00;0.00;1"]),
            ));
        }
        FakeZips::new(&months)
    };
    // (ticker, estrutura, opção do estoque, deve pedir dez/2023)
    for (ticker, structure, option, wants) in [
        ("FCLOSED", "fechado", Some("geral"), true),
        ("FOPEN", "aberto", Some("geral"), false),
        ("FNOOPT", "fechado", None, false),
    ] {
        let fund = asset_in(&db, ticker, "fundo", "BRL").await;
        add(&db, tx_request(pf.id, fund.id, "compra", 100.0, 10.0, "2022-06-01", 0.0)).await;
        set_fund_term_in(&db, fund.id, "longo_prazo").await.unwrap();
        set_fund_structure_in(&db, fund.id, structure).await.unwrap();
        set_fund_estoque_in(&db, fund.id, option).await.unwrap();
        let session = Session::new();
        set_fund_cvm_in(
            &db,
            &session,
            SetFundCvmRequest { asset_id: fund.id, cnpj: Some("00017024000153".into()), subclass: None },
        )
        .await
        .unwrap();
        let fake = zips(true);
        refresh_fund_quotas_with(&db, &session, &fake, ws.id, today, true).await.unwrap();
        let overview = funds_overview_with(&db, &session, ws.id, today).await.unwrap();
        let view = overview.funds.iter().find(|f| f.ticker == ticker).unwrap();
        let has_dec23 = view.quotas.iter().any(|q| q.quota_date == "2023-12-29" && q.source == "cvm");
        assert_eq!(has_dec23, wants, "{ticker}: {:?}", view.quotas);
        if wants {
            // ganho de 400 (14,00 − 10,00 em 100 cotas) a 15% = 60
            let EstoqueStatus::Estimated { estimate } = &view.estoque else { panic!("{ticker}: sem estimativa do estoque") };
            assert!(close(estimate.base, 400.0) && close(estimate.tax, 60.0), "{estimate:?}");
        }
        // limpa o que a CVM gravou para o próximo fundo (mesmo CNPJ) não herdar a cota
        set_fund_cvm_in(&db, &session, SetFundCvmRequest { asset_id: fund.id, cnpj: None, subclass: None })
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn the_next_come_cotas_basis_adds_the_taxed_2023_stock_to_the_cost_after_an_amortization() {
    use crate::commands::fund::{
        funds_overview_in, set_fund_estoque_in, set_fund_structure_in, set_fund_term_in, upsert_fund_quota_in,
        UpsertFundQuotaRequest,
    };
    use crate::domain::tax::come_cotas::BasisSource;

    let db = memory_db().await;
    let ws = workspace(&db, "Estoque e come-cotas").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let fund = asset_in(&db, "FUNDOEC", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
    let quota = |date: &str, value: f64| UpsertFundQuotaRequest {
        asset_id: fund.id,
        quota_date: date.to_string(),
        value,
    };

    // 100 cotas a 10,00 em 2022; amortização de 600 a cota 15,00 (posição de 1.500, fração 0,4): o custo médio cai para 6,00
    add(&db, tx_request(pf.id, fund.id, "compra", 100.0, 10.0, "2022-06-01", 0.0)).await;
    add(&db, amortization_request(pf.id, fund.id, 600.0, 15.0, "2026-03-01")).await;
    set_fund_term_in(&db, fund.id, "longo_prazo").await.unwrap();
    upsert_fund_quota_in(&db, quota("2026-10-20", 16.0), today).await.unwrap();

    // sem estoque a base é o custo médio (6,00): ganho de 10,00 por cota
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let e = &overview.estimates[0];
    assert_eq!(e.basis_source, Some(BasisSource::Cost));
    assert!(close(e.basis_per_quota.unwrap(), 6.0) && close(e.gain, 1_000.0), "{e:?}");

    // fundo fechado com o estoque a 14,00: 4,00 por cota já tributados, que a amortização baixa junto com o custo
    // (14,00 × 0,6 = 8,40); o ganho do próximo come-cotas é (16 − 8,40) × 100 = 760 e o imposto, 15% = 114
    set_fund_structure_in(&db, fund.id, "fechado").await.unwrap();
    set_fund_estoque_in(&db, fund.id, Some("geral")).await.unwrap();
    upsert_fund_quota_in(&db, quota("2023-12-29", 14.0), today).await.unwrap();
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let e = &overview.estimates[0];
    assert_eq!(e.basis_source, Some(BasisSource::Estoque));
    assert!(close(e.basis_per_quota.unwrap(), 8.4), "{e:?}");
    assert!(close(e.gain, 760.0) && close(e.tax, 114.0), "{e:?}");
}

#[tokio::test]
async fn an_investment_entity_is_taxed_at_a_flat_15_percent_and_has_no_come_cotas() {
    use crate::commands::fund::{funds_overview_in, set_fund_term_in};
    use crate::domain::tax::come_cotas::EstimateStatus;

    let db = memory_db().await;
    let ws = workspace(&db, "Entidade").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let fund = asset_in(&db, "FIDC11", "fundo", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();

    add(&db, tx_request(pf.id, fund.id, "compra", 100.0, 10.0, "2026-03-01", 0.0)).await;
    add(&db, tx_request(pf.id, fund.id, "venda", 100.0, 12.0, "2026-03-06", 0.0)).await;
    set_fund_term_in(&db, fund.id, "entidade_investimento").await.unwrap();

    // 5 dias de aplicação: sem IOF e sem tabela por prazo, 15% de 200
    let overview = funds_overview_in(&db, ws.id, today).await.unwrap();
    let r = &overview.funds[0].redemptions[0];
    assert!(close(r.iof, 0.0) && close(r.ir, 30.0), "{r:?}");
    assert_eq!(overview.funds[0].term.as_deref(), Some("entidade_investimento"));
    // sem posição não há o que estimar; o importante é que não pede come-cotas
    assert!(overview.estimates.iter().all(|e| e.status != EstimateStatus::Estimated));
}

#[tokio::test]
async fn the_come_cotas_entry_refuses_what_is_not_a_fund_quota_deduction() {
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Come-cotas").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let fund = asset_in(&db, "FUNDO1", "fundo", "BRL").await;
    let stock = asset_in(&db, "PETR4", "acao_br", "BRL").await;
    use crate::commands::transaction::create_transaction_in;

    add(&db, tx_request(pf.id, fund.id, "compra", 10.0, 10.0, "2025-01-10", 0.0)).await;
    add(&db, tx_request(pf.id, stock.id, "compra", 10.0, 30.0, "2025-01-10", 0.0)).await;

    // só vale para fundo
    let on_stock = create_transaction_in(&db, tx_request(pf.id, stock.id, "come_cotas", 1.0, 30.0, "2026-05-29", 0.0)).await;
    assert!(matches!(on_stock, Err(AppError::InvalidGuard(_))), "ativo que não é fundo");
    // não passa do saldo
    let too_many = create_transaction_in(&db, tx_request(pf.id, fund.id, "come_cotas", 11.0, 12.0, "2026-05-29", 0.0)).await;
    assert!(matches!(too_many, Err(AppError::InvalidGuard(_))), "mais cotas que o saldo");
    // o total precisa bater com cotas × valor da cota
    let mut wrong_total = tx_request(pf.id, fund.id, "come_cotas", 1.0, 12.0, "2026-05-29", 0.0);
    wrong_total.total_value = 99.0;
    assert!(matches!(create_transaction_in(&db, wrong_total).await, Err(AppError::InvalidGuard(_))));
    // o come-cotas não tem taxa
    let with_fee = tx_request(pf.id, fund.id, "come_cotas", 1.0, 12.0, "2026-05-29", 1.0);
    assert!(matches!(create_transaction_in(&db, with_fee).await, Err(AppError::InvalidGuard(_))), "sem taxa");
    // sem ativo
    let mut no_asset = tx_request(pf.id, fund.id, "come_cotas", 1.0, 12.0, "2026-05-29", 0.0);
    no_asset.asset_id = None;
    assert!(matches!(create_transaction_in(&db, no_asset).await, Err(AppError::InvalidGuard(_))));
    // o caso válido grava
    assert!(create_transaction_in(&db, tx_request(pf.id, fund.id, "come_cotas", 1.0, 12.0, "2026-05-29", 0.0)).await.is_ok());
}

#[tokio::test]
async fn the_fund_commands_refuse_invalid_input() {
    use crate::commands::fund::{set_fund_term_in, upsert_fund_quota_in, UpsertFundQuotaRequest};
    use crate::error::AppError;

    let db = memory_db().await;
    let fund = asset_in(&db, "FUNDO1", "fundo", "BRL").await;
    let stock = asset_in(&db, "PETR4", "acao_br", "BRL").await;
    let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
    let quota = |asset_id: i32, date: &str, value: f64| {
        upsert_fund_quota_in(&db, UpsertFundQuotaRequest { asset_id, quota_date: date.to_string(), value }, today)
    };

    assert!(matches!(set_fund_term_in(&db, fund.id, "outro").await, Err(AppError::InvalidInput(_))));
    assert!(matches!(set_fund_term_in(&db, stock.id, "longo_prazo").await, Err(AppError::InvalidInput(_))), "só fundo");
    assert!(matches!(set_fund_term_in(&db, 9999, "longo_prazo").await, Err(AppError::NotFound(_))));
    assert!(set_fund_term_in(&db, fund.id, "acoes").await.is_ok());

    assert!(matches!(quota(fund.id, "2026-10-23", 10.0).await, Err(AppError::InvalidInput(_))), "data futura");
    assert!(matches!(quota(fund.id, "22/10/2026", 10.0).await, Err(AppError::InvalidInput(_))), "formato da data");
    assert!(matches!(quota(fund.id, "2026-10-20", 0.0).await, Err(AppError::InvalidInput(_))), "valor acima de zero");
    assert!(matches!(quota(fund.id, "2026-10-20", f64::NAN).await, Err(AppError::InvalidInput(_))));
    assert!(matches!(quota(stock.id, "2026-10-20", 10.0).await, Err(AppError::InvalidInput(_))), "só fundo");
    assert!(quota(fund.id, "2026-10-20", 10.0).await.is_ok());
}

#[tokio::test]
async fn a_weekend_fact_uses_the_last_ptax_before_it_and_a_long_gap_falls_back_to_the_market() {
    let db = memory_db().await;
    let ws = workspace(&db, "PTAX").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let aapl = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    seed_price(&db, "USDBRL", "2025-03-01", 5.0).await;
    seed_price(&db, "USDBRL", "2025-06-01", 5.0).await;
    // PTAX só na sexta 28/02/2025 e nada depois: o sábado 01/03 usa a sexta; 01/06 está a mais de 10 dias dela.
    seed_ptax(&db, "USD", "2025-02-28", 5.9, 6.0).await;
    trade(&db, pf.id, aapl.id, "compra", 1.0, 100.0, "2025-03-01").await;
    trade(&db, pf.id, aapl.id, "venda", 1.0, 100.0, "2025-06-01").await;

    let assessment = assess_workspace(&db, ws.id).await.unwrap();
    let sale = &assessment.foreign_years[0].sales[0];
    // custo 100 × 6,0 (PTAX da sexta); venda 100 × 5,0 (mercado) → perda de 100
    assert!(close(sale.cost, 600.0) && close(sale.sale_value, 500.0), "{sale:?}");
    assert_eq!(sale.fx_source, crate::domain::ptax::RateSource::Market);
    assert!(assessment.warnings.iter().any(|w| matches!(
        w,
        crate::domain::tax::assessment::TaxWarning::ForeignFxApproximate { year: 2025 }
    )));
}

#[tokio::test]
async fn the_foreign_rule_is_seeded_and_a_withholding_outside_a_dividend_is_refused() {
    use crate::commands::tax::load_rule_set;
    use crate::domain::tax::rules::FOREIGN_RATE;

    let db = memory_db().await;
    let rules = load_rule_set(&db).await.unwrap();
    let on = chrono::NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
    assert!(close(rules.get(FOREIGN_RATE, on).unwrap(), 0.15));

    let ws = workspace(&db, "Exterior").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let aapl = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    let mut purchase = tx_request(pf.id, aapl.id, "compra", 1.0, 100.0, "2025-01-10", 0.0);
    purchase.foreign_tax_withheld = Some(1.0);
    assert!(crate::commands::transaction::create_transaction_in(&db, purchase).await.is_err());
    let mut negative = tx_request(pf.id, aapl.id, "provento", 1.0, 100.0, "2025-01-10", 0.0);
    negative.foreign_tax_withheld = Some(-1.0);
    assert!(crate::commands::transaction::create_transaction_in(&db, negative).await.is_err());
}

#[tokio::test]
async fn the_foreign_tax_payment_date_needs_a_dividend_with_a_withholding_and_a_valid_date() {
    use crate::commands::transaction::{create_transaction_in, list_transactions_in};
    use crate::error::AppError;

    let db = memory_db().await;
    let ws = workspace(&db, "Pagamento").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let aapl = asset_in(&db, "AAPL", "acao_internacional", "USD").await;
    let dividend = |withheld: Option<f64>, paid_on: Option<&str>, kind: &str| {
        let mut request = tx_request(pf.id, aapl.id, kind, 1.0, 100.0, "2025-03-10", 0.0);
        request.foreign_tax_withheld = withheld;
        request.foreign_tax_paid_on = paid_on.map(str::to_string);
        request
    };

    // sem retenção, com retenção zero, em tipo que não é provento ou com data inválida: recusado
    for (withheld, paid_on, kind) in [
        (None, Some("2025-03-14"), "provento"),
        (Some(0.0), Some("2025-03-14"), "provento"),
        (Some(10.0), Some("2025-03-14"), "compra"),
        (Some(10.0), Some("14/03/2025"), "provento"),
        (Some(10.0), Some("2025-3-14"), "provento"),
    ] {
        let result = create_transaction_in(&db, dividend(withheld, paid_on, kind)).await;
        assert!(matches!(result, Err(AppError::InvalidGuard(_))), "{withheld:?} {paid_on:?} {kind}: {result:?}");
    }

    // provento com retenção e data válida (inclusive antes do provento) grava e volta na lista
    create_transaction_in(&db, dividend(Some(10.0), Some("2025-03-14"), "provento")).await.unwrap();
    create_transaction_in(&db, dividend(Some(10.0), Some("2025-03-01"), "provento")).await.unwrap();
    let listed = list_transactions_in(&db, pf.id).await.unwrap();
    let mut dates: Vec<_> = listed.iter().filter_map(|t| t.foreign_tax_paid_on.clone()).collect();
    dates.sort();
    assert_eq!(dates, ["2025-03-01", "2025-03-14"]);
}

#[tokio::test]
async fn a_refused_crypto_swap_leaves_nothing_behind() {
    use crate::commands::transaction::record_crypto_swap_in;
    use crate::entity::transactions;
    use sea_orm::EntityTrait;

    let db = memory_db().await;
    let ws = workspace(&db, "Swap").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let btc = asset(&db, "BTC", "cripto").await;
    let eth = asset(&db, "ETH", "cripto").await;
    let stock = asset(&db, "PETR4", "acao_br").await;
    let usd_coin = asset_in(&db, "SOL", "cripto", "USD").await;
    trade(&db, pf.id, btc.id, "compra", 1.0, 10000.0, "2026-01-02").await;

    // Vender mais do que há recusa a perna da venda, e a compra do ETH não pode ficar gravada.
    assert!(record_crypto_swap_in(&db, swap_request(pf.id, btc.id, eth.id, "2026-02-10", 5.0, 20.0, 40000.0)).await.is_err());
    assert_eq!(transactions::Entity::find().all(&db).await.unwrap().len(), 1, "só a compra inicial do BTC");

    let refused = |request| async { record_crypto_swap_in(&db, request).await.is_err() };
    assert!(refused(swap_request(pf.id, btc.id, btc.id, "2026-02-10", 1.0, 1.0, 100.0)).await, "mesma moeda");
    assert!(refused(swap_request(pf.id, btc.id, stock.id, "2026-02-10", 1.0, 1.0, 100.0)).await, "não é cripto");
    assert!(refused(swap_request(pf.id, btc.id, usd_coin.id, "2026-02-10", 1.0, 1.0, 100.0)).await, "moedas diferentes");
    assert!(refused(swap_request(pf.id, btc.id, 9999, "2026-02-10", 1.0, 1.0, 100.0)).await, "ativo inexistente");
    assert!(refused(swap_request(pf.id, btc.id, eth.id, "2026-02-10", 1.0, 1.0, 0.0)).await, "valor zero");
    assert!(refused(swap_request(pf.id, btc.id, eth.id, "2026-02-10", -1.0, 1.0, 100.0)).await, "quantidade negativa");
    assert!(refused(swap_request(pf.id, btc.id, eth.id, "10/02/2026", 1.0, 1.0, 100.0)).await, "data inválida");
    assert_eq!(transactions::Entity::find().all(&db).await.unwrap().len(), 1, "nenhuma recusa deixou lançamento");
}

#[tokio::test]
async fn the_swap_value_is_suggested_from_the_brl_close_and_only_when_it_is_recent() {
    use crate::commands::transaction::suggest_crypto_swap_value_in;

    let db = memory_db().await;
    let btc = asset(&db, "BTC", "cripto").await;
    let usd_coin = asset_in(&db, "SOL", "cripto", "USD").await;
    let stock = asset(&db, "PETR4", "acao_br").await;
    let start = chrono::NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
    seed_prices(&db, "BTC", true, start, &[100.0, 200.0, 300.0]).await; // 1, 2 e 3 de fevereiro
    seed_prices(&db, "SOL", true, start, &[10.0]).await;

    let suggest = |asset_id, date: &'static str, quantity| suggest_crypto_swap_value_in(&db, asset_id, date, quantity);
    assert_eq!(suggest(btc.id, "2026-02-02", 1.5).await.unwrap(), Some(300.0), "fechamento do próprio dia");
    assert_eq!(suggest(btc.id, "2026-02-06", 0.5).await.unwrap(), Some(150.0), "pregão anterior, dentro de 5 dias");
    assert_eq!(suggest(btc.id, "2026-02-09", 1.0).await.unwrap(), None, "fechamento velho demais");
    assert_eq!(suggest(btc.id, "2026-01-31", 1.0).await.unwrap(), None, "nada antes da série");
    assert_eq!(suggest(usd_coin.id, "2026-02-01", 1.0).await.unwrap(), None, "ativo em dólar");
    assert_eq!(suggest(stock.id, "2026-02-01", 1.0).await.unwrap(), None, "não é cripto");
    assert_eq!(suggest(btc.id, "2026-02-01", 0.0).await.unwrap(), None, "quantidade zero");
    assert_eq!(suggest(btc.id, "01/02/2026", 1.0).await.unwrap(), None, "data inválida");
    assert!(suggest(9999, "2026-02-01", 1.0).await.is_err(), "ativo inexistente");
}

#[tokio::test]
async fn the_backtest_simulates_crypto_from_the_brl_series_and_weighs_it_in_the_base_currency() {
    use crate::commands::backtest::{run_backtest_in, BacktestRequest};
    use chrono::NaiveDate;

    let db = memory_db().await;
    let ws = workspace(&db, "Crypto").await;
    let pf = portfolio(&db, ws.id, "Main").await;
    let stock = asset(&db, "AAAA3", "acao_br").await;
    let btc = asset_in(&db, "BTC", "cripto", "USD").await;
    let eth = asset_in(&db, "ETH", "cripto", "USD").await;
    seed_fx_rate(&db, "USDBRL", 5.0).await;
    // O preço atual é o último fechamento em USD semeado abaixo (US$ 100 = R$ 500 por cripto); a ação vale R$ 1.000.
    for (id, qty, total) in [(stock.id, 10.0, 1000.0), (btc.id, 1.0, 100.0), (eth.id, 1.0, 100.0)] {
        trade(&db, pf.id, id, "compra", qty, total, "2026-01-01").await;
    }
    let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    seed_prices(&db, "AAAA3", true, start, &vec![100.0; 40]).await;
    // O BTC tem a série ajustada em reais (par `-BRL`); o fechamento em USD do CoinGecko não pode entrar no lugar dela.
    seed_prices(&db, "BTC", true, start, &(0..40).map(|i| 100.0 + i as f64).collect::<Vec<_>>()).await;
    seed_prices(&db, "BTC", false, start, &vec![100.0; 40]).await;
    // O ETH só tem o fechamento em USD: sem a série em reais, sai com aviso em vez de usar o preço errado.
    seed_prices(&db, "ETH", false, start, &vec![100.0; 40]).await;

    let request = BacktestRequest {
        portfolio_id: pf.id,
        weights: vec![],
        rebalance: "never".to_string(),
        initial: 1000.0,
        monthly_contribution: 0.0,
        start_date: None,
        brokerage_per_order: 0.0,
        apply_tax: true,
    };
    let result = run_backtest_in(&db, request).await.unwrap();

    assert_eq!(result.assets.iter().map(|x| x.ticker.as_str()).collect::<Vec<_>>(), vec!["AAAA3", "BTC"]);
    // Pesos em reais: 1.000 contra 500, não 1.000 contra US$ 100.
    let weights: Vec<f64> = result.assets.iter().map(|x| x.target_weight).collect();
    assert!(close(weights[0], 2.0 / 3.0) && close(weights[1], 1.0 / 3.0), "{weights:?}");
    assert!(!result.assets[1].price_only);
    assert!(result.metrics.twr_pct > 0.0, "o BTC sobe na série em reais: {}", result.metrics.twr_pct);
    assert!(result.warnings.iter().any(|w| w.starts_with("ETH") && w.contains("no price history")), "{:?}", result.warnings);
    assert!(!result.warnings.iter().any(|w| w.starts_with("BTC") && w.contains("no tax modelled")), "cripto tem IR modelado: {:?}", result.warnings);
}

// ---------------------------------------------------------------------------
// Fechamento ajustado para o retorno total do backtest (Fase 22, fatia 1)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_adjusted_close_is_overwritten_on_each_run_because_old_adjustments_change_with_new_dividends() {
    use crate::entity::stock_adjusted_close;
    use crate::finance_api::adjusted_close::store;
    use chrono::NaiveDate;

    let db = memory_db().await;
    let day = |d: u32| NaiveDate::from_ymd_opt(2026, 1, d).unwrap();
    assert_eq!(store(&db, "PETR4", &[(day(5), 9.0), (day(6), 9.5)], "2026-01-10T00:00:00Z").await.unwrap(), 2);
    store(&db, "VALE3", &[(day(5), 50.0)], "2026-01-10T00:00:00Z").await.unwrap();

    // Um dividendo novo reajusta os pregões antigos: a segunda coleta regrava os mesmos dias com o valor novo
    // e acrescenta o dia que faltava, sem duplicar nem mexer em outro ticker.
    store(&db, "PETR4", &[(day(5), 8.8), (day(6), 9.3), (day(7), 9.6)], "2026-01-11T00:00:00Z").await.unwrap();
    let rows = stock_adjusted_close::Entity::find().all(&db).await.unwrap();
    assert_eq!(rows.len(), 4, "3 de PETR4 + 1 de VALE3");
    let petr = |d: &str| rows.iter().find(|r| r.ticker == "PETR4" && r.price_date == d).unwrap();
    assert!(close(petr("2026-01-05").adj_close_price, 8.8) && close(petr("2026-01-06").adj_close_price, 9.3));
    assert!(close(rows.iter().find(|r| r.ticker == "VALE3").unwrap().adj_close_price, 50.0));
}
