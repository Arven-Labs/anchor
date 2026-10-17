use std::sync::atomic::{AtomicBool, Ordering};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;

use crate::entity::{
    crypto_fear_greed, stock_dcf_fundamentals, stock_dividend_payments, stock_dividends_avg,
    stock_fundamentals, stock_price_history, stock_quotes, stock_technicals,
};
use crate::error::AppError;
use crate::finance_api::{
    benchmark as finance_api_benchmark, crypto as finance_api_crypto,
    fx as finance_api_fx, metals as finance_api_metals, reit as finance_api_reit, stocks as finance_api_stocks,
    us_stock as finance_api_us_stock,
};

#[derive(Serialize)]
pub struct CollectorSummary {
    pub success: bool,
    pub output: String,
}

// `asset_class` distingue "cripto" (fonte CoinGecko, `--crypto-ticker`,
// Sessão 51), "metal" (fonte Yahoo sem `.SA` — COMEX, `--metal-ticker`,
// Sessão 55) e "acao_internacional" (Yahoo sem `.SA` — ação americana,
// `--us-ticker`) do resto (Ação BR/FII/ETF/BDR, todos o mesmo endpoint Yahoo
// `.SA`) — ver PHASE.md item 8. `None` preserva o comportamento anterior
// (chamadores que não sabem/não precisam distinguir classe, ex.: os
// formulários de valuation via `useTickerCollector`).
//
// Cada branch chama o coletor da classe: `_` usa `finance_api::stocks`,
// "cripto"/"metal" usam `finance_api::{crypto,metals}` e
// "acao_internacional"/"reit"/"etf_us" usam `finance_api::{us_stock,reit}`.
/// Fase 32, seção I — câmbio. Comando próprio (não é uma `asset_class`): o par
/// vira `EURUSD` em `stock_quotes`/`stock_price_history` e a tela lê pelos
/// mesmos `list_stock_quotes`/`list_stock_price_history`.
#[tauri::command]
pub async fn run_fx_collector(
    lock: tauri::State<'_, AtomicBool>,
    db: tauri::State<'_, DatabaseConnection>,
    base: String,
    quote: String,
) -> Result<CollectorSummary, AppError> {
    if lock.swap(true, Ordering::SeqCst) {
        return Err(AppError::CollectorBusy);
    }
    let result = finance_api_fx::collect_pair(db.inner(), &base, &quote)
        .await
        .map(|r| CollectorSummary {
            success: true,
            output: format!(
                "{}: {:.4} {} — {} history point(s)",
                r.name, r.price, r.quote_currency, r.history_points,
            ),
        });
    lock.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
pub async fn run_stock_collector(
    lock: tauri::State<'_, AtomicBool>,
    db: tauri::State<'_, DatabaseConnection>,
    ticker: String,
    asset_class: Option<String>,
) -> Result<CollectorSummary, AppError> {
    match asset_class.as_deref() {
        Some("cripto") => {
            if lock.swap(true, Ordering::SeqCst) {
                return Err(AppError::CollectorBusy);
            }
            let result = finance_api_crypto::collect_ticker(db.inner(),&ticker)
                .await
                .map(|r| CollectorSummary {
                    success: true,
                    output: format!(
                        "{} ({}): US$ {:.2} — {} history point(s) — Fear & Greed: {} ({})",
                        r.symbol,
                        r.name,
                        r.price,
                        r.history_points,
                        r.fear_greed_value,
                        r.fear_greed_classification,
                    ),
                });
            lock.store(false, Ordering::SeqCst);
            result
        }
        Some("metal") => {
            if lock.swap(true, Ordering::SeqCst) {
                return Err(AppError::CollectorBusy);
            }
            let result = finance_api_metals::collect_ticker(db.inner(),&ticker)
                .await
                .map(|r| CollectorSummary {
                    success: true,
                    output: format!(
                        "{} ({}): US$ {:.2}/oz — {} history point(s)",
                        r.ticker, r.name, r.price, r.history_points,
                    ),
                });
            lock.store(false, Ordering::SeqCst);
            result
        }
        Some("acao_internacional") => {
            if lock.swap(true, Ordering::SeqCst) {
                return Err(AppError::CollectorBusy);
            }
            let result =
                finance_api_us_stock::run_us_stock_collector(db.inner(),&[ticker])
                    .await;
            lock.store(false, Ordering::SeqCst);

            result.map(|output| CollectorSummary {
                success: true,
                output,
            })
        }
        Some("reit") => {
            if lock.swap(true, Ordering::SeqCst) {
                return Err(AppError::CollectorBusy);
            }
            let result =
                finance_api_reit::run_reit_collector(db.inner(),&[ticker]).await;
            lock.store(false, Ordering::SeqCst);

            result.map(|output| CollectorSummary {
                success: true,
                output,
            })
        }
        Some("etf_us") => {
            if lock.swap(true, Ordering::SeqCst) {
                return Err(AppError::CollectorBusy);
            }
            let result =
                finance_api_us_stock::run_etf_us_collector(db.inner(),&[ticker])
                    .await;
            lock.store(false, Ordering::SeqCst);

            result.map(|output| CollectorSummary {
                success: true,
                output,
            })
        }
        _ => {
            if lock.swap(true, Ordering::SeqCst) {
                return Err(AppError::CollectorBusy);
            }
            let result =
                finance_api_stocks::run_stock_collector(db.inner(),&[ticker])
                    .await;
            lock.store(false, Ordering::SeqCst);

            result.map(|output| CollectorSummary {
                success: true,
                output,
            })
        }
    }
}

// Fase 14.4 — trocou de `run_collector` (subprocess Python, `crypto`) pra
// `finance_api::crypto::collect_indicators` direto — os indicadores do ciclo
// da moeda (ETH: 17, BTC: 12 desde a Sessão 112), gravados sob essa moeda. Uma
// fonte fora do ar não derruba as outras: as falhas vão como linhas `ERROR`
// no resumo. `coin` é parâmetro top-level (camelCase igual no `invoke`).
#[tauri::command]
pub async fn run_crypto_collector(
    lock: tauri::State<'_, AtomicBool>,
    db: tauri::State<'_, DatabaseConnection>,
    coin: String,
) -> Result<CollectorSummary, AppError> {
    if lock.swap(true, Ordering::SeqCst) {
        return Err(AppError::CollectorBusy);
    }
    let result = finance_api_crypto::collect_indicators(db.inner(), &coin)
        .await
        .map(|collection| CollectorSummary {
            success: collection.errors.is_empty(),
            output: collection
                .readings
                .iter()
                .map(|r| format!("{}: {:.4} -> {}", r.indicator, r.raw_value, r.signal))
                .chain(collection.errors.iter().map(|e| format!("ERROR {e}")))
                .collect::<Vec<_>>()
                .join("\n"),
        });
    lock.store(false, Ordering::SeqCst);

    result
}

#[tauri::command]
pub async fn list_stock_quotes(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<stock_quotes::Model>, AppError> {
    let quotes = stock_quotes::Entity::find()
        .order_by_desc(stock_quotes::Column::FetchedAt)
        .all(db.inner())
        .await?;

    Ok(quotes)
}

#[tauri::command]
pub async fn list_stock_fundamentals(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<stock_fundamentals::Model>, AppError> {
    let fundamentals = stock_fundamentals::Entity::find()
        .order_by_desc(stock_fundamentals::Column::FetchedAt)
        .all(db.inner())
        .await?;

    Ok(fundamentals)
}

#[tauri::command]
pub async fn list_stock_dividends_avg(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<stock_dividends_avg::Model>, AppError> {
    let dividends = stock_dividends_avg::Entity::find()
        .order_by_desc(stock_dividends_avg::Column::FetchedAt)
        .all(db.inner())
        .await?;

    Ok(dividends)
}

#[tauri::command]
pub async fn list_stock_dcf_fundamentals(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<stock_dcf_fundamentals::Model>, AppError> {
    let fundamentals = stock_dcf_fundamentals::Entity::find()
        .order_by_desc(stock_dcf_fundamentals::Column::FetchedAt)
        .all(db.inner())
        .await?;

    Ok(fundamentals)
}

#[tauri::command]
pub async fn list_stock_technicals(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<stock_technicals::Model>, AppError> {
    let technicals = stock_technicals::Entity::find()
        .order_by_desc(stock_technicals::Column::FetchedAt)
        .all(db.inner())
        .await?;

    Ok(technicals)
}

#[tauri::command]
pub async fn list_stock_dividend_payments(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Vec<stock_dividend_payments::Model>, AppError> {
    let payments = stock_dividend_payments::Entity::find()
        .order_by_asc(stock_dividend_payments::Column::PaymentDate)
        .all(db.inner())
        .await?;

    Ok(payments)
}

// Fase 14.4 — trocou de `run_collector` (subprocess Python, `--price-history`)
// pra `finance_api::stocks::collect_price_history` direto, mesma regra de
// escrita (`ON CONFLICT DO NOTHING` no índice único `(ticker, price_date)`).
#[tauri::command]
pub async fn run_price_history_backfill(
    lock: tauri::State<'_, AtomicBool>,
    db: tauri::State<'_, DatabaseConnection>,
    tickers: Vec<String>,
) -> Result<CollectorSummary, AppError> {
    if lock.swap(true, Ordering::SeqCst) {
        return Err(AppError::CollectorBusy);
    }
    let result = finance_api_stocks::collect_price_history(db.inner(),&tickers)
        .await
        .map(|count| CollectorSummary {
            success: true,
            output: format!("Fetched {count} daily price point(s)"),
        });
    lock.store(false, Ordering::SeqCst);

    result
}

// Fase 13.5 — atualiza os 7 benchmarks de fonte gratuita (CDI/IPCA/IBOV/
// IVVB11/IFIX/SMLL/IDIV) usados em "Return vs. benchmarks"
// (`commands::profitability::get_profitability_comparison`). Sem tickers
// pra passar — são sempre os mesmos 7.
//
// Chama `finance_api::benchmark::collect_benchmarks` direto.
#[tauri::command]
pub async fn run_benchmark_backfill(
    lock: tauri::State<'_, AtomicBool>,
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<CollectorSummary, AppError> {
    if lock.swap(true, Ordering::SeqCst) {
        return Err(AppError::CollectorBusy);
    }
    let result = finance_api_benchmark::collect_benchmarks(db.inner())
        .await
        .map(|s| CollectorSummary {
            success: true,
            output: format!(
                "CDI: {} month(s); IPCA: {} month(s); Selic: {} month(s); IBOV: {} point(s); \
                 IVVB11: {} point(s); IFIX: {} point(s); SMLL: {} point(s); IDIV: {} point(s)",
                s.cdi_count,
                s.ipca_count,
                s.selic_count,
                s.ibov_count,
                s.ivvb11_count,
                s.ifix_count,
                s.smll_count,
                s.idiv_count,
            ),
        });
    lock.store(false, Ordering::SeqCst);

    result
}

#[tauri::command]
pub async fn list_stock_price_history(
    db: tauri::State<'_, DatabaseConnection>,
    ticker: String,
) -> Result<Vec<stock_price_history::Model>, AppError> {
    let history = stock_price_history::Entity::find()
        .filter(stock_price_history::Column::Ticker.eq(ticker))
        .order_by_asc(stock_price_history::Column::PriceDate)
        .all(db.inner())
        .await?;

    Ok(history)
}

// Fase 10, item 8, Sessão 51 — pedido explícito do dono do projeto: Fear &
// Greed Index em "toda tela que for cripto". Global (não por coin), então
// só a leitura mais recente interessa — `run_stock_collector` com
// `asset_class: "cripto"` já mantém `crypto_fear_greed` em dia (ver
// `collect_crypto_ticker` no coletor Python), esta é só a leitura.
#[tauri::command]
pub async fn get_latest_crypto_fear_greed(
    db: tauri::State<'_, DatabaseConnection>,
) -> Result<Option<crypto_fear_greed::Model>, AppError> {
    let latest = crypto_fear_greed::Entity::find()
        .order_by_desc(crypto_fear_greed::Column::ReadingDate)
        .one(db.inner())
        .await?;

    Ok(latest)
}
