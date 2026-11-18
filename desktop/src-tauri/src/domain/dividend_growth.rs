//! Crescimento histórico dos dividendos por ação, para sugerir o `g` de Gordon e
//! do Teto Projetado. É só uma sugestão: o usuário decide se usa.

use chrono::NaiveDate;

/// Crescimento anual composto dos dividendos, com o período que o sustenta.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct DividendGrowth {
    /// CAGR em % a.a. (negativo quando o dividendo caiu).
    pub cagr_pct: f64,
    pub from_year: i32,
    pub to_year: i32,
}

/// Janela máxima: os últimos 5 anos de variação (6 anos-calendário).
pub const MAX_SPAN_YEARS: i32 = 5;
/// Menos que isso é ruído: não sugere.
pub const MIN_SPAN_YEARS: i32 = 3;

/// CAGR da soma anual de dividendos por ação. `payments` = (data, valor por ação).
///
/// - Só anos-calendário completos: o ano de `today` e qualquer data futura ficam de fora.
/// - O último ano completo (`today.year() - 1`) precisa ter pago algo; se parou de pagar, `None`.
/// - O ano inicial é o mais antigo com pagamento, no máximo `MAX_SPAN_YEARS` antes do final.
/// - Variação com menos de `MIN_SPAN_YEARS` anos, ponta não positiva ou valor não finito: `None`.
pub fn dividend_growth(_payments: &[(NaiveDate, f64)], _today: NaiveDate) -> Option<DividendGrowth> {
    unimplemented!("etapa 2 do plano: implementar depois dos testes")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn today() -> NaiveDate {
        d("2026-03-10")
    }

    /// Um pagamento por ano, em 15/06, com os valores dados a partir de `first_year`.
    fn yearly(first_year: i32, amounts: &[f64]) -> Vec<(NaiveDate, f64)> {
        amounts.iter().enumerate().map(|(i, a)| (d(&format!("{}-06-15", first_year + i as i32)), *a)).collect()
    }

    fn assert_cagr(got: Option<DividendGrowth>, pct: f64, from: i32, to: i32) {
        let g = got.expect("deveria sugerir");
        assert!((g.cagr_pct - pct).abs() < 1e-6, "cagr {} != {pct}", g.cagr_pct);
        assert_eq!((g.from_year, g.to_year), (from, to));
    }

    #[test]
    fn steady_ten_percent_growth_gives_ten_percent() {
        let p = yearly(2020, &[1.0, 1.1, 1.21, 1.331, 1.4641, 1.61051]);
        assert_cagr(dividend_growth(&p, today()), 10.0, 2020, 2025);
    }

    #[test]
    fn current_year_partial_payments_are_ignored() {
        let mut p = yearly(2020, &[1.0, 1.1, 1.21, 1.331, 1.4641, 1.61051]);
        p.push((d("2026-02-01"), 0.01));
        assert_cagr(dividend_growth(&p, today()), 10.0, 2020, 2025);
    }

    #[test]
    fn future_dated_payments_are_ignored() {
        let mut p = yearly(2020, &[1.0, 1.1, 1.21, 1.331, 1.4641, 1.61051]);
        p.push((d("2027-06-15"), 99.0));
        assert_cagr(dividend_growth(&p, today()), 10.0, 2020, 2025);
    }

    #[test]
    fn several_payments_in_a_year_are_summed() {
        // 2022: 1,00 + 1,00 = 2,00; 2025: 4,00 + 4,00 = 8,00 → (8/2)^(1/3) - 1.
        let p = vec![
            (d("2022-03-01"), 1.0),
            (d("2022-09-01"), 1.0),
            (d("2023-06-01"), 3.0),
            (d("2024-06-01"), 5.0),
            (d("2025-03-01"), 4.0),
            (d("2025-09-01"), 4.0),
        ];
        let expected = (4.0f64.powf(1.0 / 3.0) - 1.0) * 100.0;
        assert_cagr(dividend_growth(&p, today()), expected, 2022, 2025);
    }

    #[test]
    fn long_history_uses_only_the_last_five_years_of_change() {
        // 2015..2019 com valores malucos; de 2020 a 2025 cresce 10% ao ano.
        let mut p = yearly(2015, &[50.0, 0.1, 30.0, 0.2, 9.0]);
        p.extend(yearly(2020, &[1.0, 1.1, 1.21, 1.331, 1.4641, 1.61051]));
        assert_cagr(dividend_growth(&p, today()), 10.0, 2020, 2025);
    }

    #[test]
    fn two_years_of_change_is_too_little() {
        let p = yearly(2023, &[1.0, 1.1, 1.21]);
        assert_eq!(dividend_growth(&p, today()), None);
    }

    #[test]
    fn exactly_three_years_of_change_is_enough() {
        let p = yearly(2022, &[1.0, 2.0, 4.0, 8.0]);
        assert_cagr(dividend_growth(&p, today()), 100.0, 2022, 2025);
    }

    #[test]
    fn stopped_paying_last_year_gives_nothing() {
        let p = yearly(2019, &[1.0, 1.1, 1.21, 1.331, 1.4641]); // último pagamento em 2023
        assert_eq!(dividend_growth(&p, today()), None);
    }

    #[test]
    fn falling_dividends_give_a_negative_growth() {
        let p = yearly(2021, &[2.0, 1.7, 1.4, 1.2, 1.0]);
        let expected = (0.5f64.powf(1.0 / 4.0) - 1.0) * 100.0;
        assert_cagr(dividend_growth(&p, today()), expected, 2021, 2025);
    }

    #[test]
    fn a_gap_year_in_the_middle_does_not_break_the_endpoints() {
        // 2023 sem pagamento: só 2020 (1,0) e 2025 (2,0) contam para o CAGR.
        let p = vec![(d("2020-06-01"), 1.0), (d("2021-06-01"), 1.2), (d("2022-06-01"), 1.4), (d("2024-06-01"), 1.8), (d("2025-06-01"), 2.0)];
        let expected = (2.0f64.powf(1.0 / 5.0) - 1.0) * 100.0;
        assert_cagr(dividend_growth(&p, today()), expected, 2020, 2025);
    }

    #[test]
    fn no_payments_gives_nothing() {
        assert_eq!(dividend_growth(&[], today()), None);
    }

    #[test]
    fn a_non_finite_or_non_positive_amount_gives_nothing() {
        for bad in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            let at_start = yearly(2020, &[bad, 1.1, 1.21, 1.331, 1.4641, 1.61051]);
            assert_eq!(dividend_growth(&at_start, today()), None, "ano inicial {bad}");
            let at_end = yearly(2020, &[1.0, 1.1, 1.21, 1.331, 1.4641, bad]);
            assert_eq!(dividend_growth(&at_end, today()), None, "ano final {bad}");
        }
    }
}
