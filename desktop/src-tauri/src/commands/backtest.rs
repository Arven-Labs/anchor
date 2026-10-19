// Fase 22, fatia 3 — comando do backtest: monta as séries da carteira atual (fechamento ajustado), roda o motor puro
// (`domain::backtest`) e compara com CDI, IPCA e IBOV. Fatia 4: corretagem por ordem e IR opcional das vendas.
//
// A carteira de partida são as posições abertas de ações, FIIs, ETFs, BDRs e cripto em BRL; os pesos padrão são o
// valor de mercado de hoje, ou os que o usuário mandar. O que não dá para simular (renda fixa, imóvel, moeda
// estrangeira, ativo sem histórico) fica de fora e vira aviso, nunca erro mudo.
use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, NaiveDate};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::commands::profitability::ASSET_CLASSES_WITH_AUTO_QUOTE;
use crate::commands::currency::positions_in_base;
use crate::commands::tax::load_rule_set;
use crate::domain::backtest::{self, AssetSeries, Params, Rebalance, TaxModel};
use crate::domain::tax::classify::{treatment, TaxTreatment};
use crate::entity::{macro_index_monthly, stock_adjusted_close, stock_price_history};
use crate::error::AppError;

const IBOV_TICKER: &str = "^BVSP";

#[derive(Deserialize)]
pub struct WeightOverride {
    pub asset_id: i32,
    pub weight: f64,
}

#[derive(Deserialize)]
pub struct BacktestRequest {
    pub portfolio_id: i32,
    /// Vazio = valor de mercado de hoje. Com pesos, só entram os ativos que aparecem aqui com peso positivo.
    #[serde(default)]
    pub weights: Vec<WeightOverride>,
    /// `never`, `monthly`, `quarterly` ou `yearly`.
    pub rebalance: String,
    pub initial: f64,
    #[serde(default)]
    pub monthly_contribution: f64,
    /// `YYYY-MM-DD`: corta o início da janela (por padrão, o primeiro dia em que todos têm preço).
    #[serde(default)]
    pub start_date: Option<String>,
    /// Corretagem fixa, em R$, por ordem (compra inicial, aportes e rebalanceamentos).
    #[serde(default)]
    pub brokerage_per_order: f64,
    /// Apura o IR de renda variável das vendas do rebalanceamento e o debita no mês seguinte.
    #[serde(default)]
    pub apply_tax: bool,
}

#[derive(Serialize)]
pub struct BacktestAsset {
    pub asset_id: i32,
    pub ticker: String,
    pub target_weight: f64,
    pub final_weight: f64,
    /// Sem fechamento ajustado: o retorno usa só o preço e subestima quem paga dividendos.
    pub price_only: bool,
}

#[derive(Serialize)]
pub struct BacktestMetrics {
    pub total_invested: f64,
    pub final_value: f64,
    pub twr_pct: f64,
    pub cagr_pct: f64,
    pub volatility_pct: f64,
    pub max_drawdown_pct: f64,
    pub max_drawdown_date: Option<String>,
    pub sharpe: Option<f64>,
    pub rebalance_count: u32,
    pub total_fees: f64,
    pub total_tax_paid: f64,
    /// IR do último mês, devido e ainda não debitado.
    pub tax_pending: f64,
    /// Taxa livre de risco usada no Sharpe (CDI anualizado no período), em %.
    pub risk_free_pct: f64,
}

/// Um ponto por mês (o último dia do mês na simulação). Os índices têm base 1,0 no início da janela; `None` quando
/// o benchmark não tem dado naquele mês (ex.: CDI do mês corrente ainda não publicado).
#[derive(Serialize)]
pub struct BacktestMonth {
    pub date: String,
    pub value: f64,
    pub invested: f64,
    pub portfolio_index: f64,
    pub cdi_index: Option<f64>,
    pub ipca_index: Option<f64>,
    pub ibov_index: Option<f64>,
}

#[derive(Serialize)]
pub struct BenchmarkSummary {
    pub code: String,
    pub label: String,
    pub total_return_pct: f64,
    pub cagr_pct: f64,
}

#[derive(Serialize)]
pub struct BacktestResponse {
    pub start: String,
    pub end: String,
    pub assets: Vec<BacktestAsset>,
    pub metrics: BacktestMetrics,
    pub months: Vec<BacktestMonth>,
    pub benchmarks: Vec<BenchmarkSummary>,
    pub names: Vec<String>,
    pub correlation: Vec<Vec<f64>>,
    pub warnings: Vec<String>,
}

fn parse_day(raw: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| AppError::InvalidInput(format!("invalid date: '{raw}'")))
}

const CRYPTO_CLASS: &str = "cripto";

/// Série diária do ativo: fechamento ajustado quando existe, senão o fechamento simples (`price_only`). Cripto não
/// cai no fechamento simples: o do CoinGecko é em USD, e a série ajustada dela é o par `-BRL` do Yahoo.
async fn load_series(db: &DatabaseConnection, ticker: &str, asset_class: &str) -> Result<(Vec<(NaiveDate, f64)>, bool), AppError> {
    let adjusted = stock_adjusted_close::Entity::find()
        .filter(stock_adjusted_close::Column::Ticker.eq(ticker))
        .order_by_asc(stock_adjusted_close::Column::PriceDate)
        .all(db)
        .await?;
    if !adjusted.is_empty() {
        return Ok((adjusted.into_iter().filter_map(|r| Some((parse_day(&r.price_date).ok()?, r.adj_close_price))).collect(), false));
    }
    if asset_class == CRYPTO_CLASS {
        return Ok((Vec::new(), false));
    }
    Ok((load_closes(db, ticker).await?, true))
}

async fn load_closes(db: &DatabaseConnection, ticker: &str) -> Result<Vec<(NaiveDate, f64)>, AppError> {
    let rows = stock_price_history::Entity::find()
        .filter(stock_price_history::Column::Ticker.eq(ticker))
        .order_by_asc(stock_price_history::Column::PriceDate)
        .all(db)
        .await?;
    Ok(rows.into_iter().filter_map(|r| Some((parse_day(&r.price_date).ok()?, r.close_price))).collect())
}

async fn macro_months(db: &DatabaseConnection, code: &str) -> Result<HashMap<(i32, u32), f64>, AppError> {
    let rows = macro_index_monthly::Entity::find().filter(macro_index_monthly::Column::IndexCode.eq(code)).all(db).await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let (year, month) = r.year_month.split_once('-')?;
            Some(((year.parse().ok()?, month.parse().ok()?), r.value_pct))
        })
        .collect())
}

fn next_month((year, month): (i32, u32)) -> (i32, u32) {
    if month == 12 { (year + 1, 1) } else { (year, month + 1) }
}

/// Índice acumulado (base 1,0) de uma série mensal em %, dos meses depois do de `start` até o de `at`; `None` se
/// faltar algum mês no caminho.
fn macro_index(series: &HashMap<(i32, u32), f64>, start: NaiveDate, at: NaiveDate) -> Option<f64> {
    let mut month = next_month((start.year(), start.month()));
    let last = (at.year(), at.month());
    let mut index = 1.0;
    while month <= last {
        index *= 1.0 + series.get(&month)? / 100.0;
        month = next_month(month);
    }
    Some(index)
}

fn price_on_or_before(series: &BTreeMap<NaiveDate, f64>, date: NaiveDate) -> Option<f64> {
    series.range(..=date).next_back().map(|(_, p)| *p)
}

fn annualized(index: f64, days: f64) -> f64 {
    if days > 0.0 { (index.powf(365.25 / days) - 1.0) * 100.0 } else { 0.0 }
}

pub async fn run_backtest_in(db: &DatabaseConnection, request: BacktestRequest) -> Result<BacktestResponse, AppError> {
    let rebalance = Rebalance::parse(&request.rebalance)
        .ok_or_else(|| AppError::InvalidInput("rebalance must be never, monthly, quarterly or yearly".to_string()))?;
    let start_cut = request.start_date.as_deref().map(parse_day).transpose()?;

    let mut warnings = Vec::new();
    let overrides: HashMap<i32, f64> = request.weights.iter().map(|w| (w.asset_id, w.weight)).collect();
    let mut series: Vec<AssetSeries> = Vec::new();
    let mut picked: Vec<(i32, String, f64, bool)> = Vec::new(); // asset_id, ticker, peso, price_only
    let mut classes: Vec<(String, String)> = Vec::new(); // classe, ticker (para o IR), na ordem de `series`

    // Valores na moeda base: o peso padrão de uma cripto em USD não pode ficar em dólar contra ações em reais.
    for position in positions_in_base(db, request.portfolio_id).await?.0 {
        if position.quantity <= 0.0 {
            continue;
        }
        let label = &position.ticker;
        if !ASSET_CLASSES_WITH_AUTO_QUOTE.contains(&position.asset_class.as_str()) {
            warnings.push(format!("{label}: left out, its class has no price history to simulate"));
            continue;
        }
        if position.native_currency != "BRL" && position.asset_class != CRYPTO_CLASS {
            warnings.push(format!("{label}: left out, the simulation only handles BRL assets for now"));
            continue;
        }
        let weight = if overrides.is_empty() {
            position.market_value.or(position.average_buy_price.map(|p| p * position.quantity)).unwrap_or(0.0)
        } else {
            overrides.get(&position.asset_id).copied().unwrap_or(0.0)
        };
        if weight <= 0.0 {
            if !overrides.is_empty() {
                warnings.push(format!("{label}: left out, no weight given"));
            } else {
                warnings.push(format!("{label}: left out, no market value to weigh it by"));
            }
            continue;
        }
        let (mut points, price_only) = load_series(db, &position.ticker, &position.asset_class).await?;
        if let Some(cut) = start_cut {
            points.retain(|(date, _)| *date >= cut);
        }
        if points.len() < 2 {
            warnings.push(format!("{label}: left out, no price history (run Update price history in Profitability)"));
            continue;
        }
        if price_only {
            warnings.push(format!("{label}: no dividend-adjusted close yet, so its return counts the price only (run Update price history)"));
        }
        if request.apply_tax && !matches!(treatment(&position.asset_class, &position.ticker), TaxTreatment::StockExemptEligible | TaxTreatment::CommonNoExemption | TaxTreatment::Fii | TaxTreatment::Crypto) {
            warnings.push(format!("{label}: no tax modelled for its class, so its sales are not taxed here"));
        }
        classes.push((position.asset_class.clone(), position.ticker.clone()));
        series.push(AssetSeries { name: position.ticker.clone(), points });
        picked.push((position.asset_id, position.ticker.clone(), weight, price_only));
    }
    if series.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "no asset to simulate: {}",
            if warnings.is_empty() { "the portfolio has no open position".to_string() } else { warnings.join("; ") }
        )));
    }

    // Janela e taxa livre de risco (CDI anualizado no período).
    let window_start = series.iter().map(|s| s.points[0].0).max().expect("séries não vazias");
    let window_end = series.iter().map(|s| s.points[s.points.len() - 1].0).min().expect("séries não vazias");
    let cdi = macro_months(db, "cdi").await?;
    let ipca = macro_months(db, "ipca").await?;
    let window_days = (window_end - window_start).num_days() as f64;
    let risk_free = match macro_index(&cdi, window_start, window_end) {
        Some(index) if window_days > 0.0 => index.powf(365.25 / window_days) - 1.0,
        _ => {
            warnings.push("CDI is missing for part of the period, so the Sharpe ratio uses a zero risk-free rate".to_string());
            0.0
        }
    };

    let total_weight: f64 = picked.iter().map(|p| p.2).sum();
    let tax = if request.apply_tax { Some(TaxModel { rules: load_rule_set(db).await?, assets: classes }) } else { None };
    let result = backtest::run(
        &series,
        &Params {
            weights: picked.iter().map(|p| p.2 / total_weight).collect(),
            rebalance,
            initial: request.initial,
            monthly_contribution: request.monthly_contribution,
            risk_free_annual: risk_free,
            brokerage_per_order: request.brokerage_per_order,
            tax,
        },
    )
    .map_err(AppError::InvalidInput)?;

    // Pontos mensais: o primeiro é o início (tudo em 1,0); depois, o último dia de cada mês seguinte.
    let ibov: BTreeMap<NaiveDate, f64> = load_closes(db, IBOV_TICKER).await?.into_iter().collect();
    let ibov_base = price_on_or_before(&ibov, result.start);
    let ibov_index = |date: NaiveDate| Some(price_on_or_before(&ibov, date)? / ibov_base?);
    let mut sampled: BTreeMap<(i32, u32), &backtest::CurvePoint> = BTreeMap::new();
    for point in &result.curve {
        sampled.insert((point.date.year(), point.date.month()), point);
    }
    let start_month = (result.start.year(), result.start.month());
    let first = &result.curve[0];
    let mut months = vec![BacktestMonth {
        date: first.date.to_string(),
        value: first.value,
        invested: first.invested,
        portfolio_index: 1.0,
        cdi_index: Some(1.0),
        ipca_index: Some(1.0),
        ibov_index: ibov_base.map(|_| 1.0),
    }];
    for (month, point) in &sampled {
        if *month == start_month {
            continue;
        }
        months.push(BacktestMonth {
            date: point.date.to_string(),
            value: point.value,
            invested: point.invested,
            portfolio_index: point.index,
            cdi_index: macro_index(&cdi, result.start, point.date),
            ipca_index: macro_index(&ipca, result.start, point.date),
            ibov_index: ibov_index(point.date),
        });
    }

    let mut benchmarks = Vec::new();
    for (code, label, pick) in [
        ("cdi", "CDI", (|m: &BacktestMonth| m.cdi_index) as fn(&BacktestMonth) -> Option<f64>),
        ("ipca", "IPCA", |m: &BacktestMonth| m.ipca_index),
        ("ibov", "IBOV", |m: &BacktestMonth| m.ibov_index),
    ] {
        if let Some((month, index)) = months.iter().rev().find_map(|m| Some((m, pick(m)?))) {
            let days = (parse_day(&month.date)? - result.start).num_days() as f64;
            benchmarks.push(BenchmarkSummary {
                code: code.to_string(),
                label: label.to_string(),
                total_return_pct: (index - 1.0) * 100.0,
                cagr_pct: annualized(index, days),
            });
        }
    }

    let m = &result.metrics;
    Ok(BacktestResponse {
        start: result.start.to_string(),
        end: result.end.to_string(),
        assets: picked
            .iter()
            .zip(&result.final_weights)
            .map(|((asset_id, ticker, weight, price_only), final_weight)| BacktestAsset {
                asset_id: *asset_id,
                ticker: ticker.clone(),
                target_weight: weight / total_weight,
                final_weight: *final_weight,
                price_only: *price_only,
            })
            .collect(),
        metrics: BacktestMetrics {
            total_invested: m.total_invested,
            final_value: m.final_value,
            twr_pct: m.twr_pct,
            cagr_pct: m.cagr_pct,
            volatility_pct: m.volatility_pct,
            max_drawdown_pct: m.max_drawdown_pct,
            max_drawdown_date: m.max_drawdown_date.map(|d| d.to_string()),
            sharpe: m.sharpe,
            rebalance_count: m.rebalance_count,
            total_fees: m.total_fees,
            total_tax_paid: m.total_tax_paid,
            tax_pending: m.tax_pending,
            risk_free_pct: risk_free * 100.0,
        },
        months,
        benchmarks,
        names: result.names,
        correlation: result.correlation,
        warnings,
    })
}

#[tauri::command]
pub async fn run_backtest(
    db: tauri::State<'_, DatabaseConnection>,
    request: BacktestRequest,
) -> Result<BacktestResponse, AppError> {
    run_backtest_in(db.inner(), request).await
}
