// Fase 17 — SELIC e DI Futuro. Puro: conversões da Meta Selic diária (BCB
// 432, vem do BCB em dias corridos) em retorno mensal e em "decisões do
// Copom", e leitura de prazos-chave na curva DI×PRE da B3.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};

use crate::domain::tax::calendar::is_business_day;

const BUSINESS_DAYS_PER_YEAR: f64 = 252.0;

/// Retorno mensal (em %, mesma unidade do CDI em `macro_index_monthly`) de
/// quem rendesse a Meta Selic: capitaliza `(1 + meta/100)^(1/252)` em cada
/// dia útil, usando a meta vigente naquele dia. `points` = (data, meta % a.a.)
/// em qualquer ordem; vai até `until` (mês corrente fica parcial).
pub fn selic_monthly_returns(points: &[(NaiveDate, f64)], until: NaiveDate) -> Vec<(String, f64)> {
    let mut sorted: Vec<(NaiveDate, f64)> = points.to_vec();
    sorted.sort_by_key(|(d, _)| *d);
    let Some(&(first, _)) = sorted.first() else {
        return Vec::new();
    };

    let mut factors: BTreeMap<String, f64> = BTreeMap::new();
    let mut idx = 0;
    let mut day = first;
    while day <= until {
        while idx + 1 < sorted.len() && sorted[idx + 1].0 <= day {
            idx += 1;
        }
        let key = format!("{:04}-{:02}", day.year(), day.month());
        let factor = factors.entry(key).or_insert(1.0);
        if is_business_day(day) {
            *factor *= (1.0 + sorted[idx].1 / 100.0).powf(1.0 / BUSINESS_DAYS_PER_YEAR);
        }
        day += Duration::days(1);
    }

    factors.into_iter().map(|(ym, f)| (ym, (f - 1.0) * 100.0)).collect()
}

/// Comprime a série diária nos dias em que a meta mudou (decisões do
/// Copom). O primeiro ponto sempre entra.
pub fn selic_changes(points: &[(NaiveDate, f64)]) -> Vec<(NaiveDate, f64)> {
    let mut sorted: Vec<(NaiveDate, f64)> = points.to_vec();
    sorted.sort_by_key(|(d, _)| *d);
    let mut changes: Vec<(NaiveDate, f64)> = Vec::new();
    for (date, value) in sorted {
        if changes.last().is_none_or(|(_, last)| (last - value).abs() > 1e-9) {
            changes.push((date, value));
        }
    }
    changes
}

/// Taxa da curva DI no prazo de `years` anos, interpolada linearmente por
/// dias úteis (anos × 252) entre os dois vértices vizinhos. `vertices` =
/// (dias úteis, taxa % a.a.). Fora do intervalo da curva → `None` (não
/// extrapola).
pub fn rate_at_years(vertices: &[(i32, f64)], years: f64) -> Option<f64> {
    let mut sorted: Vec<(i32, f64)> = vertices.to_vec();
    sorted.sort_by_key(|(du, _)| *du);
    let target = years * BUSINESS_DAYS_PER_YEAR;
    let (first, last) = (sorted.first()?, sorted.last()?);
    if target < first.0 as f64 || target > last.0 as f64 {
        return None;
    }
    for pair in sorted.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if target >= a.0 as f64 && target <= b.0 as f64 {
            if b.0 == a.0 {
                return Some(a.1);
            }
            let t = (target - a.0 as f64) / (b.0 - a.0) as f64;
            return Some(a.1 + t * (b.1 - a.1));
        }
    }
    // Curva de um vértice só, exatamente no prazo pedido.
    Some(first.1)
}

/// De onde veio a taxa livre de risco sugerida aos valuations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskFreeSource {
    /// DI futuro de 10 anos (juro nominal de longo prazo).
    Di10y,
    /// Meta Selic vigente (reserva: curtíssimo prazo).
    SelicMeta,
}

/// Taxa livre de risco sugerida, em % a.a., com a fonte e a data a que se refere.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct RiskFreeSuggestion {
    pub rate_pct: f64,
    pub source: RiskFreeSource,
    pub as_of: NaiveDate,
}

/// Escolhe a taxa livre de risco: DI de 10 anos se houver; senão a meta Selic;
/// senão nada. Cada entrada é (data de referência, taxa % a.a.). Taxa que não
/// é finita ou é ≤ 0 não é usada (cai para a próxima fonte).
pub fn risk_free_suggestion(
    di_10y: Option<(NaiveDate, f64)>,
    selic: Option<(NaiveDate, f64)>,
) -> Option<RiskFreeSuggestion> {
    let usable = |point: Option<(NaiveDate, f64)>| point.filter(|&(_, rate)| rate.is_finite() && rate >= 0.0);
    usable(di_10y)
        .map(|(as_of, rate_pct)| RiskFreeSuggestion { rate_pct, source: RiskFreeSource::Di10y, as_of })
        .or_else(|| {
            usable(selic).map(|(as_of, rate_pct)| RiskFreeSuggestion {
                rate_pct,
                source: RiskFreeSource::SelicMeta,
                as_of,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn risk_free_prefers_the_ten_year_di_over_the_selic() {
        let got = risk_free_suggestion(Some((d("2026-11-14"), 11.8)), Some((d("2026-06-18"), 10.5))).unwrap();
        assert_eq!(got.source, RiskFreeSource::Di10y);
        assert_eq!(got.rate_pct, 11.8);
        assert_eq!(got.as_of, d("2026-11-14"));
    }

    #[test]
    fn risk_free_falls_back_to_the_selic_when_there_is_no_ten_year_di() {
        let got = risk_free_suggestion(None, Some((d("2026-06-18"), 10.5))).unwrap();
        assert_eq!(got.source, RiskFreeSource::SelicMeta);
        assert_eq!(got.rate_pct, 10.5);
        assert_eq!(got.as_of, d("2026-06-18"));
    }

    #[test]
    fn risk_free_is_none_without_any_rate() {
        assert_eq!(risk_free_suggestion(None, None), None);
    }

    #[test]
    fn risk_free_skips_an_unusable_di_and_uses_the_selic() {
        for bad in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            let got = risk_free_suggestion(Some((d("2026-11-14"), bad)), Some((d("2026-06-18"), 10.5))).unwrap();
            assert_eq!(got.source, RiskFreeSource::SelicMeta, "DI {bad}");
        }
    }

    #[test]
    fn risk_free_is_none_when_every_rate_is_unusable() {
        assert_eq!(risk_free_suggestion(Some((d("2026-11-14"), f64::NAN)), Some((d("2026-06-18"), 0.0))), None);
    }

    fn month<'a>(returns: &'a [(String, f64)], ym: &str) -> f64 {
        returns.iter().find(|(m, _)| m == ym).unwrap().1
    }

    #[test]
    fn constant_meta_compounds_over_business_days_only() {
        // Junho/2026: 22 dias de semana, menos Corpus Christi (04/06) = 21.
        let returns = selic_monthly_returns(&[(d("2026-05-01"), 15.0)], d("2026-06-30"));
        let expected = (1.15f64.powf(21.0 / 252.0) - 1.0) * 100.0;

        assert!((month(&returns, "2026-06") - expected).abs() < 1e-9);
    }

    #[test]
    fn meta_change_mid_month_uses_each_rate_on_its_days() {
        // Julho/2026: 11 dias úteis até 15/07 a 15%, 12 a partir de 16/07 a 14%.
        let points = [(d("2026-06-01"), 15.0), (d("2026-07-16"), 14.0)];
        let returns = selic_monthly_returns(&points, d("2026-07-31"));
        let expected = (1.15f64.powf(11.0 / 252.0) * 1.14f64.powf(12.0 / 252.0) - 1.0) * 100.0;

        assert!((month(&returns, "2026-07") - expected).abs() < 1e-9);
    }

    #[test]
    fn current_month_is_partial() {
        // Até 05/06/2026 (sexta): 01, 02, 03, 05 — 04 é Corpus Christi.
        let returns = selic_monthly_returns(&[(d("2026-05-01"), 15.0)], d("2026-06-05"));
        let expected = (1.15f64.powf(4.0 / 252.0) - 1.0) * 100.0;

        assert!((month(&returns, "2026-06") - expected).abs() < 1e-9);
    }

    #[test]
    fn changes_keep_only_decision_days() {
        let points = [
            (d("2026-01-02"), 15.0),
            (d("2026-01-03"), 15.0),
            (d("2026-03-19"), 14.75),
            (d("2026-03-20"), 14.75),
            (d("2026-05-07"), 14.5),
        ];

        assert_eq!(
            selic_changes(&points),
            vec![(d("2026-01-02"), 15.0), (d("2026-03-19"), 14.75), (d("2026-05-07"), 14.5)]
        );
    }

    #[test]
    fn interpolates_between_vertices() {
        let curve = [(1, 13.9), (252, 14.2), (504, 13.8)];

        assert_eq!(rate_at_years(&curve, 1.0), Some(14.2));
        // 1,5 ano = 378 dias úteis, no meio de 252 e 504.
        assert!((rate_at_years(&curve, 1.5).unwrap() - 14.0).abs() < 1e-9);
    }

    #[test]
    fn does_not_extrapolate() {
        let curve = [(1, 13.9), (252, 14.2)];

        assert_eq!(rate_at_years(&curve, 2.0), None);
        assert_eq!(rate_at_years(&[], 1.0), None);
    }
}
