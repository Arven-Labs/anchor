use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::commands::currency::{
    base_currency, load_fx_history, workspace_of_portfolio, DEFAULT_BASE_CURRENCY,
};
use crate::domain::benchmark::{self, PricePoint};
use crate::domain::twr::{self, CashFlow, FlowKind};
use crate::entity::{assets, macro_index_monthly, stock_price_history, transactions};
use crate::error::AppError;

const BUY: &str = "compra";
const SELL: &str = "venda";
const DIVIDEND: &str = "provento";

// Fase 10.3, fatia escopada às classes com preço histórico automatizado
// (decisão explícita do dono do projeto — Stocks internacionais/Tesouro
// Direto/Renda Fixa ainda não têm, ver project/PHASE.md). `aporte`/
// `retirada` (sem asset_id) ficam fora deste cálculo. `fii` entrou na
// Sessão 41, `etf_br` na Sessão 49 (2026-08-02) — mesmo endpoint Yahoo
// `{ticker}.SA` que `acao_br` já usa pra `stock_price_history`, sem
// coletor novo. `cripto` entrou na Sessão 51 — fonte diferente
// (CoinGecko), mas grava na mesma tabela genérica `stock_price_history`
// (`ticker`/`price_date`/`close_price`, sem coluna de fonte usada aqui),
// então esta consulta não distingue de onde o preço veio. `bdr` entrou na
// Sessão 53 — mesmo endpoint Yahoo `.SA` de `acao_br`/`fii`/`etf_br`, zero
// mudança de coletor. `metal` entrou na Sessão 55 — fonte Yahoo sem `.SA`
// (COMEX), mesma tabela genérica, mesmo raciocínio. Essa sub-carteira
// é tratada como autocontida: suas próprias compra(+)/venda(−) são os
// fluxos de caixa externos do Dietz Modificado — o app não modela um
// sub-ledger de caixa, então uma compra não é "caixa→ativo", é "ativo
// aparece", e pro efeito desta fatia isso equivale a um aporte na
// sub-carteira.
const ASSET_CLASSES_WITH_AUTO_QUOTE: [&str; 9] = [
    "acao_br",
    "fii",
    "etf_br",
    "cripto",
    "bdr",
    "metal",
    "acao_internacional",
    "reit",
    "etf_us",
];
const PRICE_TOLERANCE_DAYS: i64 = 7;

#[derive(Serialize)]
pub struct MonthlyReturn {
    pub year_month: String,
    pub bmv: f64,
    pub emv: f64,
    pub cf_total: f64,
    pub r_month_pct: f64,
    pub r_cumulative_pct: f64,
}

fn days_in_month(year: i32, month: u32) -> i64 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .expect("valid date");
    let start = NaiveDate::from_ymd_opt(year, month, 1).expect("valid date");
    (next - start).num_days()
}

// Peso de um fluxo dentro do mês: dia 1 ≈ quase todo o mês "trabalhado",
// último dia ≈ 0. Bate com o exemplo do PHASE.md (dia 10 de um mês de 31
// dias → 21/31 ≈ 0,6774).
fn flow_weight(date: NaiveDate) -> f64 {
    let days = days_in_month(date.year(), date.month());
    (days - date.day() as i64) as f64 / days as f64
}

// Fim do mês pro qual valorar EMV — pro mês corrente (ainda em curso), usa
// "hoje" em vez do último dia calendário (que ainda não aconteceu); ao
// rodar de novo já no mês seguinte, esse mesmo mês passa a usar seu último
// dia real.
fn month_end_date(year: i32, month: u32, today: NaiveDate) -> NaiveDate {
    let last_day = days_in_month(year, month);
    let candidate = NaiveDate::from_ymd_opt(year, month, last_day as u32).expect("valid date");
    candidate.min(today)
}

async fn closest_price(
    db: &DatabaseConnection,
    ticker: &str,
    target_date: NaiveDate,
) -> Result<Option<f64>, AppError> {
    let lower = (target_date - Duration::days(PRICE_TOLERANCE_DAYS))
        .format("%Y-%m-%d")
        .to_string();
    let upper = (target_date + Duration::days(PRICE_TOLERANCE_DAYS))
        .format("%Y-%m-%d")
        .to_string();

    let candidates = stock_price_history::Entity::find()
        .filter(stock_price_history::Column::Ticker.eq(ticker))
        .filter(stock_price_history::Column::PriceDate.gte(lower))
        .filter(stock_price_history::Column::PriceDate.lte(upper))
        .all(db)
        .await?;

    Ok(candidates
        .into_iter()
        .filter_map(|row| {
            NaiveDate::parse_from_str(&row.price_date, "%Y-%m-%d")
                .ok()
                .map(|d| (d, row.close_price))
        })
        .min_by_key(|(d, _)| (*d - target_date).num_days().abs())
        .map(|(_, price)| price))
}

#[tauri::command]
pub async fn get_portfolio_profitability(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<Vec<MonthlyReturn>, AppError> {
    let db = db.inner();
    let base = base_currency(db, workspace_of_portfolio(db, portfolio_id).await?).await?;
    compute_profitability(db, &[portfolio_id], None, &base).await
}

// Extraída pra ser reaproveitada por `get_profitability_comparison` (Fase
// 13.5), mesmo precedente de `commands::transaction::compute_positions`
// (Fase 13.1) — evita duplicar a reconstrução mês a mês do TWR.
// `portfolio_ids` aceita mais de um portfolio (Fase 12 — retorno alavancado
// precisa somar as transações de um ativo em todos os portfolios de um
// workspace, já que `Liability` só carrega contexto de workspace, não de
// portfolio); `asset_id`, quando `Some`, isola o retorno de um único ativo
// em vez de agregar a sub-carteira inteira.
pub async fn compute_profitability(
    db: &DatabaseConnection,
    portfolio_ids: &[i32],
    asset_id: Option<i32>,
    base: &str,
) -> Result<Vec<MonthlyReturn>, AppError> {
    let txs = transactions::Entity::find()
        .filter(transactions::Column::PortfolioId.is_in(portfolio_ids.iter().copied()))
        .all(db)
        .await?;

    let asset_ids: Vec<i32> = txs.iter().filter_map(|t| t.asset_id).collect();
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

    let auto_quote_ids: HashSet<i32> = assets_map
        .values()
        .filter(|a| ASSET_CLASSES_WITH_AUTO_QUOTE.contains(&a.asset_class.as_str()))
        .map(|a| a.id)
        .collect();

    let mut relevant: Vec<&transactions::Model> = txs
        .iter()
        .filter(|t| {
            matches!(t.transaction_type.as_str(), BUY | SELL | DIVIDEND)
                && t.asset_id.is_some_and(|id| auto_quote_ids.contains(&id))
                && asset_id.is_none_or(|target| t.asset_id == Some(target))
        })
        .collect();
    relevant.sort_by(|a, b| a.transaction_date.cmp(&b.transaction_date));

    if relevant.is_empty() {
        return Ok(vec![]);
    }

    // Fase 24 — DARF pago, rateado pras carteiras/ativo pedidos: custo sem
    // ativo correspondente, reduz o retorno no mês do pagamento.
    let tax_flows = crate::commands::tax::tax_cost_flows(db, portfolio_ids, asset_id).await?;

    // Fase 19 — fluxos e valor de fim de mês entram na moeda base pela taxa da
    // própria data (a variação cambial faz parte do retorno); DARF é sempre BRL.
    let mut currencies: HashSet<String> = relevant
        .iter()
        .filter_map(|t| t.asset_id)
        .filter_map(|id| assets_map.get(&id).map(|a| a.currency.clone()))
        .collect();
    currencies.insert(DEFAULT_BASE_CURRENCY.to_string());
    let fx = load_fx_history(db, base, &currencies).await?;
    let to_base = |value: f64, currency: &str, date: NaiveDate| -> Result<f64, AppError> {
        fx.rate_on(currency, base, date).map(|rate| value * rate).ok_or_else(|| {
            AppError::NotFound(format!(
                "sem câmbio {currency}/{base} — colete o par em Research → FX"
            ))
        })
    };

    let today = Utc::now().date_naive();
    let first_date = NaiveDate::parse_from_str(&relevant[0].transaction_date, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidGuard("data de transação inválida".to_string()))?;

    let mut months: Vec<(i32, u32)> = Vec::new();
    let (mut y, mut m) = (first_date.year(), first_date.month());
    while (y, m) <= (today.year(), today.month()) {
        months.push((y, m));
        m += 1;
        if m == 13 {
            m = 1;
            y += 1;
        }
    }

    let mut holdings: HashMap<i32, f64> = HashMap::new();
    let mut bmv = 0.0;
    let mut monthly_r: Vec<f64> = Vec::new();
    let mut results = Vec::new();

    for (year, month) in months {
        let month_prefix = format!("{year:04}-{month:02}");
        let month_txs: Vec<&&transactions::Model> = relevant
            .iter()
            .filter(|t| t.transaction_date.starts_with(&month_prefix))
            .collect();

        let mut cash_flows = Vec::new();
        for tx in &month_txs {
            let date = NaiveDate::parse_from_str(&tx.transaction_date, "%Y-%m-%d")
                .map_err(|_| AppError::InvalidGuard("data de transação inválida".to_string()))?;
            let weight = flow_weight(date);
            let value = to_base(tx.total_value, &assets_map[&tx.asset_id.unwrap()].currency, date)?;
            match tx.transaction_type.as_str() {
                BUY => {
                    let qty = tx.quantity.unwrap_or(0.0);
                    *holdings.entry(tx.asset_id.unwrap()).or_insert(0.0) += qty;
                    cash_flows.push(CashFlow {
                        amount: twr::flow_amount(FlowKind::Contribution, value),
                        weight,
                    });
                }
                SELL => {
                    let qty = tx.quantity.unwrap_or(0.0);
                    *holdings.entry(tx.asset_id.unwrap()).or_insert(0.0) -= qty;
                    cash_flows.push(CashFlow {
                        amount: twr::flow_amount(FlowKind::Withdrawal, value),
                        weight,
                    });
                }
                DIVIDEND => {
                    cash_flows.push(CashFlow {
                        amount: twr::flow_amount(FlowKind::Income, value),
                        weight,
                    });
                }
                _ => {}
            }
        }
        for (paid_date, amount) in &tax_flows {
            if paid_date.year() == year && paid_date.month() == month {
                cash_flows.push(CashFlow {
                    amount: twr::flow_amount(
                        FlowKind::Cost,
                        to_base(*amount, DEFAULT_BASE_CURRENCY, *paid_date)?,
                    ),
                    weight: flow_weight(*paid_date),
                });
            }
        }

        let target_date = month_end_date(year, month, today);
        let mut emv = 0.0;
        for (&asset_id, &qty) in holdings.iter() {
            if qty.abs() < 1e-9 {
                continue;
            }
            let asset = &assets_map[&asset_id];
            let ticker = &asset.ticker;
            let price = closest_price(db, ticker, target_date).await?.ok_or_else(|| {
                AppError::NotFound(format!(
                    "sem preço histórico de {ticker} perto de {target_date} — rode \"Atualizar histórico de preços\""
                ))
            })?;
            emv += to_base(qty * price, &asset.currency, target_date)?;
        }

        let r_month = if bmv == 0.0 && emv == 0.0 && cash_flows.is_empty() {
            0.0
        } else {
            twr::modified_dietz(bmv, emv, &cash_flows)?
        };

        monthly_r.push(r_month);
        let cf_total: f64 = cash_flows.iter().map(|cf| cf.amount).sum();

        results.push(MonthlyReturn {
            year_month: month_prefix,
            bmv,
            emv,
            cf_total,
            r_month_pct: r_month * 100.0,
            r_cumulative_pct: twr::chain(&monthly_r) * 100.0,
        });

        bmv = emv;
    }

    Ok(results)
}

// Fase 13.5 — compara a carteira com os 7 benchmarks pedidos, todos com
// fonte gratuita (CDI/IPCA via `macro_index_monthly`, IBOV/IVVB11/IFIX/SMLL/
// IDIV via `stock_price_history` — IFIX/SMLL/IDIV entraram na Sessão 83 via
// a API de estatísticas de índices da B3, ver PHASE.md item 13.5). Cada
// benchmark é recortado pra mesma janela de meses da carteira antes de
// qualquer `chain()` — comparar com anos de histórico de CDI enquanto a
// carteira só tem 2 meses seria enganoso.
#[derive(Serialize)]
pub struct BenchmarkPoint {
    pub year_month: String,
    pub r_month_pct: f64,
    pub r_cumulative_pct: f64,
}

#[derive(Serialize)]
pub struct BenchmarkSeries {
    pub code: String,
    pub label: String,
    pub monthly: Vec<BenchmarkPoint>,
}

#[derive(Serialize)]
pub struct ProfitabilityComparison {
    pub portfolio: Vec<MonthlyReturn>,
    pub benchmarks: Vec<BenchmarkSeries>,
    // Fase 19 — moeda em que `portfolio` está; ≠ BRL esconde os benchmarks.
    pub base_currency: String,
}

// Encadeia uma sequência de retornos mensais (já na ordem certa) em pontos
// com `r_cumulative_pct`, mesmo padrão do loop de `compute_profitability`
// acima, só que reaproveitável pras duas fontes de benchmark (macro e
// preço), que chegam em formatos diferentes até aqui.
fn chain_into_points(monthly_r: &[(String, f64)]) -> Vec<BenchmarkPoint> {
    let mut running: Vec<f64> = Vec::new();
    monthly_r
        .iter()
        .map(|(year_month, r_month_pct)| {
            running.push(r_month_pct / 100.0);
            BenchmarkPoint {
                year_month: year_month.clone(),
                r_month_pct: *r_month_pct,
                r_cumulative_pct: twr::chain(&running) * 100.0,
            }
        })
        .collect()
}

fn month_before(months: &[(i32, u32)]) -> (i32, u32) {
    let (year, month) = months[0];
    if month == 1 { (year - 1, 12) } else { (year, month - 1) }
}

async fn build_macro_series(
    db: &DatabaseConnection,
    index_code: &str,
    label: &str,
    months: &[(i32, u32)],
) -> Result<Option<BenchmarkSeries>, AppError> {
    let rows = macro_index_monthly::Entity::find()
        .filter(macro_index_monthly::Column::IndexCode.eq(index_code))
        .all(db)
        .await?;
    let by_month: HashMap<String, f64> =
        rows.into_iter().map(|r| (r.year_month, r.value_pct)).collect();

    let monthly_r: Vec<(String, f64)> = months
        .iter()
        .filter_map(|(y, m)| {
            let key = format!("{y:04}-{m:02}");
            by_month.get(&key).map(|v| (key, *v))
        })
        .collect();

    if monthly_r.is_empty() {
        return Ok(None);
    }

    Ok(Some(BenchmarkSeries {
        code: index_code.to_string(),
        label: label.to_string(),
        monthly: chain_into_points(&monthly_r),
    }))
}

async fn build_price_series(
    db: &DatabaseConnection,
    ticker: &str,
    label: &str,
    months: &[(i32, u32)],
    today: NaiveDate,
) -> Result<Option<BenchmarkSeries>, AppError> {
    let (by, bm) = month_before(months);
    let mut month_ends = vec![month_end_date(by, bm, today)];
    month_ends.extend(months.iter().map(|&(y, m)| month_end_date(y, m, today)));

    let prices: Vec<PricePoint> = stock_price_history::Entity::find()
        .filter(stock_price_history::Column::Ticker.eq(ticker))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|row| {
            NaiveDate::parse_from_str(&row.price_date, "%Y-%m-%d")
                .ok()
                .map(|date| PricePoint { date, close: row.close_price })
        })
        .collect();

    let returns = benchmark::monthly_returns_from_prices(&prices, &month_ends, PRICE_TOLERANCE_DAYS);
    if returns.is_empty() {
        return Ok(None);
    }

    let monthly_r: Vec<(String, f64)> =
        returns.into_iter().map(|r| (r.year_month, r.r_month_pct)).collect();

    Ok(Some(BenchmarkSeries {
        code: ticker.to_string(),
        label: label.to_string(),
        monthly: chain_into_points(&monthly_r),
    }))
}

#[tauri::command]
pub async fn get_profitability_comparison(
    db: tauri::State<'_, DatabaseConnection>,
    portfolio_id: i32,
) -> Result<ProfitabilityComparison, AppError> {
    let db = db.inner();
    let base = base_currency(db, workspace_of_portfolio(db, portfolio_id).await?).await?;
    compare_with_benchmarks(db, portfolio_id, base).await
}

pub async fn compare_with_benchmarks(
    db: &DatabaseConnection,
    portfolio_id: i32,
    base_currency: String,
) -> Result<ProfitabilityComparison, AppError> {
    let portfolio = compute_profitability(db, &[portfolio_id], None, &base_currency).await?;

    // CDI/IPCA/Selic são taxas em reais e os índices de preço estão em BRL:
    // fora do BRL a comparação seria enganosa, então a tela avisa e não plota.
    if portfolio.is_empty() || base_currency != DEFAULT_BASE_CURRENCY {
        return Ok(ProfitabilityComparison { portfolio, benchmarks: vec![], base_currency });
    }

    let months: Vec<(i32, u32)> = portfolio
        .iter()
        .map(|r| {
            let (y, m) = r.year_month.split_once('-').expect("year_month sempre YYYY-MM");
            (y.parse().expect("ano válido"), m.parse().expect("mês válido"))
        })
        .collect();
    let today = Utc::now().date_naive();

    let candidates = [
        build_macro_series(db, "cdi", "CDI", &months).await?,
        build_macro_series(db, "ipca", "IPCA", &months).await?,
        build_macro_series(db, "selic", "Selic", &months).await?,
        build_price_series(db, "^BVSP", "IBOV", &months, today).await?,
        build_price_series(db, "IVVB11", "IVVB11", &months, today).await?,
        build_price_series(db, "IFIX", "IFIX", &months, today).await?,
        build_price_series(db, "SMLL", "SMLL", &months, today).await?,
        build_price_series(db, "IDIV", "IDIV", &months, today).await?,
    ];
    let benchmarks = candidates.into_iter().flatten().collect();

    Ok(ProfitabilityComparison { portfolio, benchmarks, base_currency })
}
