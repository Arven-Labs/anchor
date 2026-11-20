//! Rascunho de tese a partir dos dados já coletados (frente 3 do ROADMAP, sem IA).
//! Puro: devolve um Markdown com o retrato datado dos números e seções vazias para o dono
//! preencher. O retrato é texto congelado: o número é o que valia quando a tese foi escrita.

use std::collections::BTreeMap;

use chrono::NaiveDate;

/// Indicadores como o screener os calcula (`commands::screener::ScreenerRow`).
#[derive(Debug, Clone, PartialEq)]
pub struct DraftIndicators {
    pub price: f64,
    pub currency: Option<String>,
    pub pe: Option<f64>,
    pub pb: Option<f64>,
    /// Percentual: 15 significa 15%.
    pub roe: f64,
    pub dividend_yield_12m: Option<f64>,
    pub dividend_yield_5y_avg: Option<f64>,
    /// Data e hora da coleta dos fundamentos (`AAAA-MM-DDT...`); só a data aparece.
    pub fetched_at: String,
}

/// Um valuation salvo. `safety_margin` é fração (0,2 = 20%), como em `valuation`.
#[derive(Debug, Clone, PartialEq)]
pub struct DraftValuation {
    pub model: String,
    pub fair_price: Option<f64>,
    pub safety_margin: Option<f64>,
    pub verdict: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DraftInput {
    pub ticker: String,
    pub name: Option<String>,
    pub today: NaiveDate,
    pub indicators: Option<DraftIndicators>,
    pub valuations: Vec<DraftValuation>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Draft {
    pub title: String,
    pub content_markdown: String,
}

/// O valuation mais recente (maior `updated_at`) de cada modelo, em ordem de nome do modelo.
pub fn latest_per_model(valuations: Vec<DraftValuation>) -> Vec<DraftValuation> {
    let _ = valuations;
    let _ = BTreeMap::<String, DraftValuation>::new();
    todo!("latest_per_model")
}

/// Título "`TICKER` thesis" e o Markdown do rascunho. Campo ausente vira `n/d`, nunca zero.
pub fn render_draft(input: &DraftInput) -> Draft {
    let _ = input;
    todo!("render_draft")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
    }

    fn indicators() -> DraftIndicators {
        DraftIndicators {
            price: 20.0,
            currency: Some("BRL".into()),
            pe: Some(8.5),
            pb: Some(1.2),
            roe: 15.0,
            dividend_yield_12m: Some(6.0),
            dividend_yield_5y_avg: Some(5.0),
            fetched_at: "2026-11-01T10:00:00+00:00".into(),
        }
    }

    fn valuation(model: &str, fair: Option<f64>, margin: Option<f64>, verdict: Option<&str>, at: &str) -> DraftValuation {
        DraftValuation {
            model: model.into(),
            fair_price: fair,
            safety_margin: margin,
            verdict: verdict.map(String::from),
            updated_at: at.into(),
        }
    }

    fn input() -> DraftInput {
        DraftInput {
            ticker: "BBAS3".into(),
            name: Some("Banco do Brasil".into()),
            today: today(),
            indicators: Some(indicators()),
            valuations: vec![valuation("bazin", Some(25.0), Some(0.2), Some("CHEAP"), "2026-10-01T09:00:00+00:00")],
        }
    }

    fn md(input: &DraftInput) -> String {
        render_draft(input).content_markdown
    }

    #[test]
    fn the_title_is_the_ticker_and_the_word_thesis() {
        assert_eq!(render_draft(&input()).title, "BBAS3 thesis");
    }

    #[test]
    fn the_heading_has_the_ticker_and_the_name() {
        assert!(md(&input()).starts_with("# BBAS3 — Banco do Brasil\n"));
        let mut no_name = input();
        no_name.name = None;
        assert!(md(&no_name).starts_with("# BBAS3\n"));
    }

    #[test]
    fn the_snapshot_carries_today_and_the_collection_date() {
        let text = md(&input());
        assert!(text.contains("## Snapshot (2026-11-20)"), "{text}");
        assert!(text.contains("Data collected 2026-11-01."), "{text}");
    }

    #[test]
    fn the_indicators_are_listed_with_their_units() {
        let text = md(&input());
        for line in [
            "- Price: 20.00 BRL",
            "- P/E: 8.50",
            "- P/B: 1.20",
            "- ROE: 15.0%",
            "- Dividend yield (12m): 6.0%",
            "- Dividend yield (5y avg): 5.0%",
        ] {
            assert!(text.contains(line), "faltou `{line}` em:\n{text}");
        }
    }

    #[test]
    fn a_missing_indicator_is_n_d_never_zero() {
        let mut i = input();
        let ind = i.indicators.as_mut().unwrap();
        ind.pe = None;
        ind.dividend_yield_12m = None;
        ind.currency = None;
        let text = md(&i);
        assert!(text.contains("- P/E: n/d"), "{text}");
        assert!(text.contains("- Dividend yield (12m): n/d"), "{text}");
        assert!(text.contains("- Price: 20.00\n"), "{text}");
        assert!(!text.contains("- P/E: 0"), "{text}");
    }

    #[test]
    fn a_ticker_without_collected_data_gets_a_notice_and_no_numbers() {
        let mut i = input();
        i.indicators = None;
        let text = md(&i);
        assert!(text.contains("No collected data for BBAS3 yet (n/d)."), "{text}");
        assert!(!text.contains("- Price:"), "{text}");
    }

    #[test]
    fn a_saved_valuation_shows_fair_price_margin_verdict_and_date() {
        let text = md(&input());
        assert!(
            text.contains("- bazin: fair price 25.00, margin 20.0%, CHEAP (updated 2026-10-01)"),
            "{text}"
        );
    }

    #[test]
    fn a_valuation_with_missing_parts_uses_n_d() {
        let mut i = input();
        i.valuations = vec![valuation("dcf", None, None, None, "2026-10-02T09:00:00+00:00")];
        let text = md(&i);
        assert!(text.contains("- dcf: fair price n/d, margin n/d, n/d (updated 2026-10-02)"), "{text}");
    }

    #[test]
    fn no_saved_valuation_says_so() {
        let mut i = input();
        i.valuations = vec![];
        assert!(md(&i).contains("No saved valuation (n/d)."));
    }

    #[test]
    fn the_blank_sections_for_the_owner_come_last_and_in_order() {
        let text = md(&input());
        let positions: Vec<usize> = ["## Why own it", "## Risks", "## What would make me sell", "## Next review"]
            .iter()
            .map(|h| text.find(h).unwrap_or_else(|| panic!("faltou `{h}` em:\n{text}")))
            .collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
        assert!(positions[0] > text.find("## Snapshot").unwrap());
    }

    #[test]
    fn latest_per_model_keeps_the_newest_of_each_model_sorted_by_name() {
        let kept = latest_per_model(vec![
            valuation("graham", Some(10.0), None, None, "2026-09-01T00:00:00+00:00"),
            valuation("bazin", Some(24.0), None, None, "2026-08-01T00:00:00+00:00"),
            valuation("bazin", Some(25.0), None, None, "2026-10-01T00:00:00+00:00"),
        ]);
        let summary: Vec<(&str, Option<f64>)> = kept.iter().map(|v| (v.model.as_str(), v.fair_price)).collect();
        assert_eq!(summary, [("bazin", Some(25.0)), ("graham", Some(10.0))]);
    }
}
