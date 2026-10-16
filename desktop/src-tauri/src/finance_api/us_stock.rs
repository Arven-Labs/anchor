// Fase 14.4 (fatia final, Sessão 92) — porta pra Rust a lógica de fetch+write
// de `data-collector/main.py::main_us_stock`/`main_etf_us` (ação americana
// comum e ETF-US, Yahoo sem sufixo `.SA` — desbloqueado pela Fase 1.11 do
// easybusiness). Escreve nas **mesmas tabelas** que `finance_api::stocks` já
// usa pra Ação BR (`stock_quotes`/`stock_technicals`/`stock_dividends_avg`/
// `stock_price_history`/`stock_dividend_payments`) — não existe tabela
// `us_stock_*` separada do lado Rust, mesma decisão que o coletor Python já
// tomava (`INSERT INTO stock_quotes ...` dentro de `collect_us_stock_quotes`).
// `run_reit_collector` (`finance_api::reit`) reaproveita as 5 funções deste
// módulo antes de acrescentar os indicadores imobiliários.
use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, Set};

use crate::entity::{
    stock_dividend_payments, stock_dividends_avg, stock_price_history, stock_quotes,
    stock_technicals,
};
use crate::error::AppError;
use crate::finance_api::{client, insert_ignoring_conflicts, skip_not_found};

const SOURCE: &str = "yahoo_finance";

pub async fn collect_quotes(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<Vec<client::UsStockQuoteResponse>, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut quotes = Vec::new();
    let mut models = Vec::new();

    for ticker in tickers {
        let quote = client::fetch_us_stock_quote(ticker).await?;
        models.push(stock_quotes::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            ticker: Set(quote.ticker.clone()),
            price: Set(quote.price),
            source: Set(SOURCE.to_string()),
            fetched_at: Set(now.clone()),
            name: Set(quote.name.clone()),
            exchange: Set(quote.exchange.clone()),
            currency: Set(quote.currency.clone()),
        });
        quotes.push(quote);
    }

    if !models.is_empty() {
        stock_quotes::Entity::insert_many(models).exec(db).await?;
    }

    Ok(quotes)
}

pub async fn collect_dividends_avg(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<Vec<client::UsStockDividendsAvgResponse>, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut results = Vec::new();
    let mut models = Vec::new();

    for ticker in tickers {
        let Some(item) =
            skip_not_found(client::fetch_us_stock_dividends_avg(ticker)).await?
        else {
            continue;
        };
        models.push(stock_dividends_avg::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            ticker: Set(item.ticker.clone()),
            avg_dividend_5y: Set(item.avg_dividend_5y),
            source: Set(SOURCE.to_string()),
            fetched_at: Set(now.clone()),
        });
        results.push(item);
    }

    if !models.is_empty() {
        stock_dividends_avg::Entity::insert_many(models).exec(db).await?;
    }

    Ok(results)
}

pub async fn collect_technicals(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<Vec<client::UsStockTechnicalsResponse>, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut results = Vec::new();
    let mut models = Vec::new();

    for ticker in tickers {
        let item = client::fetch_us_stock_technicals(ticker).await?;
        models.push(stock_technicals::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            ticker: Set(item.ticker.clone()),
            sma_50: Set(item.sma_50),
            sma_100: Set(item.sma_100),
            sma_200: Set(item.sma_200),
            cagr_5y: Set(item.cagr_5y),
            cagr_10y: Set(item.cagr_10y),
            source: Set(SOURCE.to_string()),
            fetched_at: Set(now.clone()),
        });
        results.push(item);
    }

    if !models.is_empty() {
        stock_technicals::Entity::insert_many(models).exec(db).await?;
    }

    Ok(results)
}

pub async fn collect_dividend_payments(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<usize, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut total = 0usize;
    let mut models = Vec::new();

    for ticker in tickers {
        let response = client::fetch_us_stock_dividend_payments(ticker).await?;
        total += response.data.len();
        for point in response.data {
            models.push(stock_dividend_payments::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                ticker: Set(response.ticker.clone()),
                payment_date: Set(point.payment_date.to_string()),
                amount: Set(point.amount),
                price_at_payment: Set(point.price_at_payment),
                yield_pct: Set(point.yield_pct),
                source: Set(SOURCE.to_string()),
                fetched_at: Set(now.clone()),
            });
        }
    }

    insert_ignoring_conflicts::<stock_dividend_payments::Entity>(
        db,
        models,
        OnConflict::columns([
            stock_dividend_payments::Column::Ticker,
            stock_dividend_payments::Column::PaymentDate,
        ])
        .do_nothing()
        .to_owned(),
    )
    .await?;

    Ok(total)
}

/// Reaproveitada por `finance_api::benchmark::collect_benchmarks` pro IBOV
/// (`^BVSP`) — é literalmente "ticker sem sufixo", sem tratamento especial,
/// mesma decisão que `data-collector/main.py::collect_benchmark_returns` já
/// tomava (`collect_us_price_history(["^BVSP"])`, mesma função usada aqui).
pub async fn collect_price_history(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<usize, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut total = 0usize;
    let mut models = Vec::new();

    for ticker in tickers {
        let response = client::fetch_us_stock_price_history(ticker).await?;
        total += response.data.len();
        for point in response.data {
            models.push(stock_price_history::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                ticker: Set(response.ticker.clone()),
                price_date: Set(point.price_date.to_string()),
                close_price: Set(point.close_price),
                source: Set(SOURCE.to_string()),
                fetched_at: Set(now.clone()),
            });
        }
    }

    insert_ignoring_conflicts::<stock_price_history::Entity>(
        db,
        models,
        OnConflict::columns([
            stock_price_history::Column::Ticker,
            stock_price_history::Column::PriceDate,
        ])
        .do_nothing()
        .to_owned(),
    )
    .await?;

    Ok(total)
}

/// ETF-US (`main_etf_us`) — sem fundamentos/DCF nem indicadores dedicados,
/// ETF não tem demonstração financeira própria na SEC (mesmo motivo pelo
/// qual `etf_br` também não busca fundamentos).
pub async fn run_etf_us_collector(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<String, AppError> {
    let quotes = collect_quotes(db,tickers).await?;
    let dividends_avg = collect_dividends_avg(db,tickers).await?;
    let technicals = collect_technicals(db,tickers).await?;
    let payments = collect_dividend_payments(db,tickers).await?;
    let price_history = collect_price_history(db,tickers).await?;

    Ok(format!(
        "Updated {} quote(s), {} dividend average record(s), {} technicals record(s), \
         {} dividend payment(s), {} price history point(s)",
        quotes.len(),
        dividends_avg.len(),
        technicals.len(),
        payments,
        price_history,
    ))
}

/// Ação americana comum (`main_us_stock`) — os 5 recursos Yahoo acima +
/// fundamentos/DCF via SEC EDGAR, já servidos pela Finance API desde a Fase
/// 1.7 (`client::fetch_us_stock_fundamentals/payout/dcf_fundamentals`, sem
/// chamador até agora). Mesmo contrato do Python: sem CIK resolvido (ex.
/// `SEC_EDGAR_CONTACT_EMAIL` ausente do lado da API) o passo de fundamentos
/// falha e propaga — quem decide "pula e não derruba o resto" é o chamador
/// em `commands::collector`, igual ao `try/except RuntimeError` do
/// `main_us_stock` original.
pub async fn run_us_stock_collector(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<String, AppError> {
    let quotes = collect_quotes(db,tickers).await?;
    let dividends_avg = collect_dividends_avg(db,tickers).await?;
    let technicals = collect_technicals(db,tickers).await?;
    let payments = collect_dividend_payments(db,tickers).await?;
    let price_history = collect_price_history(db,tickers).await?;

    let mut summary = format!(
        "Updated {} quote(s), {} dividend average record(s), {} technicals record(s), \
         {} dividend payment(s), {} price history point(s)",
        quotes.len(),
        dividends_avg.len(),
        technicals.len(),
        payments,
        price_history,
    );

    let fundamentals = collect_us_fundamentals(db,tickers).await?;
    if fundamentals.is_empty() {
        return Ok(summary);
    }

    let dcf_count = collect_us_dcf_fundamentals(db,&fundamentals).await?;
    summary.push_str(&format!(
        ", {} fundamentals record(s), {} DCF fundamentals record(s)",
        fundamentals.len(),
        dcf_count,
    ));

    Ok(summary)
}

/// Linha intermediária entre fundamentos e DCF — mesmo papel de
/// `finance_api::stocks::StockFundamentalsRow`, mas sem `cvm_code`: o DCF de
/// ação americana busca direto por `ticker` (`fetch_us_stock_dcf_fundamentals`),
/// não precisa resolver nenhum código intermediário.
struct UsStockFundamentalsRow {
    ticker: String,
    lpa: f64,
    vpa: f64,
    roe: f64,
    payout: Option<f64>,
    shares_outstanding: f64,
}

/// LPA/VPA/ROE/Payout via SEC EDGAR — mais simples que a versão BR
/// (`finance_api::stocks::collect_fundamentals`): a SEC já dá o ROE
/// calculado direto de `NetIncomeLoss`/`StockholdersEquity`, sem a
/// instabilidade trimestral-vs-TTM que o campo `roe` da bolsai tinha (por
/// isso não precisa de uma segunda fonte pra corrigir o ROE, como a versão
/// BR faz com a CVM). `payout` é opcional — um ticker sem payout na SEC
/// continua sendo gravado, só com `payout = None`.
async fn collect_us_fundamentals(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<Vec<UsStockFundamentalsRow>, AppError> {
    use crate::entity::stock_fundamentals;

    let now = Utc::now().to_rfc3339();
    let mut rows = Vec::new();
    let mut models = Vec::new();

    for ticker in tickers {
        let Some(fundamentals) =
            skip_not_found(client::fetch_us_stock_fundamentals(ticker)).await?
        else {
            continue;
        };
        let payout = skip_not_found(client::fetch_us_stock_payout(ticker)).await?;

        let row = UsStockFundamentalsRow {
            ticker: fundamentals.ticker.clone(),
            lpa: fundamentals.lpa,
            vpa: fundamentals.vpa,
            roe: fundamentals.roe,
            payout: payout.map(|p| p.payout_avg_5y),
            shares_outstanding: fundamentals.shares_outstanding,
        };

        models.push(stock_fundamentals::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            ticker: Set(row.ticker.clone()),
            lpa: Set(row.lpa),
            vpa: Set(row.vpa),
            roe: Set(row.roe),
            source: Set("sec_edgar".to_string()),
            fetched_at: Set(now.clone()),
            payout: Set(row.payout),
        });
        rows.push(row);
    }

    if !models.is_empty() {
        stock_fundamentals::Entity::insert_many(models).exec(db).await?;
    }

    Ok(rows)
}

async fn collect_us_dcf_fundamentals(
    db: &DatabaseConnection,
    fundamentals: &[UsStockFundamentalsRow],
) -> Result<usize, AppError> {
    use crate::entity::stock_dcf_fundamentals;

    let now = Utc::now().to_rfc3339();
    let mut models = Vec::new();

    for row in fundamentals {
        let Some(dcf) =
            skip_not_found(client::fetch_us_stock_dcf_fundamentals(&row.ticker)).await?
        else {
            continue;
        };
        let f = dcf.fundamentals;
        models.push(stock_dcf_fundamentals::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            ticker: Set(row.ticker.clone()),
            reference_year: Set(f.reference_year),
            ebit: Set(f.ebit),
            depreciation_amortization: Set(f.depreciation_amortization),
            capex: Set(f.capex),
            nwc_change: Set(f.nwc_change),
            total_debt: Set(f.total_debt),
            cash: Set(f.cash),
            shares_outstanding: Set(row.shares_outstanding / 1_000_000.0),
            source: Set("sec_edgar".to_string()),
            fetched_at: Set(now.clone()),
            tax_rate: Set(f.tax_rate),
            revenue: Set(Some(f.revenue)),
            inventory: Set(Some(f.inventory)),
        });
    }

    let count = models.len();
    if !models.is_empty() {
        stock_dcf_fundamentals::Entity::insert_many(models).exec(db).await?;
    }

    Ok(count)
}

// ---------------------------------------------------------------------------
// Testes `#[ignore]` — mesma convenção de `finance_api::stocks::tests`:
// vão às fontes públicas de verdade (rede) e precisam do banco real de dev.
// Rodar com `cargo test --lib -- --ignored finance_api::us_stock`.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ColumnTrait, Database, QueryFilter};

    async fn dev_db() -> DatabaseConnection {
        Database::connect("sqlite:///data/anchor.db?mode=rwc")
            .await
            .expect("failed to connect to the real dev database")
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_quotes_writes_a_real_row() {
        let db = dev_db().await;
        let ticker = "AAPL".to_string();

        let quotes = collect_quotes(&db,&[ticker.clone()]).await.unwrap();
        assert_eq!(quotes.len(), 1);
        assert!(quotes[0].price > 0.0);

        let rows = stock_quotes::Entity::find()
            .filter(stock_quotes::Column::Ticker.eq(ticker))
            .all(&db)
            .await
            .unwrap();
        assert!(!rows.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_technicals_and_dividends_avg_write_real_rows() {
        let db = dev_db().await;
        let tickers = vec!["AAPL".to_string()];

        let technicals = collect_technicals(&db,&tickers).await.unwrap();
        assert_eq!(technicals.len(), 1);

        let dividends_avg = collect_dividends_avg(&db,&tickers).await.unwrap();
        assert_eq!(dividends_avg.len(), 1);
        assert!(dividends_avg[0].avg_dividend_5y >= 0.0);
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_price_history_is_idempotent() {
        let db = dev_db().await;
        let tickers = vec!["AAPL".to_string()];

        let first = collect_price_history(&db,&tickers).await.unwrap();
        assert!(first > 0);

        let before = stock_price_history::Entity::find()
            .filter(stock_price_history::Column::Ticker.eq("AAPL"))
            .all(&db)
            .await
            .unwrap()
            .len();
        collect_price_history(&db,&tickers).await.unwrap();
        let after = stock_price_history::Entity::find()
            .filter(stock_price_history::Column::Ticker.eq("AAPL"))
            .all(&db)
            .await
            .unwrap()
            .len();
        assert_eq!(before, after, "re-running price history must not duplicate rows");
    }

    #[tokio::test]
    #[ignore]
    async fn live_run_etf_us_collector_writes_real_rows() {
        let db = dev_db().await;

        let summary = run_etf_us_collector(&db,&["IVV".to_string()])
            .await
            .unwrap();
        assert!(summary.contains("Updated"));
    }
}
