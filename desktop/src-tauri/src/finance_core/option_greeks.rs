// Gregas e volatilidade implícita de uma série de opção da B3 — porta de
// `lume/api/app/services/option_greeks_service.py`. Junta as peças:
// - a série e o último preço (`b3_options`);
// - o **preço do ativo-objeto, que vem de quem chama** (`underlying_ticker`): a
//   raiz da opção não diz a classe (ON/PN/PNA…) e uma grega calculada com o papel
//   errado seria um erro silencioso;
// - a taxa livre de risco, da curva DI×PRE (`b3_taxa_swap`), interpolada
//   linearmente por **dias corridos** (evita calendário de feriados; o `T` da
//   opção usa a mesma base) e sem extrapolar: fora do intervalo vale o vértice
//   mais próximo. Sem pregão no dia, recua dia a dia, até 10;
// - Black-Scholes europeu (`black_scholes`).
//
// Nada é guardado: o preço do ativo-objeto muda a todo tick, então as gregas são
// recomputadas a cada chamada. A resposta traz `last_trade_date`, porque o preço
// usado pode ter semanas numa série pouco líquida.

use chrono::{Duration as Days, NaiveDate, Utc};

use crate::error::AppError;
use crate::finance_core::b3_options::{self, LastTrade, OptionSeries, OptionsMarket};
use crate::finance_core::b3_taxa_swap::{self, DiVertex};
use crate::finance_core::black_scholes::{self, OptionType};
use crate::finance_core::market::Instrument;
use crate::finance_core::yahoo;

/// Quantos dias recuar procurando a última curva DI (fim de semana e feriados).
const CURVE_LOOKBACK_DAYS: i64 = 10;
/// Falhas de provider toleradas na busca da curva antes de desistir.
const MAX_PROVIDER_FAILURES: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct OptionGreeks {
    pub series_ticker: String,
    pub underlying_ticker: String,
    pub option_type: OptionType,
    pub strike: f64,
    pub expiration: NaiveDate,
    pub spot: f64,
    pub days_to_expiry: i64,
    pub risk_free_rate_pct: f64,
    pub di_curve_date: NaiveDate,
    pub last_trade_date: NaiveDate,
    pub last_price: f64,
    pub implied_volatility_pct: f64,
    pub delta: f64,
    pub gamma: f64,
    /// Por dia corrido.
    pub theta: f64,
    /// Por ponto percentual de volatilidade.
    pub vega: f64,
}

/// Taxa (% a.a.) da curva em `days` dias corridos: linear entre os dois vértices
/// vizinhos e fixa nos extremos. `None` só se a curva estiver vazia.
pub fn rate_at_calendar_days(curve: &[DiVertex], days: i32) -> Option<f64> {
    let mut points: Vec<(i32, f64)> = curve.iter().map(|v| (v.dias_corridos, v.rate_pct)).collect();
    points.sort_by_key(|(d, _)| *d);
    let (first, last) = (*points.first()?, *points.last()?);
    if days <= first.0 {
        return Some(first.1);
    }
    if days >= last.0 {
        return Some(last.1);
    }
    let upper = points.iter().position(|(d, _)| *d >= days)?;
    let ((d0, r0), (d1, r1)) = (points[upper - 1], points[upper]);
    Some(r0 + (r1 - r0) * f64::from(days - d0) / f64::from(d1 - d0))
}

/// A série e o último negócio de `series_ticker`: série desconhecida ou nunca
/// negociada são `NoData` (o Lume devolvia 404 nos dois casos).
pub fn resolve<'a>(market: &'a OptionsMarket, series_ticker: &str) -> Result<(&'a OptionSeries, LastTrade), AppError> {
    let series = market
        .find(series_ticker)
        .ok_or_else(|| AppError::NoData(format!("unknown option series '{series_ticker}'")))?;
    let last = market
        .last_trades
        .get(series_ticker)
        .copied()
        .ok_or_else(|| AppError::NoData(format!("option series '{series_ticker}' has no traded price")))?;
    Ok((series, last))
}

/// Dias até o vencimento; opção vencida (hoje inclusive) é `InvalidInput`.
pub fn days_to_expiry(series: &OptionSeries, today: NaiveDate) -> Result<i64, AppError> {
    let days = (series.expiration - today).num_days();
    if days <= 0 {
        return Err(AppError::InvalidInput(format!(
            "option series '{}' expired on {}",
            series.series_ticker, series.expiration
        )));
    }
    Ok(days)
}

/// Gregas a partir de tudo já resolvido (função pura, sem rede).
pub fn compute(
    series: &OptionSeries,
    last: LastTrade,
    underlying_ticker: &str,
    spot: f64,
    rate_pct: f64,
    di_curve_date: NaiveDate,
    today: NaiveDate,
) -> Result<OptionGreeks, AppError> {
    let days = days_to_expiry(series, today)?;
    let years = days as f64 / 365.0;
    let rate = rate_pct / 100.0;
    let vol = black_scholes::implied_volatility(series.option_type, last.close_price, spot, series.strike, years, rate)
        .ok_or_else(|| {
            AppError::InvalidInput(format!(
                "cannot compute the implied volatility of '{}': no volatility reproduces the last price ({}, traded {}); it is below what the model allows, which usually means a stale price (or dividends/early exercise, which the model ignores)",
                series.series_ticker, last.close_price, last.trade_date
            ))
        })?;
    let greeks = black_scholes::greeks(series.option_type, spot, series.strike, years, rate, vol);
    Ok(OptionGreeks {
        series_ticker: series.series_ticker.clone(),
        underlying_ticker: underlying_ticker.to_string(),
        option_type: series.option_type,
        strike: series.strike,
        expiration: series.expiration,
        spot,
        days_to_expiry: days,
        risk_free_rate_pct: rate_pct,
        di_curve_date,
        last_trade_date: last.trade_date,
        last_price: last.close_price,
        implied_volatility_pct: vol * 100.0,
        delta: greeks.delta,
        gamma: greeks.gamma,
        theta: greeks.theta,
        vega: greeks.vega,
    })
}

/// Última curva DI disponível a partir de `today`, recuando até 10 dias. O
/// `NoData` (sem pregão) recua sempre. A falha do provider também recua, no máximo
/// `MAX_PROVIDER_FAILURES` vezes: o arquivo do dia mais recente costuma sair
/// quebrado ou lento enquanto a B3 ainda o publica (conferido ao vivo: o de ontem
/// deu timeout e os anteriores baixaram), e a resposta informa a data da curva
/// usada. Num defeito de verdade (todas as datas falham) sobe o primeiro erro, sem
/// encadear 10 timeouts.
async fn latest_curve_with<F, Fut>(today: NaiveDate, mut fetch: F) -> Result<(NaiveDate, Vec<DiVertex>), AppError>
where
    F: FnMut(NaiveDate) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<DiVertex>, AppError>>,
{
    let mut first_failure = None;
    let mut failures = 0;
    for back in 0..CURVE_LOOKBACK_DAYS {
        let date = today - Days::days(back);
        match fetch(date).await {
            Ok(curve) => return Ok((date, curve)),
            Err(AppError::NoData(_)) => continue,
            Err(err @ AppError::ProviderUnavailable(_)) => {
                eprintln!("DI curve of {date} unavailable, trying the day before: {err}");
                first_failure.get_or_insert(err);
                failures += 1;
                if failures >= MAX_PROVIDER_FAILURES {
                    break;
                }
            }
            Err(other) => return Err(other),
        }
    }
    Err(first_failure.unwrap_or_else(|| {
        AppError::ProviderUnavailable(format!(
            "no DI curve in the last {CURVE_LOOKBACK_DAYS} days, so there is no risk-free rate"
        ))
    }))
}

async fn latest_curve(today: NaiveDate) -> Result<(NaiveDate, Vec<DiVertex>), AppError> {
    latest_curve_with(today, |date| async move { Ok(b3_taxa_swap::di_curve(date).await?.value.value) }).await
}

/// Gregas de `series_ticker` (ex.: `PETRL522`) usando o preço de
/// `underlying_ticker` (ex.: `PETR4`) como spot.
pub async fn option_greeks(series_ticker: &str, underlying_ticker: &str) -> Result<OptionGreeks, AppError> {
    let series_ticker = series_ticker.trim().to_uppercase();
    let underlying_ticker = underlying_ticker.trim().to_uppercase();
    let today = Utc::now().date_naive();

    let market = b3_options::market().await?.value.value;
    let (series, last) = resolve(&market, &series_ticker)?;
    // Falha cedo, antes de gastar rede com spot e curva numa opção vencida.
    let days = days_to_expiry(series, today)?;

    let spot = yahoo::quote(&Instrument::b3(&underlying_ticker)).await?.value.price;
    let (curve_date, curve) = latest_curve(today).await?;
    let rate_pct = rate_at_calendar_days(&curve, days as i32)
        .ok_or_else(|| AppError::ProviderUnavailable(format!("DI curve of {curve_date} is empty")))?;
    compute(series, last, &underlying_ticker, spot, rate_pct, curve_date, today)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finance_core::b3_options::OptionStyle;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn vertex(corridos: i32, uteis: i32, rate: f64) -> DiVertex {
        DiVertex { dias_corridos: corridos, dias_uteis: uteis, rate_pct: rate, vertice_type: 'F' }
    }

    fn curve() -> Vec<DiVertex> {
        vec![vertex(30, 21, 14.0), vertex(60, 42, 14.5), vertex(120, 84, 13.0)]
    }

    #[test]
    fn rate_interpolates_linearly_by_calendar_days() {
        assert_eq!(rate_at_calendar_days(&curve(), 45), Some(14.25));
        assert_eq!(rate_at_calendar_days(&curve(), 60), Some(14.5));
        assert_eq!(rate_at_calendar_days(&curve(), 90), Some(13.75));
    }

    #[test]
    fn rate_is_clamped_outside_the_curve() {
        assert_eq!(rate_at_calendar_days(&curve(), 5), Some(14.0));
        assert_eq!(rate_at_calendar_days(&curve(), 400), Some(13.0));
    }

    #[test]
    fn rate_of_an_empty_curve_is_none() {
        assert_eq!(rate_at_calendar_days(&[], 30), None);
    }

    fn series(option_type: OptionType, strike: f64, expiration: NaiveDate) -> OptionSeries {
        OptionSeries {
            series_ticker: "PETRL522".into(),
            root: "PETR".into(),
            option_type,
            strike,
            expiration,
            style: OptionStyle::European,
        }
    }

    fn trade(price: f64) -> LastTrade {
        LastTrade { trade_date: date(2026, 9, 15), close_price: price }
    }

    const TODAY: (i32, u32, u32) = (2026, 9, 16);

    fn today() -> NaiveDate {
        date(TODAY.0, TODAY.1, TODAY.2)
    }

    #[test]
    fn greeks_recover_the_volatility_that_priced_the_option() {
        // Preço gerado com σ = 40 % → a IV tem que voltar 40 % e as gregas, o sinal certo.
        let expiry = date(2026, 12, 21);
        let years = (expiry - today()).num_days() as f64 / 365.0;
        let market = black_scholes::price(OptionType::Call, 49.06, 49.91, years, 0.1359, 0.40);
        let g = compute(&series(OptionType::Call, 49.91, expiry), trade(market), "petr4", 49.06, 13.59, date(2026, 9, 15), today()).unwrap();
        assert!((g.implied_volatility_pct - 40.0).abs() < 0.01, "{}", g.implied_volatility_pct);
        assert_eq!(g.days_to_expiry, 96);
        assert_eq!(g.underlying_ticker, "petr4");
        assert!(g.delta > 0.0 && g.delta < 1.0 && g.gamma > 0.0 && g.vega > 0.0 && g.theta < 0.0);
        assert_eq!(g.last_trade_date, date(2026, 9, 15));
    }

    #[test]
    fn put_delta_is_negative() {
        let expiry = date(2026, 12, 21);
        let years = (expiry - today()).num_days() as f64 / 365.0;
        let market = black_scholes::price(OptionType::Put, 49.06, 49.91, years, 0.1359, 0.35);
        let g = compute(&series(OptionType::Put, 49.91, expiry), trade(market), "PETR4", 49.06, 13.59, date(2026, 9, 15), today()).unwrap();
        assert!(g.delta < 0.0 && g.delta > -1.0, "{}", g.delta);
    }

    #[test]
    fn expired_option_is_invalid_input() {
        for expiry in [date(2026, 9, 16), date(2026, 9, 1)] {
            let result = compute(&series(OptionType::Call, 49.91, expiry), trade(1.0), "PETR4", 49.06, 13.0, today(), today());
            assert!(matches!(result, Err(AppError::InvalidInput(_))), "{expiry}");
        }
    }

    #[test]
    fn price_below_intrinsic_value_is_invalid_input() {
        // Call 20 reais dentro do dinheiro negociada a R$ 0,50: preço velho.
        let result = compute(&series(OptionType::Call, 30.0, date(2026, 12, 21)), trade(0.5), "PETR4", 50.0, 13.0, today(), today());
        assert!(matches!(result, Err(AppError::InvalidInput(m)) if m.contains("stale")));
    }

    #[test]
    fn long_dated_deep_discount_price_is_invalid_input_not_a_panic() {
        // Achado ao vivo: call ATM de 16 meses com juros de 14 % tem piso de ~10,2 no
        // modelo sem dividendos; negociada a 7,53, nenhuma vol a reproduz.
        let expiry = date(2028, 2, 18);
        let result = compute(&series(OptionType::Call, 59.69, expiry), trade(7.53), "PETR4", 59.68, 14.0, today(), today());
        assert!(matches!(result, Err(AppError::InvalidInput(_))), "{result:?}");
    }

    #[test]
    fn unknown_and_never_traded_series_are_no_data() {
        let market = OptionsMarket {
            series: vec![series(OptionType::Call, 49.91, date(2026, 12, 21))],
            last_trades: Default::default(),
        };
        assert!(matches!(resolve(&market, "NOPE"), Err(AppError::NoData(m)) if m.contains("unknown")));
        assert!(matches!(resolve(&market, "PETRL522"), Err(AppError::NoData(m)) if m.contains("no traded price")));
    }

    #[test]
    fn traded_series_resolves() {
        let mut market = OptionsMarket { series: vec![series(OptionType::Call, 49.91, date(2026, 12, 21))], last_trades: Default::default() };
        market.last_trades.insert("PETRL522".into(), trade(3.1));
        let (found, last) = resolve(&market, "PETRL522").unwrap();
        assert_eq!(found.strike, 49.91);
        assert_eq!(last.close_price, 3.1);
    }

    /// Curvas por data: `None` = sem pregão, `Err` = provider quebrado.
    fn lookup(days_back: &[(i64, Result<(), ()>)]) -> impl FnMut(NaiveDate) -> std::future::Ready<Result<Vec<DiVertex>, AppError>> {
        let base = today();
        let table: Vec<(NaiveDate, Result<(), ()>)> = days_back.iter().map(|(b, r)| (base - Days::days(*b), *r)).collect();
        move |date| {
            std::future::ready(match table.iter().find(|(d, _)| *d == date) {
                Some((_, Ok(()))) => Ok(curve()),
                Some((_, Err(()))) => Err(AppError::ProviderUnavailable(format!("broken {date}"))),
                None => Err(AppError::NoData(format!("no trading {date}"))),
            })
        }
    }

    #[tokio::test]
    async fn curve_lookup_skips_days_without_trading() {
        // Sábado e domingo sem pregão, sexta publicada.
        let (date, _) = latest_curve_with(today(), lookup(&[(2, Ok(()))])).await.unwrap();
        assert_eq!(date, today() - Days::days(2));
    }

    #[tokio::test]
    async fn curve_lookup_steps_back_over_a_broken_latest_file() {
        let (date, _) = latest_curve_with(today(), lookup(&[(0, Err(())), (1, Ok(()))])).await.unwrap();
        assert_eq!(date, today() - Days::days(1));
    }

    #[tokio::test]
    async fn curve_lookup_gives_up_after_two_provider_failures_with_the_first_error() {
        let err = latest_curve_with(today(), lookup(&[(0, Err(())), (1, Err(())), (2, Ok(()))])).await.unwrap_err();
        assert!(matches!(&err, AppError::ProviderUnavailable(m) if m == &format!("broken {}", today())), "{err}");
    }

    #[tokio::test]
    async fn curve_lookup_without_any_curve_is_provider_unavailable() {
        let err = latest_curve_with(today(), lookup(&[])).await.unwrap_err();
        assert!(matches!(&err, AppError::ProviderUnavailable(m) if m.contains("no DI curve")), "{err}");
    }

    // Ao vivo (rede; baixa o COTAHIST na primeira vez, ~2 min):
    // `cargo test --lib -- --ignored --nocapture live_option_greeks`.
    #[tokio::test]
    #[ignore]
    async fn live_option_greeks() {
        let today = Utc::now().date_naive();
        let market = b3_options::market().await.unwrap().value.value;
        // A call de PETR mais próxima do dinheiro, com negócio recente e vencimento entre 30 e 120
        // dias (as de vencimento longo caem abaixo do piso do modelo sem dividendos, ver o teste acima).
        let spot = yahoo::quote(&Instrument::b3("PETR4")).await.unwrap().value.price;
        println!("PETR4 spot {spot}");
        let (series, last) = market
            .for_root("PETR")
            .into_iter()
            .filter_map(|(s, last)| last.map(|l| (s, l)))
            .filter(|(s, l)| {
                let days = (s.expiration - today).num_days();
                s.option_type == OptionType::Call && (31..=120).contains(&days) && (today - l.trade_date).num_days() < 7
            })
            .min_by(|a, b| {
                (a.0.strike - spot).abs().total_cmp(&(b.0.strike - spot).abs()).then(a.0.expiration.cmp(&b.0.expiration))
            })
            .expect("alguma call de PETR com negócio recente");
        println!("series {} strike {} expires {} last {:?}", series.series_ticker, series.strike, series.expiration, last);

        let greeks = option_greeks(&series.series_ticker, "PETR4").await.unwrap();
        println!("{greeks:#?}");
        assert!(greeks.implied_volatility_pct > 5.0 && greeks.implied_volatility_pct < 300.0);
        assert!(greeks.delta > 0.0 && greeks.delta < 1.0);
        assert!(greeks.risk_free_rate_pct > 1.0 && greeks.risk_free_rate_pct < 40.0);
    }
}
