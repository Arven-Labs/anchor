// Fase 14.4 (primeira fatia) — porta pra Rust a lógica de fetch+write de
// `data-collector/main.py` pra Ação BR/FII/ETF-BR/BDR (o caminho padrão
// `--ticker`, sem sufixo de classe especial — cripto/metal/ação
// americana/REIT/ETF-US continuam no coletor Python por enquanto, ver
// `commands::collector::run_stock_collector`). Mesma sequência e mesmas
// regras de escrita do `main()` de lá: `stock_quotes`/`stock_dividends_avg`/
// `stock_technicals`/`stock_fundamentals`/`stock_dcf_fundamentals` sempre
// inserem linha nova (série temporal, nunca sobrescreve); `stock_dividend_
// payments`/`stock_price_history` usam `ON CONFLICT DO NOTHING` no índice
// único `(ticker, data)` — rodar de novo não duplica, só acrescenta linha
// nova.
use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, Set};

use crate::entity::{
    stock_dcf_fundamentals, stock_dividend_payments, stock_dividends_avg, stock_fundamentals,
    stock_price_history, stock_quotes, stock_technicals,
};
use crate::error::AppError;
use crate::finance_api::{client, insert_ignoring_conflicts, skip_not_found};

pub async fn collect_quotes(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<Vec<client::StockQuoteResponse>, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut quotes = Vec::new();
    let mut models = Vec::new();

    for ticker in tickers {
        let quote = client::fetch_stock_quote(ticker).await?;
        models.push(stock_quotes::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            ticker: Set(quote.ticker.clone()),
            price: Set(quote.price),
            source: Set("yahoo_finance".to_string()),
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
) -> Result<Vec<client::StockDividendsAvgResponse>, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut results = Vec::new();
    let mut models = Vec::new();

    for ticker in tickers {
        let Some(item) = skip_not_found(client::fetch_stock_dividends_avg(ticker)).await?
        else {
            continue;
        };
        models.push(stock_dividends_avg::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            ticker: Set(item.ticker.clone()),
            avg_dividend_5y: Set(item.avg_dividend_5y),
            source: Set("yahoo_finance".to_string()),
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
) -> Result<Vec<client::StockTechnicalsResponse>, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut results = Vec::new();
    let mut models = Vec::new();

    for ticker in tickers {
        let item = client::fetch_stock_technicals(ticker).await?;
        models.push(stock_technicals::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            ticker: Set(item.ticker.clone()),
            sma_50: Set(item.sma_50),
            sma_100: Set(item.sma_100),
            sma_200: Set(item.sma_200),
            cagr_5y: Set(item.cagr_5y),
            cagr_10y: Set(item.cagr_10y),
            source: Set("yahoo_finance".to_string()),
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
        let response = client::fetch_stock_dividend_payments(ticker).await?;
        total += response.data.len();
        for point in response.data {
            models.push(stock_dividend_payments::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                ticker: Set(response.ticker.clone()),
                payment_date: Set(point.payment_date.to_string()),
                amount: Set(point.amount),
                price_at_payment: Set(point.price_at_payment),
                yield_pct: Set(point.yield_pct),
                source: Set("yahoo_finance".to_string()),
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

pub async fn collect_price_history(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<usize, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut total = 0usize;
    let mut models = Vec::new();

    for ticker in tickers {
        let response = client::fetch_stock_price_history(ticker).await?;
        total += response.data.len();
        for point in response.data {
            models.push(stock_price_history::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                ticker: Set(response.ticker.clone()),
                price_date: Set(point.price_date.to_string()),
                close_price: Set(point.close_price),
                source: Set("yahoo_finance".to_string()),
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

/// Fundamentos de ação BR — mesma correção do coletor Python (comentário
/// original em `data-collector/main.py::collect_stock_fundamentals`): o
/// `roe` da bolsai mistura trimestral com TTM sem avisar qual é qual
/// (achado real, BPAC11 — bolsai devolveu 3,54% quando o real é 26,6%), por
/// isso é sempre sobrescrito pelo cálculo direto na CVM
/// (`fetch_company_roe`). Ticker sem ROE na CVM é descartado inteiro (não
/// só o campo), pra não reintroduzir silenciosamente o valor errado da
/// bolsai. `payout` é só um acréscimo — nunca teve fonte automática antes
/// desta migração, então um ticker sem payout na CVM continua sendo
/// gravado, só com `payout = None`.
pub struct StockFundamentalsRow {
    pub ticker: String,
    pub cvm_code: i32,
    pub lpa: f64,
    pub vpa: f64,
    pub roe: f64,
    pub payout: Option<f64>,
    pub shares_outstanding: f64,
}

pub async fn collect_fundamentals(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<Vec<StockFundamentalsRow>, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut rows = Vec::new();
    let mut models = Vec::new();

    for ticker in tickers {
        let Some(bolsai) =
            skip_not_found(client::fetch_stock_bolsai_fundamentals(ticker)).await?
        else {
            continue;
        };
        let Some(cvm_code) = bolsai.cvm_code.parse().ok() else {
            continue;
        };
        let Some(roe) = skip_not_found(client::fetch_company_roe(cvm_code)).await? else {
            // Sem ROE extraível na CVM — descarta o ticker inteiro (mesma
            // regra do coletor Python), não só o campo.
            continue;
        };
        let payout = skip_not_found(client::fetch_company_payout(cvm_code)).await?;

        let row = StockFundamentalsRow {
            ticker: bolsai.ticker.clone(),
            cvm_code,
            lpa: bolsai.lpa,
            vpa: bolsai.vpa,
            roe: roe.roe,
            payout: payout.map(|p| p.payout_avg_5y),
            shares_outstanding: bolsai.shares_outstanding,
        };

        models.push(stock_fundamentals::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            ticker: Set(row.ticker.clone()),
            lpa: Set(row.lpa),
            vpa: Set(row.vpa),
            roe: Set(row.roe),
            source: Set("bolsai+cvm_dfp".to_string()),
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

pub async fn collect_dcf_fundamentals(
    db: &DatabaseConnection,
    fundamentals: &[StockFundamentalsRow],
) -> Result<usize, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut models = Vec::new();

    for row in fundamentals {
        let Some(dcf) =
            skip_not_found(client::fetch_company_dcf_fundamentals(row.cvm_code)).await?
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
            source: Set("cvm_dfp".to_string()),
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

/// Orquestra a mesma sequência de `data-collector/main.py::main()` — chamado
/// pelo branch padrão (`.SA`, sem `asset_class` especial) de
/// `commands::collector::run_stock_collector`.
pub async fn run_stock_collector(
    db: &DatabaseConnection,
    tickers: &[String],
) -> Result<String, AppError> {
    let quotes = collect_quotes(db,tickers).await?;
    let dividends_avg = collect_dividends_avg(db,tickers).await?;
    let technicals = collect_technicals(db,tickers).await?;
    let payments = collect_dividend_payments(db,tickers).await?;
    let price_history = collect_price_history(db,tickers).await?;
    let fundamentals = collect_fundamentals(db,tickers).await?;

    let mut summary = format!(
        "Updated {} quote(s), {} dividend average record(s), {} technicals record(s), \
         {} dividend payment(s), {} price history point(s)",
        quotes.len(),
        dividends_avg.len(),
        technicals.len(),
        payments,
        price_history,
    );

    if fundamentals.is_empty() {
        return Ok(summary);
    }

    let dcf_count = collect_dcf_fundamentals(db,&fundamentals).await?;
    summary.push_str(&format!(
        ", {} fundamentals record(s), {} DCF fundamentals record(s)",
        fundamentals.len(),
        dcf_count,
    ));

    Ok(summary)
}

// ---------------------------------------------------------------------------
// Testes `#[ignore]` — mesma convenção de `finance_api::client::tests`:
// vão às fontes públicas de verdade (rede) e precisam do banco real de dev
// (`/data/anchor.db`, dentro do container do Anchor). Rodar com
// `cargo test --lib -- --ignored finance_api::stocks`.
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

    // `run_stock_collector` completo (incluindo o passo de fundamentos via
    // bolsai) não está coberto por um teste `#[ignore]` — precisa de uma
    // `BOLSAI_API_KEY` real (serviço pago de terceiro), que este ambiente de
    // sessão não pôde configurar (bloqueado pelo classificador de permissão
    // do modo auto ao tentar ler `.env` — corretamente, é onde a chave real
    // mora). `collect_quotes`/`collect_technicals`/`collect_dividends_avg`/
    // `collect_dividend_payments`/`collect_price_history` (que não dependem
    // da bolsai) estão cobertos abaixo; `collect_fundamentals`/
    // `collect_dcf_fundamentals` ficam revisados por código, não testados ao
    // vivo nesta sessão.
    #[tokio::test]
    #[ignore]
    async fn live_collect_quotes_writes_a_real_row() {
        let db = dev_db().await;
        let ticker = "PETR4".to_string();

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
        let tickers = vec!["PETR4".to_string()];

        let technicals = collect_technicals(&db,&tickers).await.unwrap();
        assert_eq!(technicals.len(), 1);

        let dividends_avg = collect_dividends_avg(&db,&tickers).await.unwrap();
        assert_eq!(dividends_avg.len(), 1);
        assert!(dividends_avg[0].avg_dividend_5y >= 0.0);
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_dividend_payments_is_idempotent() {
        let db = dev_db().await;
        let tickers = vec!["PETR4".to_string()];

        collect_dividend_payments(&db,&tickers).await.unwrap();
        let before = stock_dividend_payments::Entity::find()
            .filter(stock_dividend_payments::Column::Ticker.eq("PETR4"))
            .all(&db)
            .await
            .unwrap()
            .len();
        collect_dividend_payments(&db,&tickers).await.unwrap();
        let after = stock_dividend_payments::Entity::find()
            .filter(stock_dividend_payments::Column::Ticker.eq("PETR4"))
            .all(&db)
            .await
            .unwrap()
            .len();
        assert_eq!(before, after, "re-running dividend payments must not duplicate rows");
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_price_history_is_idempotent() {
        let db = dev_db().await;
        let tickers = vec!["PETR4".to_string()];

        let first = collect_price_history(&db,&tickers).await.unwrap();
        assert!(first > 0);

        // Rodar de novo não deve duplicar — mesmo índice único (ticker,
        // price_date) que o coletor Python já dependia via INSERT OR IGNORE.
        let before = stock_price_history::Entity::find()
            .filter(stock_price_history::Column::Ticker.eq("PETR4"))
            .all(&db)
            .await
            .unwrap()
            .len();
        collect_price_history(&db,&tickers).await.unwrap();
        let after = stock_price_history::Entity::find()
            .filter(stock_price_history::Column::Ticker.eq("PETR4"))
            .all(&db)
            .await
            .unwrap()
            .len();
        assert_eq!(before, after, "re-running price history must not duplicate rows");
    }

}
