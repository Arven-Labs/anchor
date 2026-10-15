// Fase 14.4 (fatia final, Sessão 92) — porta `data-collector/main.py::
// main_reit`/`collect_reit_fundamentals` pra Rust, desbloqueado pela Fase
// 1.11.2 do easybusiness (`GET /v1/us-stocks/{ticker}/reit-fundamentals`).
// REIT reaproveita os 5 recursos Yahoo de `finance_api::us_stock` (cotação/
// técnicos/dividendos/histórico de preço/proventos — REIT é só mais um
// ticker NYSE/NASDAQ) e troca fundamentos/DCF por indicadores imobiliários
// dedicados (tabela própria `reit_fundamentals` — sem os 8 modelos de
// valuation, que não encaixam bem em imobiliário).
use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, Set};

use crate::entity::reit_fundamentals;
use crate::error::AppError;
use crate::finance_api::{client, insert_ignoring_conflicts, skip_not_found, us_stock};
use crate::finance_api::FinanceApiHandle;

/// A Finance API já devolve a série histórica completa a cada chamada
/// (append-only do lado dela) — `ON CONFLICT DO NOTHING` no índice
/// `(ticker, reference_year)` é defesa contra duplicar os anos já
/// existentes ao rodar de novo, mesmo espírito do `stock_price_history`.
pub async fn collect_reit_fundamentals(
    db: &DatabaseConnection,
    handle: &FinanceApiHandle,
    tickers: &[String],
) -> Result<usize, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut total = 0usize;
    let mut models = Vec::new();

    for ticker in tickers {
        let Some(response) = skip_not_found(client::fetch_reit_fundamentals(handle, ticker)).await?
        else {
            continue;
        };
        total += response.data.len();
        for point in response.data {
            models.push(reit_fundamentals::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                ticker: Set(response.ticker.clone()),
                reference_year: Set(point.reference_year),
                revenue: Set(point.revenue),
                real_estate_property_net: Set(point.real_estate_property_net),
                real_estate_property_at_cost: Set(point.real_estate_property_at_cost),
                stockholders_equity: Set(point.stockholders_equity),
                net_income: Set(point.net_income),
                eps_diluted: Set(point.eps_diluted),
                source: Set("sec_edgar".to_string()),
                fetched_at: Set(now.clone()),
            });
        }
    }

    insert_ignoring_conflicts::<reit_fundamentals::Entity>(
        db,
        models,
        OnConflict::columns([
            reit_fundamentals::Column::Ticker,
            reit_fundamentals::Column::ReferenceYear,
        ])
        .do_nothing()
        .to_owned(),
    )
    .await?;

    Ok(total)
}

/// Orquestra a mesma sequência de `main_reit`: os 5 recursos Yahoo (via
/// `finance_api::us_stock`) + indicadores imobiliários. Mesmo contrato
/// "pula sem derrubar o resto" do Python (`try/except RuntimeError` em
/// torno só do passo SEC EDGAR) — `skip_not_found` acima já cobre o caso
/// "sem dado pra esse ticker"; um erro de configuração ainda propaga.
pub async fn run_reit_collector(
    db: &DatabaseConnection,
    handle: &FinanceApiHandle,
    tickers: &[String],
) -> Result<String, AppError> {
    let quotes = us_stock::collect_quotes(db, handle, tickers).await?;
    let dividends_avg = us_stock::collect_dividends_avg(db, handle, tickers).await?;
    let technicals = us_stock::collect_technicals(db, handle, tickers).await?;
    let payments = us_stock::collect_dividend_payments(db, handle, tickers).await?;
    let price_history = us_stock::collect_price_history(db, handle, tickers).await?;
    let reit_count = collect_reit_fundamentals(db, handle, tickers).await?;

    Ok(format!(
        "Updated {} quote(s), {} dividend average record(s), {} technicals record(s), \
         {} dividend payment(s), {} price history point(s), {} REIT fundamentals record(s)",
        quotes.len(),
        dividends_avg.len(),
        technicals.len(),
        payments,
        price_history,
        reit_count,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};

    async fn dev_db() -> DatabaseConnection {
        Database::connect("sqlite:///data/anchor.db?mode=rwc")
            .await
            .expect("failed to connect to the real dev database")
    }

    fn handle() -> FinanceApiHandle {
        FinanceApiHandle::for_test(
            "http://localhost:8000".to_string(),
            "local-dev-key-change-me".to_string(),
        )
    }

    #[tokio::test]
    #[ignore]
    async fn live_collect_reit_fundamentals_writes_a_real_row() {
        let db = dev_db().await;
        let handle = handle();
        let ticker = "O".to_string(); // Realty Income — validado ao vivo do lado easybusiness

        let count = collect_reit_fundamentals(&db, &handle, &[ticker.clone()])
            .await
            .unwrap();
        assert!(count > 0);

        let rows = reit_fundamentals::Entity::find()
            .filter(reit_fundamentals::Column::Ticker.eq(ticker))
            .all(&db)
            .await
            .unwrap();
        assert!(!rows.is_empty());
    }
}
