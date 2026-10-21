// Fase 24 — IR sobre renda variável. A apuração é sempre recalculada a partir
// de `transactions` (nunca persistida), no nível do Workspace inteiro — o
// imposto é por CPF, então somam-se as vendas de todas as carteiras dele.
// Os parâmetros (alíquotas, limites…) vêm de `tax_rules`: seed da migration,
// atualizável pela tabela embarcada no app (`sync_tax_rules`) e editável pelo
// usuário — edição do usuário nunca é sobrescrita pela sincronização.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait, Unchanged,
};
use serde::{Deserialize, Serialize};

use crate::commands::transaction::{split_ratio, BONUS, REVERSE_SPLIT, SPLIT};
use crate::commands::currency::load_fx_history;
use crate::domain::option_position as option_calc;
use crate::domain::tax::assessment::{self, Assessment, OptionResult, Side, Trade};
use crate::domain::tax::classify::{treatment, TaxTreatment};
use crate::domain::currency_conversion::FxHistory;
use crate::domain::ptax::{PtaxSeries, RateKind, RateSource};
use crate::domain::tax::foreign;
use crate::domain::tax::rules::{self, Rule, RuleSet};
use crate::entity::{
    assets, bank_account, general_transaction, general_transaction_category, option_position,
    portfolio, tax_payments, tax_rules, transactions,
};
use crate::error::AppError;
use crate::finance_api::{client, ptax_store};

const BUY: &str = "compra";
const SELL: &str = "venda";
const DIVIDEND: &str = "provento";
const DESPESA: &str = "despesa";
const TAXES_CATEGORY: &str = "Taxes";
const YIELD_CATEGORY: &str = "Yield";
const EPS: f64 = 1e-9;

fn parse_date(value: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value.get(0..10).unwrap_or(value), "%Y-%m-%d")
        .map_err(|_| AppError::InvalidGuard(format!("invalid date '{value}'")))
}

pub async fn load_rule_set(db: &DatabaseConnection) -> Result<RuleSet, AppError> {
    let rows = tax_rules::Entity::find().all(db).await?;
    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        rules.push(Rule {
            effective_from: parse_date(&row.effective_from)?,
            key: row.rule_key,
            value: row.value,
        });
    }
    Ok(RuleSet::new(rules))
}

/// IRRF do JCP vigente na data do pagamento (`YYYY-MM-DD`).
pub fn jcp_rate_on(rules: &RuleSet, date: &str) -> Result<f64, AppError> {
    rules.get(rules::JCP_IRRF_RATE, parse_date(date)?)
}

async fn workspace_portfolio_ids(db: &DatabaseConnection, workspace_id: i32) -> Result<Vec<i32>, AppError> {
    Ok(portfolio::Entity::find()
        .filter(portfolio::Column::WorkspaceId.eq(workspace_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.id)
        .collect())
}

/// Compras e vendas de todas as carteiras do Workspace, já resolvidas com os
/// dados do ativo (transação sem ativo é ignorada).
async fn load_trades(db: &DatabaseConnection, workspace_id: i32) -> Result<Vec<Trade>, AppError> {
    let portfolio_ids = workspace_portfolio_ids(db, workspace_id).await?;
    if portfolio_ids.is_empty() {
        return Ok(Vec::new());
    }

    let txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.is_in(portfolio_ids))
        .filter(transactions::Column::TransactionType.is_in([BUY, SELL, SPLIT, REVERSE_SPLIT, BONUS]))
        .all(db)
        .await?;

    let asset_ids: HashSet<i32> = txs.iter().filter_map(|t| t.asset_id).collect();
    let assets_map: HashMap<i32, assets::Model> = if asset_ids.is_empty() {
        HashMap::new()
    } else {
        assets::Entity::find()
            .filter(assets::Column::Id.is_in(asset_ids.into_iter().collect::<Vec<i32>>()))
            .all(db)
            .await?
            .into_iter()
            .map(|a| (a.id, a))
            .collect()
    };

    let mut trades = Vec::with_capacity(txs.len());
    for tx in &txs {
        let Some(asset) = tx.asset_id.and_then(|id| assets_map.get(&id)) else {
            continue;
        };
        trades.push(Trade {
            tx_id: tx.id,
            portfolio_id: tx.portfolio_id,
            asset_id: asset.id,
            ticker: asset.ticker.clone(),
            asset_class: asset.asset_class.clone(),
            currency: asset.currency.clone(),
            custodia_id: tx.custodia_id,
            date: parse_date(&tx.transaction_date)?,
            side: match tx.transaction_type.as_str() {
                BUY => Side::Buy,
                SPLIT | REVERSE_SPLIT => Side::Split,
                BONUS => Side::Bonus,
                _ => Side::Sell,
            },
            quantity: match tx.transaction_type.as_str() {
                SPLIT | REVERSE_SPLIT => split_ratio(&tx.transaction_type, tx.quantity.unwrap_or(0.0)),
                _ => tx.quantity.unwrap_or(0.0),
            },
            gross_value: tx.total_value,
            fee: tx.fee.unwrap_or(0.0),
        });
    }

    convert_crypto_to_brl(db, &mut trades).await?;
    Ok(trades)
}

/// O IR de cripto é em reais, mas a cripto nasce em USD (a cotação do CoinGecko é em dólar). Converte valor e taxa de
/// cada operação pelo USDBRL da data dela; sem taxa, a operação fica em USD e a apuração só avisa.
async fn convert_crypto_to_brl(db: &DatabaseConnection, trades: &mut [Trade]) -> Result<(), AppError> {
    let is_usd_crypto = |t: &Trade| treatment(&t.asset_class, &t.ticker) == TaxTreatment::Crypto && t.currency == "USD";
    if !trades.iter().any(is_usd_crypto) {
        return Ok(());
    }
    let fx = load_fx_history(db, "BRL", &HashSet::from(["USD".to_string()])).await?;
    for trade in trades.iter_mut().filter(|t| is_usd_crypto(t)) {
        if let Some(rate) = fx.rate_on("USD", "BRL", trade.date) {
            trade.gross_value *= rate;
            trade.fee *= rate;
            trade.currency = "BRL".to_string();
        }
    }
    Ok(())
}

/// Posições em opções **encerradas** das carteiras do Workspace (Fase 18), no
/// formato da apuração. Posição aberta não entra: só vira resultado ao encerrar.
/// Day trade = aberta e encerrada no mesmo dia.
async fn load_option_results(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Vec<OptionResult>, AppError> {
    let portfolio_ids = workspace_portfolio_ids(db, workspace_id).await?;
    if portfolio_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = option_position::Entity::find()
        .filter(option_position::Column::PortfolioId.is_in(portfolio_ids))
        .filter(option_position::Column::CloseDate.is_not_null())
        .all(db)
        .await?;

    let mut results = Vec::with_capacity(rows.len());
    for row in rows {
        let (Some(close_date), Some(close_price)) = (row.close_date.as_deref(), row.close_price) else {
            continue;
        };
        let (proceeds, cost) = option_calc::proceeds_and_cost(
            &row.side,
            row.premium,
            row.quantity,
            close_price,
            row.open_fee,
            row.close_fee,
        );
        results.push(OptionResult {
            position_id: row.id,
            portfolio_id: row.portfolio_id,
            ticker: row.series_ticker.clone(),
            date: parse_date(close_date)?,
            day_trade: row.open_date == close_date,
            quantity: row.quantity,
            proceeds,
            cost,
            exercised: row.close_kind.as_deref() == Some(option_calc::EXERCISED),
        });
    }
    Ok(results)
}

pub(crate) async fn assess_workspace(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Assessment, AppError> {
    let rules = load_rule_set(db).await?;
    let trades = load_trades(db, workspace_id).await?;
    let options = load_option_results(db, workspace_id).await?;
    let mut assessment = assessment::assess_with_options(&trades, &options, &rules)?;

    let dividends = load_foreign_dividends(db, workspace_id).await?;
    let currencies: HashSet<String> = foreign_currencies(&trades)
        .into_iter()
        .chain(dividends.iter().map(|d| d.currency.clone()).filter(|c| c != "BRL"))
        .collect();
    let fx = ForeignFx::load(db, &currencies).await?;
    let (years, warnings) =
        foreign::assess_foreign(&trades, &dividends, &rules, &|currency, date, kind| fx.rate(currency, date, kind))?;
    assessment.foreign_years = years;
    assessment.warnings.extend(warnings);
    Ok(assessment)
}

/// Moedas (fora BRL) dos ativos do exterior nas operações.
fn foreign_currencies(trades: &[Trade]) -> HashSet<String> {
    trades
        .iter()
        .filter(|t| treatment(&t.asset_class, &t.ticker) == TaxTreatment::ForeignWarning)
        .map(|t| t.currency.clone())
        .filter(|c| c != "BRL")
        .collect()
}

/// Câmbio do IR de exterior: a PTAX do Banco Central (gravada por `refresh_ptax`) e, onde ela falta, o câmbio de mercado.
struct ForeignFx {
    ptax: PtaxSeries,
    market: FxHistory,
}

impl ForeignFx {
    async fn load(db: &DatabaseConnection, currencies: &HashSet<String>) -> Result<Self, AppError> {
        Ok(Self {
            ptax: ptax_store::stored(db, currencies).await?,
            market: load_fx_history(db, "BRL", currencies).await?,
        })
    }

    fn rate(&self, currency: &str, date: NaiveDate, kind: RateKind) -> Option<(f64, RateSource)> {
        self.ptax
            .rate(currency, date, kind)
            .map(|rate| (rate, RateSource::Ptax))
            .or_else(|| self.market.rate_on(currency, "BRL", date).map(|rate| (rate, RateSource::Market)))
    }
}

/// Uma linha do informe de Bens e Direitos do exterior: a posição de um ativo num 31/12, com o nome e a bolsa do ativo.
#[derive(Serialize)]
pub struct ForeignHoldingView {
    #[serde(flatten)]
    pub holding: foreign::ForeignHolding,
    pub name: String,
    pub exchange: Option<String>,
}

/// Posição de cada ativo do exterior em cada 31/12 (de 2023 até o ano corrente), com custo médio em reais. Só lê o
/// banco: a PTAX é gravada por `get_tax_assessment` (a aba Taxes consulta este comando depois dele).
#[tauri::command]
pub async fn get_foreign_holdings(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<Vec<ForeignHoldingView>, AppError> {
    foreign_holdings_in(db.inner(), workspace_id).await
}

pub async fn foreign_holdings_in(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Vec<ForeignHoldingView>, AppError> {
    let trades = load_trades(db, workspace_id).await?;
    let fx = ForeignFx::load(db, &foreign_currencies(&trades)).await?;
    let holdings = foreign::foreign_holdings(&trades, &|currency, date, kind| fx.rate(currency, date, kind), Utc::now().year());

    let asset_ids: Vec<i32> = holdings.iter().map(|h| h.asset_id).collect::<HashSet<_>>().into_iter().collect();
    let assets_map: HashMap<i32, assets::Model> = if asset_ids.is_empty() {
        HashMap::new()
    } else {
        assets::Entity::find()
            .filter(assets::Column::Id.is_in(asset_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|a| (a.id, a))
            .collect()
    };
    Ok(holdings
        .into_iter()
        .map(|holding| {
            let asset = assets_map.get(&holding.asset_id);
            ForeignHoldingView {
                name: asset.map(|a| a.name.clone()).unwrap_or_else(|| holding.ticker.clone()),
                exchange: asset.and_then(|a| a.exchange.clone()),
                holding,
            }
        })
        .collect())
}

const HOLDINGS_CSV_HEADER: [&str; 9] =
    ["year", "ticker", "name", "exchange", "currency", "quantity", "cost_brl", "fx_approximate", "incomplete"];

/// CSV do informe: uma linha por ativo e 31/12, com o custo em reais (duas casas) e a quantidade como está.
pub fn foreign_holdings_csv_text(rows: &[ForeignHoldingView]) -> Result<String, AppError> {
    let csv_error = |e: csv::Error| AppError::InvalidInput(format!("CSV: {e}"));
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(HOLDINGS_CSV_HEADER).map_err(csv_error)?;
    for row in rows {
        let h = &row.holding;
        writer
            .write_record([
                h.year.to_string().as_str(),
                h.ticker.as_str(),
                row.name.as_str(),
                row.exchange.as_deref().unwrap_or(""),
                h.currency.as_str(),
                h.quantity.to_string().as_str(),
                format!("{:.2}", h.cost_brl).as_str(),
                h.fx_approximate.to_string().as_str(),
                h.incomplete.to_string().as_str(),
            ])
            .map_err(csv_error)?;
    }
    let bytes = writer.into_inner().map_err(|e| AppError::InvalidInput(format!("CSV: {e}")))?;
    String::from_utf8(bytes).map_err(|e| AppError::InvalidInput(format!("CSV: {e}")))
}

/// Grava o informe em `dest_path` (o front escolhe o arquivo no diálogo de salvar) e devolve quantas linhas gravou.
#[tauri::command]
pub async fn export_foreign_holdings_csv(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
    dest_path: String,
) -> Result<usize, AppError> {
    let rows = foreign_holdings_in(db.inner(), workspace_id).await?;
    let text = foreign_holdings_csv_text(&rows)?;
    std::fs::write(&dest_path, text)
        .map_err(|e| AppError::InvalidInput(format!("não consegui gravar {dest_path}: {e}")))?;
    Ok(rows.len())
}

/// Completa a PTAX gravada (`ptax_daily`) para as moedas dos ativos do exterior, do primeiro ao último fato de cada
/// uma (as compras antigas formam o custo médio). Vai ao BCB só com `allow_network`; falha vai para o log e nunca
/// derruba quem chama: a apuração segue com a PTAX gravada e o câmbio de mercado de reserva.
async fn refresh_ptax(db: &DatabaseConnection, workspace_id: i32, allow_network: bool) {
    if !allow_network {
        return;
    }
    let result: Result<(), AppError> = async {
        let trades = load_trades(db, workspace_id).await?;
        let dividends = load_foreign_dividends(db, workspace_id).await?;
        let mut span: HashMap<String, (NaiveDate, NaiveDate)> = HashMap::new();
        let facts = trades
            .iter()
            .filter(|t| treatment(&t.asset_class, &t.ticker) == TaxTreatment::ForeignWarning)
            .map(|t| (t.currency.clone(), t.date))
            .chain(dividends.iter().map(|d| (d.currency.clone(), d.date)));
        for (currency, date) in facts.filter(|(currency, _)| currency != "BRL") {
            let entry = span.entry(currency).or_insert((date, date));
            entry.0 = entry.0.min(date);
            entry.1 = entry.1.max(date);
        }
        for (currency, (since, until)) in span {
            ptax_store::refresh(db, &currency, since, until).await?;
        }
        Ok(())
    }
    .await;
    if let Err(err) = result {
        eprintln!("PTAX: using what is stored after a refresh failure: {err}");
    }
}

/// Proventos dos ativos do exterior de todas as carteiras do Workspace, na moeda do ativo, com a retenção lá fora.
async fn load_foreign_dividends(
    db: &DatabaseConnection,
    workspace_id: i32,
) -> Result<Vec<foreign::ForeignDividend>, AppError> {
    let portfolio_ids = workspace_portfolio_ids(db, workspace_id).await?;
    if portfolio_ids.is_empty() {
        return Ok(Vec::new());
    }
    let txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.is_in(portfolio_ids))
        .filter(transactions::Column::TransactionType.eq(DIVIDEND))
        .filter(transactions::Column::AssetId.is_not_null())
        .all(db)
        .await?;
    let asset_ids: Vec<i32> = txs.iter().filter_map(|t| t.asset_id).collect::<HashSet<_>>().into_iter().collect();
    if asset_ids.is_empty() {
        return Ok(Vec::new());
    }
    let assets_map: HashMap<i32, assets::Model> = assets::Entity::find()
        .filter(assets::Column::Id.is_in(asset_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();

    let mut dividends = Vec::new();
    for tx in &txs {
        let Some(asset) = tx.asset_id.and_then(|id| assets_map.get(&id)) else {
            continue;
        };
        if treatment(&asset.asset_class, &asset.ticker) != TaxTreatment::ForeignWarning {
            continue;
        }
        dividends.push(foreign::ForeignDividend {
            tx_id: tx.id,
            asset_id: asset.id,
            ticker: asset.ticker.clone(),
            currency: asset.currency.clone(),
            date: parse_date(&tx.transaction_date)?,
            gross_value: tx.total_value,
            withheld: tx.foreign_tax_withheld.unwrap_or(0.0),
        });
    }
    Ok(dividends)
}

#[derive(Deserialize)]
pub struct SimulateTaxSaleRequest {
    pub workspace_id: i32,
    pub asset_id: i32,
    pub quantity: f64,
    pub unit_price: f64,
    pub fee: Option<f64>,
    /// `YYYY-MM-DD`
    pub date: String,
}

/// Simula uma venda sem gravar nada: mostra quanto de prejuízo ela consome,
/// o imposto que geraria e o efeito nos meses seguintes. Só ativos do escopo
/// B3 (ações, units, ETF, BDR, FII).
#[tauri::command]
pub async fn simulate_tax_sale(
    db: tauri::State<'_, DatabaseConnection>,
    request: SimulateTaxSaleRequest,
) -> Result<assessment::SaleSimulation, AppError> {
    simulate_tax_sale_in(db.inner(), request).await
}

pub async fn simulate_tax_sale_in(
    db: &DatabaseConnection,
    request: SimulateTaxSaleRequest,
) -> Result<assessment::SaleSimulation, AppError> {
    if !request.quantity.is_finite() || request.quantity <= 0.0 {
        return Err(AppError::InvalidGuard("quantity must be greater than zero".to_string()));
    }
    if !request.unit_price.is_finite() || request.unit_price <= 0.0 {
        return Err(AppError::InvalidGuard("price must be greater than zero".to_string()));
    }
    let fee = request.fee.unwrap_or(0.0);
    if !fee.is_finite() || fee < 0.0 {
        return Err(AppError::InvalidGuard("fee cannot be negative".to_string()));
    }
    let date = parse_date(&request.date)?;

    let asset = assets::Entity::find_by_id(request.asset_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {}", request.asset_id)))?;
    if !matches!(
        treatment(&asset.asset_class, &asset.ticker),
        TaxTreatment::StockExemptEligible | TaxTreatment::CommonNoExemption | TaxTreatment::Fii
    ) {
        return Err(AppError::InvalidInput(format!(
            "{} is outside the B3 capital gains scope Anchor calculates",
            asset.ticker
        )));
    }

    let rules = load_rule_set(db).await?;
    let trades = load_trades(db, request.workspace_id).await?;
    // Carteira da venda: a da última operação do ativo (a posição que
    // provavelmente seria vendida).
    let portfolio_id = trades
        .iter()
        .filter(|t| t.asset_id == asset.id)
        .max_by_key(|t| (t.date, t.tx_id))
        .map(|t| t.portfolio_id)
        .ok_or_else(|| AppError::InvalidGuard("this asset has no recorded purchases to sell".to_string()))?;

    let sale = Trade {
        tx_id: assessment::SIMULATED_TX_ID,
        portfolio_id,
        asset_id: asset.id,
        ticker: asset.ticker,
        asset_class: asset.asset_class,
        currency: asset.currency,
        custodia_id: None,
        date,
        side: Side::Sell,
        quantity: request.quantity,
        gross_value: request.quantity * request.unit_price,
        fee,
    };
    let options = load_option_results(db, request.workspace_id).await?;
    assessment::simulate_sale_with_options(&trades, &options, &rules, sale)
}

// ---------------------------------------------------------------------------
// Regras
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_tax_rules(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<tax_rules::Model>, AppError> {
    Ok(tax_rules::Entity::find()
        .order_by_asc(tax_rules::Column::RuleKey)
        .order_by_asc(tax_rules::Column::EffectiveFrom)
        .all(db.inner())
        .await?)
}

async fn find_rule(db: &DatabaseConnection, id: i32) -> Result<tax_rules::Model, AppError> {
    tax_rules::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("tax rule {id}")))
}

#[tauri::command]
pub async fn update_tax_rule(
    db: tauri::State<'_, DatabaseConnection>,
    id: i32,
    value: f64,
) -> Result<tax_rules::Model, AppError> {
    if !value.is_finite() || value < 0.0 {
        return Err(AppError::InvalidInput(format!("invalid tax rule value {value}")));
    }
    let existing = find_rule(db.inner(), id).await?;
    Ok(tax_rules::ActiveModel {
        id: Unchanged(existing.id),
        value: Set(value),
        user_edited: Set((value - existing.default_value).abs() > EPS),
        updated_at: Set(Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .update(db.inner())
    .await?)
}

#[tauri::command]
pub async fn reset_tax_rule(
    db: tauri::State<'_, DatabaseConnection>,
    id: i32,
) -> Result<tax_rules::Model, AppError> {
    let existing = find_rule(db.inner(), id).await?;
    Ok(tax_rules::ActiveModel {
        id: Unchanged(existing.id),
        value: Set(existing.default_value),
        user_edited: Set(false),
        updated_at: Set(Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .update(db.inner())
    .await?)
}

#[derive(Serialize)]
pub struct TaxRuleConflict {
    pub id: i32,
    pub rule_key: String,
    pub effective_from: String,
    pub your_value: f64,
    pub official_value: f64,
}

#[derive(Serialize)]
pub struct TaxRuleSyncResult {
    pub added: u32,
    pub updated: u32,
    /// Regras editadas pelo usuário cujo valor oficial mudou na tabela embarcada
    /// — não são tocadas; a UI oferece "reset" pra aplicar o valor novo.
    pub conflicts: Vec<TaxRuleConflict>,
}

#[tauri::command]
pub async fn sync_tax_rules(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<TaxRuleSyncResult, AppError> {
    let db = db.inner();
    let response = client::fetch_tax_rules_br().await?;
    let existing: HashMap<(String, String), tax_rules::Model> = tax_rules::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|r| ((r.rule_key.clone(), r.effective_from.clone()), r))
        .collect();

    let now = Utc::now().to_rfc3339();
    let mut result = TaxRuleSyncResult { added: 0, updated: 0, conflicts: Vec::new() };

    for rule in response.rules {
        let effective_from = rule.effective_from.format("%Y-%m-%d").to_string();
        let verified_at = rule.verified_at.format("%Y-%m-%d").to_string();
        match existing.get(&(rule.key.clone(), effective_from.clone())) {
            None => {
                tax_rules::ActiveModel {
                    rule_key: Set(rule.key),
                    effective_from: Set(effective_from),
                    value: Set(rule.value),
                    default_value: Set(rule.value),
                    description: Set(rule.description),
                    source: Set(rule.source),
                    verified_at: Set(verified_at),
                    user_edited: Set(false),
                    updated_at: Set(now.clone()),
                    ..Default::default()
                }
                .insert(db)
                .await?;
                result.added += 1;
            }
            Some(row) => {
                let official_changed = (row.default_value - rule.value).abs() > EPS;
                let keep_user_value = row.user_edited;
                if keep_user_value && (row.value - rule.value).abs() > EPS {
                    result.conflicts.push(TaxRuleConflict {
                        id: row.id,
                        rule_key: row.rule_key.clone(),
                        effective_from: row.effective_from.clone(),
                        your_value: row.value,
                        official_value: rule.value,
                    });
                }
                if official_changed && !keep_user_value {
                    result.updated += 1;
                }
                tax_rules::ActiveModel {
                    id: Unchanged(row.id),
                    value: Set(if keep_user_value { row.value } else { rule.value }),
                    default_value: Set(rule.value),
                    description: Set(rule.description),
                    source: Set(rule.source),
                    verified_at: Set(verified_at),
                    updated_at: Set(now.clone()),
                    ..Default::default()
                }
                .update(db)
                .await?;
            }
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Apuração e pagamento
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct TaxAssessmentView {
    pub assessment: Assessment,
    pub payments: Vec<tax_payments::Model>,
}

#[tauri::command]
pub async fn get_tax_assessment(
    db: tauri::State<'_, DatabaseConnection>,
    workspace_id: i32,
) -> Result<TaxAssessmentView, AppError> {
    let db = db.inner();
    refresh_ptax(db, workspace_id, std::env::var_os(crate::ingestion::DISABLE_ENV).is_none()).await;
    let assessment = assess_workspace(db, workspace_id).await?;
    let payments = tax_payments::Entity::find()
        .filter(tax_payments::Column::WorkspaceId.eq(workspace_id))
        .order_by_asc(tax_payments::Column::Period)
        .all(db)
        .await?;
    Ok(TaxAssessmentView { assessment, payments })
}

#[derive(Deserialize)]
pub struct MarkTaxPaidRequest {
    pub workspace_id: i32,
    pub period: String,
    pub amount: f64,
    pub paid_date: String,
    pub bank_account_id: i32,
    /// `b3` (padrão) ou `crypto`.
    #[serde(default = "default_payment_kind")]
    pub kind: String,
}

pub const PAYMENT_KIND_B3: &str = "b3";
pub const PAYMENT_KIND_CRYPTO: &str = "crypto";

fn default_payment_kind() -> String {
    PAYMENT_KIND_B3.to_string()
}

#[tauri::command]
pub async fn mark_tax_paid(
    db: tauri::State<'_, DatabaseConnection>,
    request: MarkTaxPaidRequest,
) -> Result<tax_payments::Model, AppError> {
    mark_tax_paid_in(db.inner(), request).await
}

/// Categoria "Taxes" do Workspace, criada se ainda não existir (DARF e IR retido
/// de renda fixa caem nela).
pub(crate) async fn ensure_taxes_category<C: sea_orm::ConnectionTrait>(
    db: &C,
    workspace_id: i32,
) -> Result<i32, AppError> {
    ensure_category(db, workspace_id, TAXES_CATEGORY).await
}

/// Categoria "Yield" do Workspace, criada se ainda não existir (rendimento de renda fixa e
/// a receita da conciliação pelo saldo caem nela).
pub(crate) async fn ensure_yield_category<C: sea_orm::ConnectionTrait>(
    db: &C,
    workspace_id: i32,
) -> Result<i32, AppError> {
    ensure_category(db, workspace_id, YIELD_CATEGORY).await
}

async fn ensure_category<C: sea_orm::ConnectionTrait>(
    db: &C,
    workspace_id: i32,
    name: &str,
) -> Result<i32, AppError> {
    if let Some(category) = general_transaction_category::Entity::find()
        .filter(general_transaction_category::Column::WorkspaceId.eq(workspace_id))
        .filter(general_transaction_category::Column::Nome.eq(name))
        .one(db)
        .await?
    {
        return Ok(category.id);
    }
    Ok(general_transaction_category::ActiveModel {
        workspace_id: Set(workspace_id),
        nome: Set(name.to_string()),
        limite_mensal: Set(None),
        created_at: Set(Utc::now().to_rfc3339()),
        ..Default::default()
    }
    .insert(db)
    .await?
    .id)
}

pub async fn mark_tax_paid_in(
    db: &DatabaseConnection,
    request: MarkTaxPaidRequest,
) -> Result<tax_payments::Model, AppError> {
    if request.amount <= 0.0 || !request.amount.is_finite() {
        return Err(AppError::InvalidInput("amount must be positive".to_string()));
    }
    parse_date(&request.paid_date)?;
    parse_date(&format!("{}-01", request.period))?;
    let note_kind = match request.kind.as_str() {
        PAYMENT_KIND_B3 => "stock exchange",
        PAYMENT_KIND_CRYPTO => "crypto",
        other => return Err(AppError::InvalidInput(format!("payment kind must be b3 or crypto, got '{other}'"))),
    };

    let account = bank_account::Entity::find_by_id(request.bank_account_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("bank account {}", request.bank_account_id)))?;
    if account.workspace_id != request.workspace_id {
        return Err(AppError::InvalidGuard("bank account belongs to another workspace".to_string()));
    }
    // O DARF é sempre em reais; numa conta em outra moeda o débito sairia no
    // valor errado.
    if account.currency != "BRL" {
        return Err(AppError::InvalidInput(format!(
            "the tax is paid in BRL, but the account is in {}",
            account.currency
        )));
    }

    let now = Utc::now().to_rfc3339();
    let txn = db.begin().await?;

    let category_id = ensure_taxes_category(&txn, request.workspace_id).await?;

    let expense = general_transaction::ActiveModel {
        workspace_id: Set(request.workspace_id),
        bank_account_id: Set(request.bank_account_id),
        transaction_type: Set(DESPESA.to_string()),
        category_id: Set(Some(category_id)),
        valor: Set(request.amount),
        transaction_date: Set(request.paid_date.clone()),
        notes: Set(Some(format!("DARF — capital gains tax ({note_kind}), period {}", request.period))),
        created_at: Set(now.clone()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    let payment = tax_payments::ActiveModel {
        workspace_id: Set(request.workspace_id),
        period: Set(request.period),
        amount: Set(request.amount),
        paid_date: Set(request.paid_date),
        kind: Set(request.kind),
        general_transaction_id: Set(Some(expense.id)),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;
    Ok(payment)
}

/// Desfaz o "pago": apaga o registro e a despesa criada junto em Finanças Gerais.
#[tauri::command]
pub async fn unmark_tax_paid(
    db: tauri::State<'_, DatabaseConnection>,
    payment_id: i32,
) -> Result<(), AppError> {
    let db = db.inner();
    let payment = tax_payments::Entity::find_by_id(payment_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("tax payment {payment_id}")))?;

    let txn = db.begin().await?;
    tax_payments::Entity::delete_by_id(payment.id).exec(&txn).await?;
    if let Some(gt_id) = payment.general_transaction_id {
        general_transaction::Entity::delete_by_id(gt_id).exec(&txn).await?;
    }
    txn.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Rentabilidade líquida de imposto
// ---------------------------------------------------------------------------

/// Imposto pago que cabe às carteiras/ativo pedidos, como (data do
/// pagamento, valor). Cada DARF é rateado entre as vendas do mês apurado,
/// pró-rata do ganho tributável de cada uma (lucro isento e prejuízo não
/// geram imposto, então não recebem rateio). Valor acumulado de meses
/// anteriores (abaixo do mínimo) é rateado pelas vendas do mês do DARF —
/// aproximação aceita.
pub(crate) async fn tax_cost_flows(
    db: &DatabaseConnection,
    portfolio_ids: &[i32],
    asset_id: Option<i32>,
) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let workspace_ids: HashSet<i32> = portfolio::Entity::find()
        .filter(portfolio::Column::Id.is_in(portfolio_ids.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.workspace_id)
        .collect();

    let mut flows = Vec::new();
    for workspace_id in workspace_ids {
        let payments = tax_payments::Entity::find()
            .filter(tax_payments::Column::WorkspaceId.eq(workspace_id))
            .all(db)
            .await?;
        if payments.is_empty() {
            continue;
        }
        let assessment = assess_workspace(db, workspace_id).await?;
        for payment in payments {
            // Peso de cada venda do mês apurado: (carteira, ativo, ganho tributável). A B3 e a cripto têm DARF e
            // apuração próprios, então cada pagamento só rateia entre as vendas do seu tipo.
            let weights: Vec<(i32, i32, f64)> = if payment.kind == PAYMENT_KIND_CRYPTO {
                let Some(month) = assessment.crypto_months.iter().find(|m| m.period == payment.period) else {
                    continue;
                };
                month
                    .sales
                    .iter()
                    .map(|s| (s.portfolio_id, s.asset_id, if month.exempt { 0.0 } else { s.gain.max(0.0) }))
                    .collect()
            } else {
                let Some(month) = assessment.months.iter().find(|m| m.period == payment.period) else {
                    continue;
                };
                month
                    .sales
                    .iter()
                    .map(|s| (s.portfolio_id, s.asset_id, if s.exemption_eligible && month.stock_exempt { 0.0 } else { s.gain.max(0.0) }))
                    .collect()
            };
            let total: f64 = weights.iter().map(|w| w.2).sum();
            if total <= EPS {
                continue;
            }
            let ours: f64 = weights
                .iter()
                .filter(|(portfolio, _, _)| portfolio_ids.contains(portfolio))
                .filter(|(_, asset, _)| asset_id.is_none_or(|a| *asset == a))
                .map(|w| w.2)
                .sum();
            if ours > EPS {
                flows.push((parse_date(&payment.paid_date)?, payment.amount * ours / total));
            }
        }
    }
    Ok(flows)
}

// Fase 26 — a tabela embarcada (`finance_core::tax_rules`, o que
// `sync_tax_rules` aplica) e o seed da migration (fallback offline) guardam
// o mesmo conteúdo; este teste pega a divergência sem rede nem Lume.
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn embedded_rules_match_the_migration_seed() {
        let db = crate::test_support::memory_db().await;
        let seeded: HashMap<(String, String), tax_rules::Model> = tax_rules::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|r| ((r.rule_key.clone(), r.effective_from.clone()), r))
            .collect();

        let embedded = crate::finance_core::tax_rules::rules_br();
        assert_eq!(embedded.len(), seeded.len());
        for rule in embedded {
            let key = (rule.key.to_string(), rule.effective_from.format("%Y-%m-%d").to_string());
            let seed = seeded.get(&key).unwrap_or_else(|| panic!("missing seed for {key:?}"));
            assert!((seed.default_value - rule.value).abs() < EPS, "{key:?} value");
            assert_eq!(seed.description, rule.description, "{key:?} description");
            assert_eq!(seed.source, rule.source, "{key:?} source");
        }
    }
}
