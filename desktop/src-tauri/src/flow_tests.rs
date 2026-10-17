// Testes de fluxo: cada um percorre uma funcionalidade de ponta a ponta contra
// o schema **real** (as migrations do app, com seeds, num SQLite em memória),
// chamando as mesmas funções que os comandos Tauri usam. Pegam o que os testes
// de lógica pura não pegam: coluna que a migration não criou, seed faltando,
// lançamento de caixa que não bate, regra fiscal semeada diferente do esperado.

use sea_orm::EntityTrait;

use crate::commands::crypto_indicator::{
    apply_threshold, crypto_score_for, record_crypto_indicator_in, RecordCryptoIndicatorRequest,
};
use crate::commands::loan::{
    create_borrower_in, create_loan_in, delete_loan_in, list_loans_view, register_loan_repayment_in,
    update_loan_in, CreateBorrowerRequest, CreateLoanRequest, RegisterLoanRepaymentRequest,
    UpdateLoanRequest,
};
use crate::commands::net_worth::compute_net_worth;
use crate::commands::tax::{assess_workspace, simulate_tax_sale_in, SimulateTaxSaleRequest};
use crate::domain::crypto_score::{ScoreVerdict, DEFAULT_THRESHOLDS};
use crate::entity::{indicator_thresholds, tax_rules};
use crate::test_support::{asset, bank_account, cash_in, memory_db, portfolio, trade, workspace};

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
    assert_eq!(thresholds.len(), DEFAULT_THRESHOLDS.len());
}

#[tokio::test]
async fn seeded_thresholds_match_the_reset_defaults() {
    // `DEFAULT_THRESHOLDS` (o Reset do editor) duplica os seeds das migrations;
    // este teste falha se um mudar sem o outro.
    let db = memory_db().await;
    let rows = indicator_thresholds::Entity::find().all(&db).await.unwrap();

    for (indicator, green, red) in DEFAULT_THRESHOLDS {
        let row = rows
            .iter()
            .find(|r| r.indicator == indicator)
            .unwrap_or_else(|| panic!("no seeded threshold for {indicator}"));
        assert!(close(row.green_boundary, green), "{indicator} green");
        assert!(close(row.red_boundary, red), "{indicator} red");
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
    apply_threshold(&db, "funding_rate_7d", 15.0, 30.0).await.unwrap();
    let score = crypto_score_for(&db, "ETH".to_string()).await.unwrap();
    assert_eq!(score.green, 2);
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
        },
    )
    .await
    .unwrap();
    record_crypto_indicator_in(&source, reading("funding_rate_7d", 7.5)).await.unwrap();

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
    assert_eq!(target_provider.export_all().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(source_dir);
    let _ = std::fs::remove_dir_all(target_dir);
}
