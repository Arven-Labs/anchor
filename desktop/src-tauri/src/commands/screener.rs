// Screener de ações (ROADMAP, Sessão 340): filtra os tickers que o Anchor já coletou.
// O universo é o que está no banco (os que o dono buscou, tem em carteira ou em
// watchlist), não a B3 inteira: as coletas são sob demanda, por ticker.

use std::collections::HashMap;

use chrono::{Duration, NaiveDate, Utc};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};

use crate::entity::{stock_dividend_payments, stock_dividends_avg, stock_fundamentals, stock_quotes};
use crate::error::AppError;

/// Cada filtro é opcional. Valores em percentual onde o indicador é percentual
/// (`roe_min` e `dy_min`: 15 significa 15%). Filtro ligado exclui o ticker cujo
/// indicador não existe (ex.: P/L de empresa com LPA ≤ 0). `dy_min` compara o
/// rendimento de dividendos dos últimos 12 meses (`dividend_yield_12m`), não a
/// média de 5 anos.
#[derive(Debug, Default, Deserialize)]
pub struct ScreenerFilters {
    pub pe_max: Option<f64>,
    pub pb_max: Option<f64>,
    pub roe_min: Option<f64>,
    pub dy_min: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ScreenerRow {
    pub ticker: String,
    pub name: Option<String>,
    pub currency: Option<String>,
    pub price: f64,
    /// Preço ÷ LPA. `None` com LPA ≤ 0 ou preço ≤ 0.
    pub pe: Option<f64>,
    /// Preço ÷ VPA. `None` com VPA ≤ 0 ou preço ≤ 0.
    pub pb: Option<f64>,
    /// Percentual, como guardado em `stock_fundamentals`.
    pub roe: f64,
    pub payout: Option<f64>,
    /// Dividendos pagos nos últimos 12 meses ÷ preço, em percentual. `None` sem nenhum
    /// pagamento coletado; `0` com pagamentos coletados mas nenhum na janela.
    pub dividend_yield_12m: Option<f64>,
    /// Dividendo médio de 5 anos ÷ preço, em percentual. `None` sem dividendo coletado.
    pub dividend_yield_5y_avg: Option<f64>,
    /// Data da coleta mais recente dos fundamentos, para a tela mostrar a idade do dado.
    pub fundamentals_fetched_at: String,
}

#[derive(Debug, Serialize)]
pub struct ScreenerResult {
    /// Quantos tickers têm preço e fundamentos na base (o universo, antes dos filtros).
    pub universe_size: usize,
    pub rows: Vec<ScreenerRow>,
}

impl ScreenerFilters {
    fn validate(&self) -> Result<(), AppError> {
        let named = [
            ("pe_max", self.pe_max),
            ("pb_max", self.pb_max),
            ("roe_min", self.roe_min),
            ("dy_min", self.dy_min),
        ];
        for (name, value) in named {
            if value.is_some_and(|v| !v.is_finite()) {
                return Err(AppError::InvalidInput(format!("filtro {name} precisa ser um número finito")));
            }
        }
        Ok(())
    }

    /// Filtro ligado exclui a linha cujo indicador não existe.
    fn accepts(&self, row: &ScreenerRow) -> bool {
        let max_ok = |limit: Option<f64>, value: Option<f64>| limit.is_none_or(|l| value.is_some_and(|v| v <= l));
        let min_ok = |limit: Option<f64>, value: Option<f64>| limit.is_none_or(|l| value.is_some_and(|v| v >= l));
        max_ok(self.pe_max, row.pe)
            && max_ok(self.pb_max, row.pb)
            && min_ok(self.roe_min, Some(row.roe))
            && min_ok(self.dy_min, row.dividend_yield_12m)
    }
}

const TRAILING_WINDOW_DAYS: i64 = 365;

/// Soma dos dividendos dos últimos 12 meses: pagamentos com
/// `today - 365 dias < data <= today` (data futura não conta). `None` sem nenhum
/// pagamento coletado; `Some(0.0)` com pagamentos coletados mas nenhum na janela
/// (parou de pagar). `payments` = (data, valor por ação).
fn trailing_12m_dividends(payments: &[(NaiveDate, f64)], today: NaiveDate) -> Option<f64> {
    if payments.is_empty() {
        return None;
    }
    let window_start = today - Duration::days(TRAILING_WINDOW_DAYS);
    Some(payments.iter().filter(|(date, _)| *date > window_start && *date <= today).map(|(_, amount)| amount).sum())
}

/// `numerator / price`, só quando os dois são positivos (LPA ≤ 0 ou preço ≤ 0 não dão razão útil).
fn ratio(price: f64, per_share: f64) -> Option<f64> {
    (price > 0.0 && per_share > 0.0).then(|| price / per_share)
}

/// A linha mais recente de cada ticker. As coletas inserem uma linha nova a cada
/// rodada, então o histórico fica na tabela; a de maior `fetched_at` (e, no
/// empate, a de maior `id`) vale.
fn latest_by_ticker<T>(rows: Vec<T>, key: impl Fn(&T) -> (&str, &str, i32)) -> HashMap<String, T> {
    let mut latest: HashMap<String, T> = HashMap::new();
    for row in rows {
        let (ticker, at, id) = key(&row);
        let newer = latest.get(ticker).is_none_or(|current| {
            let (_, current_at, current_id) = key(current);
            (at, id) > (current_at, current_id)
        });
        if newer {
            latest.insert(ticker.to_string(), row);
        }
    }
    latest
}

pub async fn screen_stocks_in(
    db: &DatabaseConnection,
    filters: &ScreenerFilters,
) -> Result<ScreenerResult, AppError> {
    screen_stocks_on(db, filters, Utc::now().date_naive()).await
}

/// `today` é entrada (não lê o relógio) para a janela de 12 meses ser testável.
pub async fn screen_stocks_on(
    db: &DatabaseConnection,
    filters: &ScreenerFilters,
    today: NaiveDate,
) -> Result<ScreenerResult, AppError> {
    filters.validate()?;

    let quotes = latest_by_ticker(stock_quotes::Entity::find().all(db).await?, |q| {
        (q.ticker.as_str(), q.fetched_at.as_str(), q.id)
    });
    let fundamentals = latest_by_ticker(stock_fundamentals::Entity::find().all(db).await?, |f| {
        (f.ticker.as_str(), f.fetched_at.as_str(), f.id)
    });
    let dividends = latest_by_ticker(stock_dividends_avg::Entity::find().all(db).await?, |d| {
        (d.ticker.as_str(), d.fetched_at.as_str(), d.id)
    });
    let mut payments: HashMap<String, Vec<(NaiveDate, f64)>> = HashMap::new();
    for payment in stock_dividend_payments::Entity::find().all(db).await? {
        if let Ok(date) = NaiveDate::parse_from_str(&payment.payment_date, "%Y-%m-%d") {
            payments.entry(payment.ticker).or_default().push((date, payment.amount));
        }
    }

    let mut universe_size = 0;
    let mut rows = Vec::new();
    for (ticker, fund) in &fundamentals {
        let Some(quote) = quotes.get(ticker) else { continue };
        universe_size += 1;
        let row = ScreenerRow {
            ticker: ticker.clone(),
            name: quote.name.clone(),
            currency: quote.currency.clone(),
            price: quote.price,
            pe: ratio(quote.price, fund.lpa),
            pb: ratio(quote.price, fund.vpa),
            roe: fund.roe,
            payout: fund.payout,
            dividend_yield_12m: payments
                .get(ticker)
                .and_then(|paid| trailing_12m_dividends(paid, today))
                .filter(|_| quote.price > 0.0)
                .map(|total| total / quote.price * 100.0),
            dividend_yield_5y_avg: dividends
                .get(ticker)
                .filter(|_| quote.price > 0.0)
                .map(|d| d.avg_dividend_5y / quote.price * 100.0),
            fundamentals_fetched_at: fund.fetched_at.clone(),
        };
        if filters.accepts(&row) {
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| a.ticker.cmp(&b.ticker));
    Ok(ScreenerResult { universe_size, rows })
}

#[tauri::command]
pub async fn screen_stocks(
    db: tauri::State<'_, DatabaseConnection>,
    filters: ScreenerFilters,
) -> Result<ScreenerResult, AppError> {
    screen_stocks_in(db.inner(), &filters).await
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

    use super::*;
    use crate::entity::{stock_dividends_avg, stock_fundamentals, stock_quotes};
    use crate::test_support::memory_db;

    async fn quote(db: &DatabaseConnection, ticker: &str, price: f64, at: &str) {
        stock_quotes::ActiveModel {
            ticker: Set(ticker.to_string()),
            price: Set(price),
            source: Set("test".to_string()),
            fetched_at: Set(at.to_string()),
            name: Set(Some(format!("{ticker} S.A."))),
            exchange: Set(None),
            currency: Set(Some("BRL".to_string())),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert quote");
    }

    async fn fundamentals(db: &DatabaseConnection, ticker: &str, lpa: f64, vpa: f64, roe: f64, at: &str) {
        stock_fundamentals::ActiveModel {
            ticker: Set(ticker.to_string()),
            lpa: Set(lpa),
            vpa: Set(vpa),
            roe: Set(roe),
            source: Set("test".to_string()),
            fetched_at: Set(at.to_string()),
            payout: Set(Some(0.4)),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert fundamentals");
    }

    async fn dividend(db: &DatabaseConnection, ticker: &str, avg: f64, at: &str) {
        stock_dividends_avg::ActiveModel {
            ticker: Set(ticker.to_string()),
            avg_dividend_5y: Set(avg),
            source: Set("test".to_string()),
            fetched_at: Set(at.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert dividend");
    }

    /// Ticker completo: preço, fundamentos e dividendo, tudo coletado em 2026-11-01.
    async fn full(db: &DatabaseConnection, ticker: &str, price: f64, lpa: f64, vpa: f64, roe: f64, div: f64) {
        let at = "2026-11-01T10:00:00+00:00";
        quote(db, ticker, price, at).await;
        fundamentals(db, ticker, lpa, vpa, roe, at).await;
        dividend(db, ticker, div, at).await;
    }

    fn tickers(result: &ScreenerResult) -> Vec<&str> {
        let mut t: Vec<&str> = result.rows.iter().map(|r| r.ticker.as_str()).collect();
        t.sort_unstable();
        t
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 17).unwrap()
    }

    /// Pagamento de dividendo `days_ago` dias antes de `today()` (negativo = no futuro).
    async fn paid(db: &DatabaseConnection, ticker: &str, days_ago: i64, amount: f64) {
        let date = today() - chrono::Duration::days(days_ago);
        stock_dividend_payments::ActiveModel {
            ticker: Set(ticker.to_string()),
            payment_date: Set(date.to_string()),
            amount: Set(amount),
            source: Set("test".to_string()),
            fetched_at: Set("2026-11-01T10:00:00+00:00".to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert dividend payment");
    }

    fn on(days_ago: i64, amount: f64) -> (NaiveDate, f64) {
        (today() - chrono::Duration::days(days_ago), amount)
    }

    #[test]
    fn trailing_12m_sums_only_the_payments_inside_the_window() {
        let payments = [on(60, 1.0), on(200, 1.0), on(400, 5.0)];
        assert_eq!(trailing_12m_dividends(&payments, today()), Some(2.0));
    }

    #[test]
    fn trailing_12m_window_includes_364_days_ago_and_excludes_365() {
        assert_eq!(trailing_12m_dividends(&[on(364, 1.0), on(365, 100.0)], today()), Some(1.0));
    }

    #[test]
    fn trailing_12m_includes_today_and_ignores_future_dates() {
        assert_eq!(trailing_12m_dividends(&[on(0, 1.0), on(-1, 50.0)], today()), Some(1.0));
    }

    #[test]
    fn trailing_12m_is_zero_when_payments_exist_but_none_in_the_window() {
        assert_eq!(trailing_12m_dividends(&[on(400, 5.0)], today()), Some(0.0));
    }

    #[test]
    fn trailing_12m_is_none_without_any_payment() {
        assert_eq!(trailing_12m_dividends(&[], today()), None);
    }

    #[tokio::test]
    async fn the_12m_yield_is_the_window_total_over_the_current_price() {
        let db = memory_db().await;
        full(&db, "PAY3", 20.0, 2.0, 10.0, 15.0, 3.0).await;
        paid(&db, "PAY3", 60, 1.0).await;
        paid(&db, "PAY3", 200, 1.0).await;
        paid(&db, "PAY3", 400, 5.0).await; // fora da janela

        let result = screen_stocks_on(&db, &ScreenerFilters::default(), today()).await.unwrap();

        let row = &result.rows[0];
        assert!(close(row.dividend_yield_12m.unwrap(), 10.0), "2 / 20: {row:?}");
        assert!(close(row.dividend_yield_5y_avg.unwrap(), 15.0), "3 / 20: {row:?}");
    }

    #[tokio::test]
    async fn dy_min_filters_the_12m_yield_not_the_five_year_average() {
        let db = memory_db().await;
        // A: média de 5 anos alta (DY 20%), mas só 1% nos últimos 12 meses.
        full(&db, "A3", 10.0, 2.0, 10.0, 15.0, 2.0).await;
        paid(&db, "A3", 30, 0.1).await;
        // B: média de 5 anos baixa (DY 1%), mas 10% nos últimos 12 meses.
        full(&db, "B3", 10.0, 2.0, 10.0, 15.0, 0.1).await;
        paid(&db, "B3", 30, 1.0).await;

        let result = screen_stocks_on(&db, &ScreenerFilters { dy_min: Some(5.0), ..Default::default() }, today())
            .await
            .unwrap();

        assert_eq!(tickers(&result), vec!["B3"]);
    }

    #[tokio::test]
    async fn a_ticker_that_stopped_paying_has_zero_12m_yield_and_a_never_collected_one_has_none() {
        let db = memory_db().await;
        full(&db, "STOP3", 10.0, 2.0, 10.0, 15.0, 1.0).await;
        paid(&db, "STOP3", 400, 5.0).await;
        full(&db, "NEW3", 10.0, 2.0, 10.0, 15.0, 1.0).await;

        let open = screen_stocks_on(&db, &ScreenerFilters::default(), today()).await.unwrap();
        let by = |t: &str| open.rows.iter().find(|r| r.ticker == t).unwrap().dividend_yield_12m;
        assert_eq!(by("STOP3"), Some(0.0));
        assert_eq!(by("NEW3"), None);

        let zero = screen_stocks_on(&db, &ScreenerFilters { dy_min: Some(0.0), ..Default::default() }, today())
            .await
            .unwrap();
        assert_eq!(tickers(&zero), vec!["STOP3"], "0% é dado real; sem coleta não passa");
        let one = screen_stocks_on(&db, &ScreenerFilters { dy_min: Some(1.0), ..Default::default() }, today())
            .await
            .unwrap();
        assert!(one.rows.is_empty());
    }

    #[tokio::test]
    async fn another_tickers_payments_do_not_count() {
        let db = memory_db().await;
        full(&db, "MINE3", 10.0, 2.0, 10.0, 15.0, 1.0).await;
        paid(&db, "MINE3", 30, 1.0).await;
        paid(&db, "OTHER3", 30, 9.0).await;

        let result = screen_stocks_on(&db, &ScreenerFilters::default(), today()).await.unwrap();

        assert!(close(result.rows[0].dividend_yield_12m.unwrap(), 10.0), "{:?}", result.rows[0]);
    }

    #[tokio::test]
    async fn derives_pe_pb_and_dividend_yield_from_the_stored_rows() {
        let db = memory_db().await;
        full(&db, "PETR4", 40.0, 5.0, 32.0, 16.0, 3.0).await;

        let result = screen_stocks_in(&db, &ScreenerFilters::default()).await.unwrap();

        assert_eq!(result.universe_size, 1);
        let row = &result.rows[0];
        assert!(close(row.pe.unwrap(), 8.0), "{row:?}");
        assert!(close(row.pb.unwrap(), 1.25), "{row:?}");
        assert!(close(row.dividend_yield_5y_avg.unwrap(), 7.5), "{row:?}");
        assert!(close(row.roe, 16.0) && close(row.price, 40.0));
    }

    #[tokio::test]
    async fn a_company_with_non_positive_eps_has_no_pe_and_is_dropped_by_a_pe_filter() {
        let db = memory_db().await;
        full(&db, "LOSS3", 10.0, -1.0, 5.0, -8.0, 0.0).await;
        full(&db, "GOOD3", 20.0, 2.0, 10.0, 20.0, 1.0).await;

        let open = screen_stocks_in(&db, &ScreenerFilters::default()).await.unwrap();
        assert_eq!(tickers(&open), vec!["GOOD3", "LOSS3"]);
        let loss = open.rows.iter().find(|r| r.ticker == "LOSS3").unwrap();
        assert!(loss.pe.is_none(), "P/L de lucro negativo não existe: {loss:?}");

        let filtered = screen_stocks_in(&db, &ScreenerFilters { pe_max: Some(50.0), ..Default::default() })
            .await
            .unwrap();
        assert_eq!(tickers(&filtered), vec!["GOOD3"]);
        assert_eq!(filtered.universe_size, 2, "o universo conta antes dos filtros");
    }

    #[tokio::test]
    async fn pe_max_keeps_only_the_cheaper_tickers() {
        let db = memory_db().await;
        full(&db, "CHEAP3", 16.0, 2.0, 10.0, 15.0, 1.0).await; // P/L 8
        full(&db, "PRICY3", 40.0, 2.0, 10.0, 15.0, 1.0).await; // P/L 20

        let result = screen_stocks_in(&db, &ScreenerFilters { pe_max: Some(10.0), ..Default::default() })
            .await
            .unwrap();

        assert_eq!(tickers(&result), vec!["CHEAP3"]);
    }

    #[tokio::test]
    async fn filters_combine_with_and() {
        let db = memory_db().await;
        full(&db, "A3", 16.0, 2.0, 16.0, 20.0, 1.6).await; // P/L 8, P/VP 1, ROE 20
        full(&db, "B3", 16.0, 2.0, 16.0, 5.0, 1.6).await; // ROE baixo
        full(&db, "C3", 16.0, 2.0, 16.0, 20.0, 0.16).await;
        paid(&db, "A3", 30, 1.6).await; // DY 12m 10
        paid(&db, "B3", 30, 1.6).await; // DY 12m 10
        paid(&db, "C3", 30, 0.16).await; // DY 12m 1

        let result = screen_stocks_on(
            &db,
            &ScreenerFilters { pe_max: Some(10.0), pb_max: Some(1.5), roe_min: Some(15.0), dy_min: Some(6.0) },
            today(),
        )
        .await
        .unwrap();

        assert_eq!(tickers(&result), vec!["A3"]);
    }

    #[tokio::test]
    async fn a_dividend_filter_drops_tickers_with_no_dividend_collected() {
        let db = memory_db().await;
        let at = "2026-11-01T10:00:00+00:00";
        quote(&db, "NODIV3", 10.0, at).await;
        fundamentals(&db, "NODIV3", 1.0, 5.0, 10.0, at).await;

        let open = screen_stocks_in(&db, &ScreenerFilters::default()).await.unwrap();
        assert!(open.rows[0].dividend_yield_12m.is_none() && open.rows[0].dividend_yield_5y_avg.is_none());

        let filtered = screen_stocks_in(&db, &ScreenerFilters { dy_min: Some(0.0), ..Default::default() })
            .await
            .unwrap();
        assert!(filtered.rows.is_empty(), "sem dividendo coletado não passa num filtro de DY");
    }

    #[tokio::test]
    async fn uses_the_most_recent_row_of_each_table() {
        let db = memory_db().await;
        quote(&db, "VALE3", 10.0, "2026-10-01T10:00:00+00:00").await;
        quote(&db, "VALE3", 50.0, "2026-11-01T10:00:00+00:00").await;
        fundamentals(&db, "VALE3", 1.0, 5.0, 10.0, "2026-10-01T10:00:00+00:00").await;
        fundamentals(&db, "VALE3", 5.0, 25.0, 20.0, "2026-11-01T10:00:00+00:00").await;

        let result = screen_stocks_in(&db, &ScreenerFilters::default()).await.unwrap();

        assert_eq!(result.rows.len(), 1, "um ticker, uma linha, mesmo com histórico");
        let row = &result.rows[0];
        assert!(close(row.price, 50.0) && close(row.roe, 20.0));
        assert!(close(row.pe.unwrap(), 10.0), "50 / 5: {row:?}");
        assert_eq!(row.fundamentals_fetched_at, "2026-11-01T10:00:00+00:00");
    }

    #[tokio::test]
    async fn a_ticker_without_fundamentals_is_outside_the_universe() {
        let db = memory_db().await;
        quote(&db, "BTC", 300_000.0, "2026-11-01T10:00:00+00:00").await; // cripto também grava em stock_quotes

        let result = screen_stocks_in(&db, &ScreenerFilters::default()).await.unwrap();

        assert_eq!(result.universe_size, 0);
        assert!(result.rows.is_empty());
    }

    #[tokio::test]
    async fn a_zero_price_gives_no_ratio_instead_of_infinity() {
        let db = memory_db().await;
        full(&db, "ZERO3", 0.0, 2.0, 10.0, 15.0, 1.0).await;
        paid(&db, "ZERO3", 30, 1.0).await;

        let result = screen_stocks_on(&db, &ScreenerFilters::default(), today()).await.unwrap();

        let row = &result.rows[0];
        assert!(
            row.pe.is_none()
                && row.pb.is_none()
                && row.dividend_yield_12m.is_none()
                && row.dividend_yield_5y_avg.is_none(),
            "{row:?}"
        );
    }

    #[tokio::test]
    async fn an_empty_database_gives_an_empty_screen() {
        let db = memory_db().await;

        let result = screen_stocks_in(&db, &ScreenerFilters::default()).await.unwrap();

        assert_eq!(result.universe_size, 0);
        assert!(result.rows.is_empty());
    }

    #[tokio::test]
    async fn rejects_non_finite_filters() {
        let db = memory_db().await;
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let result = screen_stocks_in(&db, &ScreenerFilters { pe_max: Some(bad), ..Default::default() }).await;
            assert!(matches!(result, Err(AppError::InvalidInput(_))), "pe_max = {bad}: {result:?}");
        }
    }
}
