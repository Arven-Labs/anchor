// Fase 19 — conversão de valores entre moedas pra camada de exibição. Pura:
// recebe o último fechamento de cada par já coletado (`USDBRL` → 5,10) e
// resolve a taxa pela rota direta, pela inversa ou cruzando por USD. Quem lê
// o banco é `commands::currency::load_rates`.

use std::collections::HashMap;

use chrono::NaiveDate;

const PIVOT: &str = "USD";

pub struct FxRates {
    base: String,
    /// Código do par (`USDBRL`) → último fechamento (unidades da segunda
    /// moeda por uma da primeira).
    pairs: HashMap<String, f64>,
}

impl FxRates {
    pub fn new(base: &str, pairs: HashMap<String, f64>) -> Self {
        Self { base: base.to_uppercase(), pairs }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// Quanto vale 1 unidade de `from` na moeda base; `None` se nenhuma rota
    /// (direta, inversa ou via USD) tem cotação.
    pub fn rate(&self, from: &str) -> Option<f64> {
        let from = from.to_uppercase();
        self.rate_between(&from, &self.base)
    }

    pub fn convert(&self, value: f64, from: &str) -> Option<f64> {
        self.rate(from).map(|rate| value * rate)
    }

    fn rate_between(&self, from: &str, to: &str) -> Option<f64> {
        if from == to {
            return Some(1.0);
        }
        self.leg(from, to).or_else(|| {
            if from == PIVOT || to == PIVOT {
                return None;
            }
            Some(self.leg(from, PIVOT)? * self.leg(PIVOT, to)?)
        })
    }

    // Uma perna: o par direto ou o inverso. Cotação zero ou negativa é dado
    // ruim, não taxa — ignorada.
    fn leg(&self, from: &str, to: &str) -> Option<f64> {
        let usable = |price: &f64| *price > 0.0;
        if let Some(price) = self.pairs.get(&format!("{from}{to}")).filter(|p| usable(p)) {
            return Some(*price);
        }
        self.pairs.get(&format!("{to}{from}")).filter(|p| usable(p)).map(|price| 1.0 / price)
    }
}

/// Série diária de cada par, pra converter um valor na data dele (rentabilidade:
/// o fluxo e o fim de cada mês valem pela taxa da época, não pela mais recente).
/// A taxa é a da data mais próxima, sem limite de distância: o histórico do par
/// pode não cobrir o período inteiro e uma taxa distante erra menos que parar.
pub struct FxHistory {
    pairs: HashMap<String, Vec<(NaiveDate, f64)>>,
}

impl FxHistory {
    pub fn new(pairs: HashMap<String, Vec<(NaiveDate, f64)>>) -> Self {
        let pairs = pairs
            .into_iter()
            .map(|(code, mut series)| {
                series.retain(|(_, price)| *price > 0.0);
                series.sort_by_key(|(date, _)| *date);
                (code, series)
            })
            .collect();
        Self { pairs }
    }

    /// Quanto vale 1 unidade de `from` em `to` na data; `None` se nenhuma rota
    /// (direta, inversa ou via USD) tem série.
    pub fn rate_on(&self, from: &str, to: &str, date: NaiveDate) -> Option<f64> {
        let (from, to) = (from.to_uppercase(), to.to_uppercase());
        if from == to {
            return Some(1.0);
        }
        self.leg(&from, &to, date).or_else(|| {
            if from == PIVOT || to == PIVOT {
                return None;
            }
            Some(self.leg(&from, PIVOT, date)? * self.leg(PIVOT, &to, date)?)
        })
    }

    fn leg(&self, from: &str, to: &str, date: NaiveDate) -> Option<f64> {
        if let Some(price) = self.pairs.get(&format!("{from}{to}")).and_then(|s| nearest(s, date)) {
            return Some(price);
        }
        self.pairs.get(&format!("{to}{from}")).and_then(|s| nearest(s, date)).map(|price| 1.0 / price)
    }
}

// Preço da data mais próxima de `date` numa série ordenada (empate: a anterior).
fn nearest(series: &[(NaiveDate, f64)], date: NaiveDate) -> Option<f64> {
    let after = series.partition_point(|(d, _)| *d < date);
    let before = after.checked_sub(1).map(|i| series[i]);
    let next = series.get(after).copied();
    match (before, next) {
        (Some(b), Some(n)) => Some(if (date - b.0) <= (n.0 - date) { b.1 } else { n.1 }),
        (Some(b), None) => Some(b.1),
        (None, Some(n)) => Some(n.1),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn history(pairs: &[(&str, &[(NaiveDate, f64)])]) -> FxHistory {
        FxHistory::new(pairs.iter().map(|(code, series)| (code.to_string(), series.to_vec())).collect())
    }

    #[test]
    fn history_uses_the_rate_of_the_nearest_date() {
        let h = history(&[("USDBRL", &[(d(2026, 1, 1), 5.0), (d(2026, 2, 1), 6.0)])]);
        assert_eq!(h.rate_on("USD", "BRL", d(2026, 1, 5)), Some(5.0));
        assert_eq!(h.rate_on("USD", "BRL", d(2026, 1, 28)), Some(6.0));
    }

    #[test]
    fn history_outside_the_series_takes_the_nearest_edge() {
        let h = history(&[("USDBRL", &[(d(2026, 1, 1), 5.0), (d(2026, 2, 1), 6.0)])]);
        assert_eq!(h.rate_on("USD", "BRL", d(2020, 1, 1)), Some(5.0));
        assert_eq!(h.rate_on("USD", "BRL", d(2030, 1, 1)), Some(6.0));
    }

    #[test]
    fn history_inverts_and_crosses_through_usd() {
        let h = history(&[
            ("USDBRL", &[(d(2026, 1, 1), 5.0)]),
            ("EURUSD", &[(d(2026, 1, 1), 1.1)]),
        ]);
        assert!((h.rate_on("BRL", "USD", d(2026, 1, 1)).unwrap() - 0.2).abs() < 1e-9);
        assert!((h.rate_on("EUR", "BRL", d(2026, 1, 1)).unwrap() - 5.5).abs() < 1e-9);
    }

    #[test]
    fn history_without_any_route_is_none_and_same_currency_is_one() {
        let h = history(&[]);
        assert_eq!(h.rate_on("USD", "BRL", d(2026, 1, 1)), None);
        assert_eq!(h.rate_on("brl", "BRL", d(2026, 1, 1)), Some(1.0));
    }

    fn rates(base: &str, pairs: &[(&str, f64)]) -> FxRates {
        FxRates::new(base, pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
    }

    #[test]
    fn same_currency_is_identity_without_any_pair() {
        assert_eq!(rates("BRL", &[]).convert(10.0, "brl"), Some(10.0));
    }

    #[test]
    fn direct_pair_multiplies() {
        assert_eq!(rates("BRL", &[("USDBRL", 5.0)]).convert(10.0, "USD"), Some(50.0));
    }

    #[test]
    fn inverse_pair_divides() {
        let r = rates("USD", &[("USDBRL", 5.0)]);
        assert!((r.convert(50.0, "BRL").unwrap() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn cross_goes_through_usd() {
        // EUR→USD 1,1 e USD→BRL 5 = EUR→BRL 5,5.
        let r = rates("BRL", &[("EURUSD", 1.1), ("USDBRL", 5.0)]);
        assert!((r.convert(2.0, "EUR").unwrap() - 11.0).abs() < 1e-9);
    }

    #[test]
    fn missing_route_is_none() {
        assert_eq!(rates("BRL", &[("EURUSD", 1.1)]).convert(1.0, "EUR"), None);
        assert_eq!(rates("BRL", &[]).convert(1.0, "USD"), None);
    }

    #[test]
    fn non_positive_quote_is_not_a_rate() {
        assert_eq!(rates("BRL", &[("USDBRL", 0.0)]).convert(1.0, "USD"), None);
    }
}
