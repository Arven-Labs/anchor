use std::collections::BTreeMap;

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::finance_api::client::{self, OptionSeriesPoint};
use crate::finance_core::b3_options::OptionStyle;
use crate::finance_core::black_scholes::OptionType;

// Fase 18, 1ª fatia (Sessão 114) — consulta de opções da B3 na tela da Ação BR:
// a cadeia (séries por vencimento, call e put lado a lado) e as gregas de uma
// série. Só leitura, sem tabela: os dados vêm de `finance_api::client`. A fonte
// só tem o último preço negociado, sem bid/ask nem open interest.

#[derive(Serialize, Debug, PartialEq)]
pub struct OptionLegView {
    pub series_ticker: String,
    /// `None` em série registrada que nunca negociou.
    pub last_price: Option<f64>,
    pub last_trade_date: Option<String>,
    pub american: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct StrikeRowView {
    pub strike: f64,
    pub call: Option<OptionLegView>,
    pub put: Option<OptionLegView>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ExpirationView {
    pub expiration: String,
    pub days_to_expiry: i64,
    pub rows: Vec<StrikeRowView>,
}

#[derive(Serialize, Debug)]
pub struct OptionChainView {
    /// Raiz que a B3 registra (`PETR`), não o ticker pedido.
    pub root: String,
    pub stale: bool,
    pub fetched_at: Option<String>,
    pub expirations: Vec<ExpirationView>,
}

#[derive(Deserialize)]
pub struct OptionChainRequest {
    pub underlying: String,
}

/// Strike em centavos inteiros: `f64` não serve de chave (59,91 pode vir como
/// 59,910000000001) e dois strikes diferem em pelo menos 1 centavo.
fn strike_key(strike: f64) -> i64 {
    (strike * 100.0).round() as i64
}

fn leg(point: &OptionSeriesPoint) -> OptionLegView {
    OptionLegView {
        series_ticker: point.series.series_ticker.clone(),
        last_price: point.last_trade.map(|t| t.close_price),
        last_trade_date: point.last_trade.map(|t| t.trade_date.to_string()),
        american: point.series.style == OptionStyle::American,
    }
}

/// Agrupa as séries por vencimento (crescente, sem os já vencidos) e pareia call
/// e put pelo strike (crescente). Vencimento de hoje ainda conta.
pub fn build_chain(points: &[OptionSeriesPoint], today: NaiveDate) -> Vec<ExpirationView> {
    let mut by_expiration: BTreeMap<NaiveDate, BTreeMap<i64, StrikeRowView>> = BTreeMap::new();
    for point in points.iter().filter(|p| p.series.expiration >= today) {
        let row = by_expiration
            .entry(point.series.expiration)
            .or_default()
            .entry(strike_key(point.series.strike))
            .or_insert_with(|| StrikeRowView { strike: point.series.strike, call: None, put: None });
        match point.series.option_type {
            OptionType::Call => row.call = Some(leg(point)),
            OptionType::Put => row.put = Some(leg(point)),
        }
    }
    by_expiration
        .into_iter()
        .map(|(expiration, rows)| ExpirationView {
            expiration: expiration.to_string(),
            days_to_expiry: (expiration - today).num_days(),
            rows: rows.into_values().collect(),
        })
        .collect()
}

/// A cadeia do ativo-objeto (`PETR4` ou a raiz `PETR`). Raiz sem séries devolve
/// a lista de vencimentos vazia, não erro. A primeira chamada do dia baixa os
/// arquivos da B3 (~80 MB, ~2 min).
#[tauri::command]
pub async fn list_option_chain(request: OptionChainRequest) -> Result<OptionChainView, AppError> {
    let response = client::fetch_option_series(&request.underlying).await?;
    Ok(OptionChainView {
        root: response.underlying_symbol,
        stale: response.meta.stale,
        fetched_at: response.meta.fetched_at.map(|at| at.to_rfc3339()),
        expirations: build_chain(&response.data, Utc::now().date_naive()),
    })
}

#[derive(Deserialize)]
pub struct OptionGreeksRequest {
    pub series_ticker: String,
    /// Ticker do ativo-objeto (`PETR4`), cujo preço é o spot.
    pub underlying: String,
}

#[derive(Serialize, Debug)]
pub struct OptionGreeksView {
    pub series_ticker: String,
    pub underlying_ticker: String,
    /// `call` ou `put`.
    pub option_type: &'static str,
    pub strike: f64,
    pub expiration: String,
    pub spot: f64,
    pub days_to_expiry: i64,
    pub risk_free_rate_pct: f64,
    pub di_curve_date: String,
    pub last_trade_date: String,
    pub last_price: f64,
    pub implied_volatility_pct: f64,
    pub delta: f64,
    pub gamma: f64,
    /// Por dia corrido.
    pub theta: f64,
    /// Por ponto percentual de volatilidade.
    pub vega: f64,
}

/// IV e gregas de uma série, com o preço de `underlying` como spot. Série nunca
/// negociada é `NoData`; opção vencida ou preço abaixo do intrínseco, `InvalidInput`.
#[tauri::command]
pub async fn get_option_greeks(request: OptionGreeksRequest) -> Result<OptionGreeksView, AppError> {
    let greeks = client::fetch_option_greeks(&request.series_ticker, &request.underlying).await?.greeks;
    Ok(OptionGreeksView {
        series_ticker: greeks.series_ticker,
        underlying_ticker: greeks.underlying_ticker,
        option_type: match greeks.option_type {
            OptionType::Call => "call",
            OptionType::Put => "put",
        },
        strike: greeks.strike,
        expiration: greeks.expiration.to_string(),
        spot: greeks.spot,
        days_to_expiry: greeks.days_to_expiry,
        risk_free_rate_pct: greeks.risk_free_rate_pct,
        di_curve_date: greeks.di_curve_date.to_string(),
        last_trade_date: greeks.last_trade_date.to_string(),
        last_price: greeks.last_price,
        implied_volatility_pct: greeks.implied_volatility_pct,
        delta: greeks.delta,
        gamma: greeks.gamma,
        theta: greeks.theta,
        vega: greeks.vega,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finance_core::b3_options::{LastTrade, OptionSeries};

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn point(ticker: &str, option_type: OptionType, strike: f64, expiration: NaiveDate, price: Option<f64>) -> OptionSeriesPoint {
        OptionSeriesPoint {
            series: OptionSeries {
                series_ticker: ticker.to_string(),
                root: "PETR".to_string(),
                option_type,
                strike,
                expiration,
                style: OptionStyle::American,
            },
            last_trade: price.map(|close_price| LastTrade { trade_date: date(2026, 10, 16), close_price }),
        }
    }

    #[test]
    fn call_and_put_of_the_same_strike_share_a_row() {
        let exp = date(2026, 11, 20);
        let chain = build_chain(
            &[
                point("PETRW600", OptionType::Put, 60.0, exp, Some(1.2)),
                point("PETRK600", OptionType::Call, 60.0, exp, Some(2.5)),
            ],
            date(2026, 10, 17),
        );
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].rows.len(), 1);
        let row = &chain[0].rows[0];
        assert_eq!(row.call.as_ref().unwrap().series_ticker, "PETRK600");
        assert_eq!(row.put.as_ref().unwrap().series_ticker, "PETRW600");
        assert_eq!(row.call.as_ref().unwrap().last_price, Some(2.5));
        assert!(row.call.as_ref().unwrap().american);
    }

    #[test]
    fn a_strike_with_only_one_side_keeps_the_other_empty() {
        let chain = build_chain(&[point("PETRK610", OptionType::Call, 61.0, date(2026, 11, 20), Some(1.0))], date(2026, 10, 17));
        let row = &chain[0].rows[0];
        assert!(row.call.is_some() && row.put.is_none());
    }

    #[test]
    fn expired_series_are_dropped_and_today_still_counts() {
        let today = date(2026, 10, 17);
        let chain = build_chain(
            &[
                point("OLD", OptionType::Call, 50.0, date(2026, 10, 16), Some(1.0)),
                point("TODAY", OptionType::Call, 50.0, today, Some(1.0)),
            ],
            today,
        );
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].expiration, "2026-10-17");
        assert_eq!(chain[0].days_to_expiry, 0);
    }

    #[test]
    fn expirations_and_strikes_come_sorted() {
        let (near, far) = (date(2026, 11, 20), date(2026, 12, 18));
        let chain = build_chain(
            &[
                point("C", OptionType::Call, 62.0, far, None),
                point("A", OptionType::Call, 61.0, near, None),
                point("B", OptionType::Call, 59.0, near, None),
            ],
            date(2026, 10, 17),
        );
        assert_eq!(chain.iter().map(|e| e.expiration.as_str()).collect::<Vec<_>>(), ["2026-11-20", "2026-12-18"]);
        assert_eq!(chain[0].rows.iter().map(|r| r.strike).collect::<Vec<_>>(), [59.0, 61.0]);
        assert_eq!(chain[0].days_to_expiry, 34);
    }

    #[test]
    fn a_never_traded_series_has_no_price_or_date() {
        let chain = build_chain(&[point("PETRK600", OptionType::Call, 60.0, date(2026, 11, 20), None)], date(2026, 10, 17));
        let leg = chain[0].rows[0].call.as_ref().unwrap();
        assert_eq!((leg.last_price, leg.last_trade_date.as_deref()), (None, None));
    }

    #[test]
    fn float_noise_in_the_strike_does_not_split_a_row() {
        let exp = date(2026, 11, 20);
        let chain = build_chain(
            &[
                point("PETRK599", OptionType::Call, 59.91, exp, None),
                point("PETRW599", OptionType::Put, 59.910000000001, exp, None),
            ],
            date(2026, 10, 17),
        );
        assert_eq!(chain[0].rows.len(), 1);
    }

    #[test]
    fn no_series_is_an_empty_chain() {
        assert!(build_chain(&[], date(2026, 10, 17)).is_empty());
    }

    // Ao vivo (rede, ~2 min na primeira carga): `cargo test --lib -- --ignored commands::options`.
    #[tokio::test]
    #[ignore]
    async fn live_option_chain_for_petr4() {
        let view = list_option_chain(OptionChainRequest { underlying: "PETR4".to_string() }).await.unwrap();
        assert_eq!(view.root, "PETR");
        assert!(!view.expirations.is_empty());
        assert!(view.expirations.iter().all(|e| !e.rows.is_empty()));
    }
}
