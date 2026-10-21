// Série PTAX do Banco Central por moeda, para o IR de exterior (Lei 14.754/2023, art. 15). Pura: quem grava e busca é
// `finance_api::ptax_store`. A PTAX só existe em dia útil, então um fato de fim de semana ou feriado usa a última
// cotação anterior (prática usual; não é regra lida na lei, e por isso a tela manda conferir), até um teto de dias: mais
// longe que isso a série está com buraco e vale mais o câmbio de mercado que uma PTAX velha.

use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDate;

use crate::finance_core::bcb_ptax::PtaxQuote;

/// Maior distância (em dias) entre o fato e a PTAX anterior aceita.
pub const MAX_LOOKBACK_DAYS: i64 = 10;

/// Qual lado da cotação: a conversão geral usa a venda; o imposto pago no exterior, a compra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateKind {
    Sell,
    Buy,
}

/// De onde veio a taxa de uma conversão.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RateSource {
    Ptax,
    Market,
}

#[derive(Default, Debug)]
pub struct PtaxSeries {
    by_currency: HashMap<String, BTreeMap<NaiveDate, PtaxQuote>>,
}

impl PtaxSeries {
    pub fn new(by_currency: HashMap<String, BTreeMap<NaiveDate, PtaxQuote>>) -> Self {
        Self { by_currency }
    }

    /// PTAX do dia ou, sem cotação nele, a última anterior dentro do teto; `None` sem série ou com buraco.
    pub fn rate(&self, currency: &str, date: NaiveDate, kind: RateKind) -> Option<f64> {
        let (day, quote) = self.by_currency.get(currency)?.range(..=date).next_back()?;
        if (date - *day).num_days() > MAX_LOOKBACK_DAYS {
            return None;
        }
        let value = match kind {
            RateKind::Sell => quote.sell,
            RateKind::Buy => quote.buy,
        };
        (value.is_finite() && value > 0.0).then_some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn series() -> PtaxSeries {
        let quotes = BTreeMap::from([
            (d(2025, 1, 2), PtaxQuote { buy: 6.208, sell: 6.2086 }),
            (d(2025, 1, 3), PtaxQuote { buy: 6.1557, sell: 6.1563 }),
        ]);
        PtaxSeries::new(HashMap::from([("USD".to_string(), quotes)]))
    }

    #[test]
    fn the_day_has_its_own_sell_and_buy_quotes() {
        let s = series();
        assert_eq!(s.rate("USD", d(2025, 1, 2), RateKind::Sell), Some(6.2086));
        assert_eq!(s.rate("USD", d(2025, 1, 2), RateKind::Buy), Some(6.208));
    }

    #[test]
    fn a_weekend_uses_the_last_quote_before_it() {
        // 04/01/2025 é sábado: vale a sexta, 03/01
        assert_eq!(series().rate("USD", d(2025, 1, 5), RateKind::Sell), Some(6.1563));
    }

    #[test]
    fn a_fact_before_the_first_quote_has_no_ptax() {
        assert_eq!(series().rate("USD", d(2025, 1, 1), RateKind::Sell), None);
    }

    #[test]
    fn a_gap_longer_than_the_lookback_has_no_ptax() {
        let s = series();
        assert_eq!(s.rate("USD", d(2025, 1, 13), RateKind::Sell), Some(6.1563), "10 dias ainda vale");
        assert_eq!(s.rate("USD", d(2025, 1, 14), RateKind::Sell), None);
    }

    #[test]
    fn a_currency_without_a_series_has_no_ptax() {
        assert_eq!(series().rate("EUR", d(2025, 1, 2), RateKind::Sell), None);
        assert_eq!(PtaxSeries::default().rate("USD", d(2025, 1, 2), RateKind::Sell), None);
    }
}
