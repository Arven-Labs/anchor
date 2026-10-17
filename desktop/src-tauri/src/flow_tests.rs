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
    default_thresholds, ScoreVerdict, BTC_DEFAULT_THRESHOLDS, ETH_DEFAULT_THRESHOLDS,
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
    assert_eq!(thresholds.len(), ETH_DEFAULT_THRESHOLDS.len() + BTC_DEFAULT_THRESHOLDS.len());
}

#[tokio::test]
async fn seeded_thresholds_match_the_reset_defaults() {
    // Os defaults por moeda (o Reset do editor) duplicam os seeds das migrations;
    // este teste falha se um mudar sem o outro. As linhas do ETH que existiam
    // antes da coluna `coin` viraram `coin = 'ETH'` na migration.
    let db = memory_db().await;
    let rows = indicator_thresholds::Entity::find().all(&db).await.unwrap();

    for coin in ["ETH", "BTC"] {
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
    assert_eq!(score.total, 17);
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
    assert_eq!((btc.total, btc.logged), (12, 2));
    assert_eq!(btc.groups.len(), 3); // sem Network fundamentals
    let derivatives = btc.groups.iter().find(|g| g.label == "Derivatives & options").unwrap();
    assert_eq!(derivatives.indicators[0].reading.as_ref().unwrap().signal, "RED");

    let eth = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!((eth.total, eth.logged), (17, 1));
    let derivatives = eth.groups.iter().find(|g| g.label == "Derivatives & options").unwrap();
    assert_eq!(derivatives.indicators[0].reading.as_ref().unwrap().signal, "NEUTRAL");
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

    // Na base USD só o preço andou.
    save_base_currency(&db, ws.id, "USD").await.unwrap();
    let (in_usd, _) = positions_in_base(&db, pf.id).await.unwrap();
    assert!(close(in_usd[0].unrealized_pl.unwrap(), 20.0));
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
    };
    assert_eq!(update_bank_account_in(&db, update("EUR")).await.unwrap().currency, "EUR");
    assert!(update_bank_account_in(&db, update("VND")).await.is_err());

    cash_in(&db, ws.id, account.id, 10.0, "2026-01-01").await;
    assert!(update_bank_account_in(&db, update("GBP")).await.is_err());
    // Repetir a mesma moeda não conta como troca.
    assert_eq!(update_bank_account_in(&db, update("EUR")).await.unwrap().currency, "EUR");
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
