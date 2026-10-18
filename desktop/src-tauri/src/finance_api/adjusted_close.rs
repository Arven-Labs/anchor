// Fase 22, fatia 1 — coleta o fechamento ajustado (dividendos e desdobramentos) do Yahoo, a base do retorno total
// do backtest. Grava em `stock_adjusted_close`, uma linha por `(ticker, price_date)`.
//
// Diferente de `stock_price_history` (`ON CONFLICT DO NOTHING`), aqui o conflito **sobrescreve**: o ajuste de um
// pregão antigo muda toda vez que o ativo paga um dividendo, então a série inteira tem de ser regravada.
use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, Set};

use crate::entity::stock_adjusted_close;
use crate::error::AppError;
use crate::finance_core::market::Instrument;
use crate::finance_core::yahoo;

/// Linhas por `INSERT` (4 colunas cada), longe do limite de parâmetros do SQLite.
const CHUNK: usize = 500;

/// Baixa e grava o fechamento ajustado de cada ticker; devolve quantos pontos gravou. `global = false` é ação, FII,
/// ETF ou BDR da B3 (sufixo `.SA`); `true` é ativo estrangeiro. Ticker sem dado no Yahoo é pulado.
pub async fn collect_adjusted_close(db: &DatabaseConnection, tickers: &[String], global: bool) -> Result<usize, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut total = 0usize;
    for ticker in tickers {
        let instrument = if global { Instrument::global(ticker) } else { Instrument::b3(ticker) };
        let history = match yahoo::adjusted_close_history(&instrument).await {
            Ok(history) => history,
            Err(AppError::NoData(_)) => continue,
            Err(err) => return Err(err),
        };
        total += store(db, ticker, &history, &now).await?;
    }
    Ok(total)
}

/// Grava uma série já lida (separado da rede para o teste de fluxo semear sem sair do processo).
pub async fn store(
    db: &DatabaseConnection,
    ticker: &str,
    history: &[(chrono::NaiveDate, f64)],
    fetched_at: &str,
) -> Result<usize, AppError> {
    for chunk in history.chunks(CHUNK) {
        let models: Vec<stock_adjusted_close::ActiveModel> = chunk
            .iter()
            .map(|(date, adjusted)| stock_adjusted_close::ActiveModel {
                ticker: Set(ticker.to_string()),
                price_date: Set(date.to_string()),
                adj_close_price: Set(*adjusted),
                fetched_at: Set(fetched_at.to_string()),
            })
            .collect();
        stock_adjusted_close::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([stock_adjusted_close::Column::Ticker, stock_adjusted_close::Column::PriceDate])
                    .update_columns([stock_adjusted_close::Column::AdjClosePrice, stock_adjusted_close::Column::FetchedAt])
                    .to_owned(),
            )
            .exec(db)
            .await?;
    }
    Ok(history.len())
}
