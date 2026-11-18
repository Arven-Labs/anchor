// Fase 22, fatia 1 — coleta o fechamento ajustado (dividendos e desdobramentos) do Yahoo, a base do retorno total
// do backtest. Grava em `stock_adjusted_close`, uma linha por `(ticker, price_date)`.
//
// Diferente de `stock_price_history` (`ON CONFLICT DO NOTHING`), aqui o conflito **sobrescreve**: o ajuste de um
// pregão antigo muda toda vez que o ativo paga um dividendo, então a série inteira tem de ser regravada.
use chrono::{NaiveDate, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, Set};

use crate::domain::currency_conversion::FxHistory;
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

/// Histórico longo de cripto em BRL para o backtest e para a sugestão de valor da permuta. O CoinGecko dá só 365 dias
/// e em USD, e o Yahoo **não tem** par em reais (`BTC-BRL` dá 404, medido em 2026-11-18; o código antigo pulava em
/// silêncio). Então busca `TICKER-USD` (10 anos, diário) e multiplica pelo câmbio USDBRL de `fx`, gravando em reais
/// sob o ticker do ativo (`BTC`). Cripto não paga dividendo, o ajustado é o fechamento. Moeda sem par `-USD` no
/// Yahoo é pulada; o backtest avisa que falta a série.
pub async fn collect_crypto_brl(db: &DatabaseConnection, tickers: &[String], fx: &FxHistory) -> Result<usize, AppError> {
    let now = Utc::now().to_rfc3339();
    let mut total = 0usize;
    for ticker in tickers {
        let instrument = Instrument::global(&format!("{ticker}-USD"));
        let usd = match yahoo::adjusted_close_history(&instrument).await {
            Ok(history) => history,
            Err(AppError::NoData(_)) => continue,
            Err(err) => return Err(err),
        };
        total += store_crypto_brl(db, ticker, &usd, fx, &now).await?;
    }
    Ok(total)
}

/// Converte uma série diária em dólar para reais, dia a dia, pela taxa USDBRL da data mais próxima (`FxHistory`). A
/// cripto negocia todo dia e o câmbio só em dia útil, então sábado e domingo usam a taxa de sexta. Dia sem taxa
/// nenhuma (par ausente) fica de fora em vez de ganhar um valor errado.
pub fn crypto_brl_series(usd: &[(NaiveDate, f64)], fx: &FxHistory) -> Vec<(NaiveDate, f64)> {
    usd.iter()
        .filter_map(|(date, close)| fx.rate_on("USD", "BRL", *date).map(|rate| (*date, close * rate)))
        .collect()
}

/// Converte e grava (separado da rede para o teste de fluxo semear sem sair do processo). Série com pontos mas sem
/// nenhuma taxa USDBRL é recusada com erro, não gravada vazia: o backtest e a permuta tratariam a falta como "moeda
/// sem série" e ninguém saberia que o problema é o câmbio.
pub async fn store_crypto_brl(
    db: &DatabaseConnection,
    ticker: &str,
    usd: &[(NaiveDate, f64)],
    fx: &FxHistory,
    fetched_at: &str,
) -> Result<usize, AppError> {
    let brl = crypto_brl_series(usd, fx);
    if brl.is_empty() && !usd.is_empty() {
        return Err(AppError::NoData(format!(
            "no USD/BRL exchange rate to convert the {ticker} history: collect the pair in Research → FX"
        )));
    }
    store(db, ticker, &brl, fetched_at).await
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
