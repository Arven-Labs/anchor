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
    let mut newest: BTreeMap<String, DraftValuation> = BTreeMap::new();
    for v in valuations {
        let replace = newest.get(&v.model).is_none_or(|current| v.updated_at > current.updated_at);
        if replace {
            newest.insert(v.model.clone(), v);
        }
    }
    newest.into_values().collect()
}

const MISSING: &str = "n/d";

fn or_missing(value: Option<f64>, format: impl Fn(f64) -> String) -> String {
    value.map_or_else(|| MISSING.to_string(), format)
}

/// Só a data (`AAAA-MM-DD`) de um carimbo ISO.
fn date_part(stamp: &str) -> &str {
    stamp.get(..10).unwrap_or(stamp)
}

fn indicator_lines(ticker: &str, ind: Option<&DraftIndicators>) -> String {
    let Some(ind) = ind else {
        return format!("No collected data for {ticker} yet ({MISSING}). Run a fetch in Research first.\n");
    };
    let price = match &ind.currency {
        Some(currency) => format!("{:.2} {currency}", ind.price),
        None => format!("{:.2}", ind.price),
    };
    let pct = |v: Option<f64>| or_missing(v, |x| format!("{x:.1}%"));
    format!(
        "Data collected {}. The numbers below are frozen at the snapshot date.\n\n\
         - Price: {price}\n\
         - P/E: {}\n\
         - P/B: {}\n\
         - ROE: {:.1}%\n\
         - Dividend yield (12m): {}\n\
         - Dividend yield (5y avg): {}\n",
        date_part(&ind.fetched_at),
        or_missing(ind.pe, |x| format!("{x:.2}")),
        or_missing(ind.pb, |x| format!("{x:.2}")),
        ind.roe,
        pct(ind.dividend_yield_12m),
        pct(ind.dividend_yield_5y_avg),
    )
}

fn valuation_lines(valuations: &[DraftValuation]) -> String {
    if valuations.is_empty() {
        return format!("No saved valuation ({MISSING}).\n");
    }
    valuations
        .iter()
        .map(|v| {
            format!(
                "- {}: fair price {}, margin {}, {} (updated {})\n",
                v.model,
                or_missing(v.fair_price, |x| format!("{x:.2}")),
                or_missing(v.safety_margin, |x| format!("{:.1}%", x * 100.0)),
                v.verdict.as_deref().unwrap_or(MISSING),
                date_part(&v.updated_at),
            )
        })
        .collect()
}

/// Título "`TICKER` thesis" e o Markdown do rascunho. Campo ausente vira `n/d`, nunca zero.
pub fn render_draft(input: &DraftInput) -> Draft {
    let heading = match &input.name {
        Some(name) => format!("# {} — {name}", input.ticker),
        None => format!("# {}", input.ticker),
    };
    let content_markdown = format!(
        "{heading}\n\n## Snapshot ({})\n\n{}\n### Saved valuations\n\n{}\n\
         ## Why own it\n\n\n## Risks\n\n\n## What would make me sell\n\n\n## Next review\n\n",
        input.today,
        indicator_lines(&input.ticker, input.indicators.as_ref()),
        valuation_lines(&latest_per_model(input.valuations.clone())),
    );
    Draft { title: format!("{} thesis", input.ticker), content_markdown }
}

/// Último mês de `fii_cvm_monthly`. Os percentuais são fração (0,05 = 5%), como no banco.
#[derive(Debug, Clone, PartialEq)]
pub struct FiiDraftMonthly {
    pub reference_date: String,
    pub net_assets: f64,
    pub book_value_per_share: f64,
    pub shareholders: Option<i32>,
    pub dividend_yield_month: Option<f64>,
    pub effective_return_month: Option<f64>,
}

/// Um imóvel de `fii_cvm_properties`, na última data de referência. Percentuais em fração.
#[derive(Debug, Clone, PartialEq)]
pub struct FiiDraftProperty {
    pub name: String,
    pub area_m2: Option<f64>,
    pub vacancy: Option<f64>,
    pub delinquency: Option<f64>,
    pub leased: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FiiDraftInput {
    pub ticker: String,
    pub name: Option<String>,
    pub today: NaiveDate,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub monthly: Option<FiiDraftMonthly>,
    pub properties: Vec<FiiDraftProperty>,
}

/// P/VP = preço ÷ VP da cota. Sem preço, sem mês da CVM ou com VP zerado, não há razão: `None`.
fn price_to_book(price: Option<f64>, monthly: Option<&FiiDraftMonthly>) -> Option<f64> {
    let book = monthly?.book_value_per_share;
    (book > 0.0).then(|| price.map(|p| p / book)).flatten()
}

fn fii_quote_lines(input: &FiiDraftInput) -> String {
    let price = match (input.price, &input.currency) {
        (Some(p), Some(currency)) => format!("{p:.2} {currency}"),
        (Some(p), None) => format!("{p:.2}"),
        (None, _) => MISSING.to_string(),
    };
    let pvp = or_missing(price_to_book(input.price, input.monthly.as_ref()), |x| format!("{x:.2}"));
    let Some(m) = &input.monthly else {
        return format!(
            "No CVM data for {} yet ({MISSING}). Update CVM data in Research first.\n\n- Price: {price}\n- P/VP: {pvp}\n",
            input.ticker
        );
    };
    let pct = |v: Option<f64>| or_missing(v.map(|x| x * 100.0), |x| format!("{x:.1}%"));
    format!(
        "Quote and CVM data below are frozen at the snapshot date. CVM reference {}.\n\n\
         - Price: {price}\n\
         - P/VP: {pvp}\n\
         - Book value per share: {:.2}\n\
         - Net assets: {:.2}\n\
         - Shareholders: {}\n\
         - Dividend yield (month): {}\n\
         - Effective return (month): {}\n",
        date_part(&m.reference_date),
        m.book_value_per_share,
        m.net_assets,
        m.shareholders.map_or_else(|| MISSING.to_string(), |n| n.to_string()),
        pct(m.dividend_yield_month),
        pct(m.effective_return_month),
    )
}

/// Média de uma figura (fração) entre os imóveis que a têm: ponderada por área quando todos esses
/// imóveis têm área, senão simples. A linha diz qual foi usada. Sem nenhum valor: `n/d`.
fn property_average_line(label: &str, properties: &[FiiDraftProperty], figure: impl Fn(&FiiDraftProperty) -> Option<f64>) -> String {
    let with_figure: Vec<(f64, Option<f64>)> = properties.iter().filter_map(|p| figure(p).map(|v| (v, p.area_m2))).collect();
    if with_figure.is_empty() {
        return format!("- {label}: {MISSING}\n");
    }
    let total_area: f64 = with_figure.iter().filter_map(|(_, area)| *area).sum();
    let weighted = with_figure.iter().all(|(_, area)| area.is_some()) && total_area > 0.0;
    let (average, method) = if weighted {
        let sum: f64 = with_figure.iter().map(|(v, area)| v * area.unwrap_or(0.0)).sum();
        (sum / total_area, "area-weighted")
    } else {
        (with_figure.iter().map(|(v, _)| v).sum::<f64>() / with_figure.len() as f64, "simple average")
    };
    format!("- {label} ({method}): {:.1}%\n", average * 100.0)
}

fn property_lines(properties: &[FiiDraftProperty]) -> String {
    if properties.is_empty() {
        return format!("No property data ({MISSING}).\n");
    }
    format!(
        "- Properties: {}\n{}{}{}",
        properties.len(),
        property_average_line("Vacancy", properties, |p| p.vacancy),
        property_average_line("Delinquency", properties, |p| p.delinquency),
        property_average_line("Leased", properties, |p| p.leased),
    )
}

/// Rascunho de tese de FII: cota, P/VP e indicadores CVM. Campo ausente vira `n/d`, nunca zero.
pub fn render_fii_draft(input: &FiiDraftInput) -> Draft {
    let heading = match &input.name {
        Some(name) => format!("# {} — {name}", input.ticker),
        None => format!("# {}", input.ticker),
    };
    let content_markdown = format!(
        "{heading}\n\n## Snapshot ({})\n\n{}\n### Properties\n\n{}\n\
         ## Why own it\n\n\n## Risks\n\n\n## What would make me sell\n\n\n## Next review\n\n",
        input.today,
        fii_quote_lines(input),
        property_lines(&input.properties),
    );
    Draft { title: format!("{} thesis", input.ticker), content_markdown }
}

/// Classes de fundo/recibo com a mesma coleta (cotação, técnicos, dividendo médio). O nome na
/// serialização é o `asset_class` do app, para o frontend mandar a classe como está.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum FundKind {
    #[serde(rename = "etf_br")]
    Etf,
    #[serde(rename = "bdr")]
    Bdr,
}

impl FundKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Etf => "ETF",
            Self::Bdr => "BDR",
        }
    }
}

/// Última linha de `stock_technicals`. SMAs em preço; CAGR em percentual (15 = 15%), como no banco.
#[derive(Debug, Clone, PartialEq)]
pub struct FundDraftTechnicals {
    pub sma_50: Option<f64>,
    pub sma_100: Option<f64>,
    pub sma_200: Option<f64>,
    pub cagr_5y: Option<f64>,
    pub cagr_10y: Option<f64>,
    pub fetched_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FundDraftInput {
    pub kind: FundKind,
    pub ticker: String,
    pub name: Option<String>,
    pub today: NaiveDate,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub technicals: Option<FundDraftTechnicals>,
    pub avg_dividend_5y: Option<f64>,
    /// Classe do ativo subjacente do BDR (`assets.underlying_asset_class`); só aparece se existir.
    pub underlying: Option<String>,
}

/// Rascunho de tese de ETF ou BDR: cotação, médias móveis, CAGR e dividendo médio. Campo ausente
/// vira `n/d`, nunca zero. Sem nenhum dado coletado, só o aviso.
pub fn render_fund_draft(input: &FundDraftInput) -> Draft {
    let heading = match &input.name {
        Some(name) => format!("# {} — {name}", input.ticker),
        None => format!("# {}", input.ticker),
    };
    let content_markdown = format!(
        "{heading}\n\n## Snapshot ({})\n\n{}\n\
         ## Why own it\n\n\n## Risks\n\n\n## What would make me sell\n\n\n## Next review\n\n",
        input.today,
        fund_lines(input),
    );
    Draft { title: format!("{} thesis", input.ticker), content_markdown }
}

fn fund_lines(input: &FundDraftInput) -> String {
    let tech = input.technicals.as_ref();
    if input.price.is_none() && tech.is_none() && input.avg_dividend_5y.is_none() {
        return format!("No collected data for {} yet ({MISSING}). Run a fetch in Research first.\n", input.ticker);
    }
    let price = match (input.price, &input.currency) {
        (Some(p), Some(currency)) => format!("{p:.2} {currency}"),
        (Some(p), None) => format!("{p:.2}"),
        (None, _) => MISSING.to_string(),
    };
    let amount = |pick: fn(&FundDraftTechnicals) -> Option<f64>| or_missing(tech.and_then(pick), |x| format!("{x:.2}"));
    let percent = |pick: fn(&FundDraftTechnicals) -> Option<f64>| or_missing(tech.and_then(pick), |x| format!("{x:.1}%"));
    let underlying = input.underlying.as_ref().map_or_else(String::new, |u| format!("- Underlying: {u}\n"));
    format!(
        "Quote and technicals below are frozen at the snapshot date.\n\n\
         - Class: {}\n\
         {underlying}\
         - Price: {price}\n\
         - SMA 50: {}\n\
         - SMA 100: {}\n\
         - SMA 200: {}\n\
         - CAGR 5y: {}\n\
         - CAGR 10y: {}\n\
         - Avg dividend/share (5y): {}\n\
         - Technicals collected: {}\n",
        input.kind.label(),
        amount(|t| t.sma_50),
        amount(|t| t.sma_100),
        amount(|t| t.sma_200),
        percent(|t| t.cagr_5y),
        percent(|t| t.cagr_10y),
        or_missing(input.avg_dividend_5y, |x| format!("{x:.2}")),
        tech.map_or_else(|| MISSING.to_string(), |t| date_part(&t.fetched_at).to_string()),
    )
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

    fn fii_monthly() -> FiiDraftMonthly {
        FiiDraftMonthly {
            reference_date: "2026-10-31".into(),
            net_assets: 3_000_000_000.0,
            book_value_per_share: 200.0,
            shareholders: Some(150_000),
            dividend_yield_month: Some(0.008),
            effective_return_month: Some(0.012),
        }
    }

    fn fii_property(name: &str, area: Option<f64>, vacancy: Option<f64>, delinquency: Option<f64>, leased: Option<f64>) -> FiiDraftProperty {
        FiiDraftProperty { name: name.into(), area_m2: area, vacancy, delinquency, leased }
    }

    fn fii_input() -> FiiDraftInput {
        FiiDraftInput {
            ticker: "HGLG11".into(),
            name: Some("CSHG Logística".into()),
            today: today(),
            price: Some(160.0),
            currency: Some("BRL".into()),
            monthly: Some(fii_monthly()),
            properties: vec![
                fii_property("Galpão A", Some(100.0), Some(0.10), Some(0.0), Some(0.90)),
                fii_property("Galpão B", Some(300.0), Some(0.02), Some(0.04), Some(0.98)),
            ],
        }
    }

    fn fii_md(input: &FiiDraftInput) -> String {
        render_fii_draft(input).content_markdown
    }

    #[test]
    fn the_fii_title_is_the_ticker_and_the_word_thesis() {
        assert_eq!(render_fii_draft(&fii_input()).title, "HGLG11 thesis");
    }

    #[test]
    fn the_fii_heading_has_the_ticker_and_the_name() {
        assert!(fii_md(&fii_input()).starts_with("# HGLG11 — CSHG Logística\n"));
        let mut no_name = fii_input();
        no_name.name = None;
        assert!(fii_md(&no_name).starts_with("# HGLG11\n"));
    }

    #[test]
    fn the_fii_snapshot_carries_today_and_the_cvm_reference_date() {
        let text = fii_md(&fii_input());
        assert!(text.contains("## Snapshot (2026-11-20)"), "{text}");
        assert!(text.contains("CVM reference 2026-10-31."), "{text}");
    }

    #[test]
    fn the_fii_indicators_are_listed_with_their_units() {
        let text = fii_md(&fii_input());
        for line in [
            "- Price: 160.00 BRL",
            "- P/VP: 0.80",
            "- Book value per share: 200.00",
            "- Net assets: 3000000000.00",
            "- Shareholders: 150000",
            "- Dividend yield (month): 0.8%",
            "- Effective return (month): 1.2%",
        ] {
            assert!(text.contains(line), "faltou `{line}` em:\n{text}");
        }
    }

    #[test]
    fn the_pvp_is_the_price_over_the_book_value_per_share() {
        let mut i = fii_input();
        i.price = Some(250.0);
        assert!(fii_md(&i).contains("- P/VP: 1.25"));
    }

    #[test]
    fn a_missing_fii_number_is_n_d_never_zero() {
        let mut i = fii_input();
        i.price = None;
        let m = i.monthly.as_mut().unwrap();
        m.dividend_yield_month = None;
        m.shareholders = None;
        let text = fii_md(&i);
        assert!(text.contains("- Price: n/d"), "{text}");
        assert!(text.contains("- P/VP: n/d"), "{text}");
        assert!(text.contains("- Dividend yield (month): n/d"), "{text}");
        assert!(text.contains("- Shareholders: n/d"), "{text}");
        assert!(!text.contains("- P/VP: 0"), "{text}");
    }

    #[test]
    fn a_zero_book_value_makes_the_pvp_n_d_instead_of_dividing_by_zero() {
        let mut i = fii_input();
        i.monthly.as_mut().unwrap().book_value_per_share = 0.0;
        let text = fii_md(&i);
        assert!(text.contains("- P/VP: n/d"), "{text}");
        assert!(!text.contains("inf"), "{text}");
    }

    #[test]
    fn a_fii_without_cvm_data_gets_a_notice_and_no_cvm_numbers() {
        let mut i = fii_input();
        i.monthly = None;
        let text = fii_md(&i);
        assert!(text.contains("No CVM data for HGLG11 yet (n/d)."), "{text}");
        assert!(text.contains("- P/VP: n/d"), "{text}");
        assert!(!text.contains("- Net assets:"), "{text}");
    }

    #[test]
    fn the_property_averages_are_weighted_by_area_when_every_area_is_known() {
        let text = fii_md(&fii_input());
        for line in [
            "- Properties: 2",
            "- Vacancy (area-weighted): 4.0%",
            "- Delinquency (area-weighted): 3.0%",
            "- Leased (area-weighted): 96.0%",
        ] {
            assert!(text.contains(line), "faltou `{line}` em:\n{text}");
        }
    }

    #[test]
    fn without_every_area_the_property_average_is_simple_and_says_so() {
        let mut i = fii_input();
        i.properties[1].area_m2 = None;
        let text = fii_md(&i);
        assert!(text.contains("- Vacancy (simple average): 6.0%"), "{text}");
        assert!(!text.contains("area-weighted"), "{text}");
    }

    #[test]
    fn a_property_without_a_figure_is_left_out_of_that_average() {
        let mut i = fii_input();
        i.properties[0].vacancy = None;
        assert!(fii_md(&i).contains("- Vacancy (area-weighted): 2.0%"));
        i.properties[1].vacancy = None;
        assert!(fii_md(&i).contains("- Vacancy: n/d"));
    }

    #[test]
    fn a_fii_without_properties_says_so() {
        let mut i = fii_input();
        i.properties = vec![];
        assert!(fii_md(&i).contains("No property data (n/d)."));
    }

    #[test]
    fn the_fii_draft_has_no_valuations_section_and_ends_with_the_blank_sections_in_order() {
        let text = fii_md(&fii_input());
        assert!(!text.contains("Saved valuations"), "{text}");
        let positions: Vec<usize> = ["## Why own it", "## Risks", "## What would make me sell", "## Next review"]
            .iter()
            .map(|h| text.find(h).unwrap_or_else(|| panic!("faltou `{h}` em:\n{text}")))
            .collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
        assert!(positions[0] > text.find("- Properties:").unwrap());
    }

    fn fund_input() -> FundDraftInput {
        FundDraftInput {
            kind: FundKind::Etf,
            ticker: "BOVA11".into(),
            name: Some("iShares Ibovespa".into()),
            today: today(),
            price: Some(130.0),
            currency: Some("BRL".into()),
            technicals: Some(FundDraftTechnicals {
                sma_50: Some(128.5),
                sma_100: Some(125.0),
                sma_200: Some(120.25),
                cagr_5y: Some(12.5),
                cagr_10y: Some(9.0),
                fetched_at: "2026-11-01T10:00:00+00:00".into(),
            }),
            avg_dividend_5y: Some(1.2),
            underlying: None,
        }
    }

    fn fund_md(input: &FundDraftInput) -> String {
        render_fund_draft(input).content_markdown
    }

    #[test]
    fn the_fund_title_is_the_ticker_and_the_word_thesis() {
        assert_eq!(render_fund_draft(&fund_input()).title, "BOVA11 thesis");
    }

    #[test]
    fn the_fund_heading_has_the_ticker_and_the_name() {
        assert!(fund_md(&fund_input()).starts_with("# BOVA11 — iShares Ibovespa\n"));
        let mut no_name = fund_input();
        no_name.name = None;
        assert!(fund_md(&no_name).starts_with("# BOVA11\n"));
    }

    #[test]
    fn the_fund_snapshot_carries_today_and_the_class() {
        let text = fund_md(&fund_input());
        assert!(text.contains("## Snapshot (2026-11-20)"), "{text}");
        assert!(text.contains("- Class: ETF"), "{text}");
        let mut bdr = fund_input();
        bdr.kind = FundKind::Bdr;
        assert!(fund_md(&bdr).contains("- Class: BDR"));
    }

    #[test]
    fn the_fund_numbers_are_listed_with_their_units() {
        let text = fund_md(&fund_input());
        for line in [
            "- Price: 130.00 BRL",
            "- SMA 50: 128.50",
            "- SMA 100: 125.00",
            "- SMA 200: 120.25",
            "- CAGR 5y: 12.5%",
            "- CAGR 10y: 9.0%",
            "- Avg dividend/share (5y): 1.20",
            "- Technicals collected: 2026-11-01",
        ] {
            assert!(text.contains(line), "faltou `{line}` em:\n{text}");
        }
    }

    #[test]
    fn a_missing_fund_number_is_n_d_never_zero() {
        let mut i = fund_input();
        i.currency = None;
        let t = i.technicals.as_mut().unwrap();
        t.sma_200 = None;
        t.cagr_10y = None;
        i.avg_dividend_5y = None;
        let text = fund_md(&i);
        assert!(text.contains("- Price: 130.00\n"), "{text}");
        assert!(text.contains("- SMA 200: n/d"), "{text}");
        assert!(text.contains("- CAGR 10y: n/d"), "{text}");
        assert!(text.contains("- Avg dividend/share (5y): n/d"), "{text}");
        assert!(!text.contains("- SMA 200: 0"), "{text}");
    }

    #[test]
    fn a_fund_with_a_quote_but_no_technicals_keeps_the_quote_and_marks_the_rest_n_d() {
        let mut i = fund_input();
        i.technicals = None;
        let text = fund_md(&i);
        assert!(text.contains("- Price: 130.00 BRL"), "{text}");
        assert!(text.contains("- SMA 50: n/d"), "{text}");
        assert!(text.contains("- Technicals collected: n/d"), "{text}");
    }

    #[test]
    fn a_fund_without_any_collected_data_gets_a_notice_and_no_numbers() {
        let mut i = fund_input();
        i.price = None;
        i.technicals = None;
        i.avg_dividend_5y = None;
        let text = fund_md(&i);
        assert!(text.contains("No collected data for BOVA11 yet (n/d)."), "{text}");
        assert!(!text.contains("- Price:"), "{text}");
        assert!(text.contains("## Why own it"), "{text}");
    }

    #[test]
    fn the_underlying_class_appears_only_when_known() {
        let mut bdr = fund_input();
        bdr.kind = FundKind::Bdr;
        assert!(!fund_md(&bdr).contains("Underlying"));
        bdr.underlying = Some("acao_internacional".into());
        assert!(fund_md(&bdr).contains("- Underlying: acao_internacional"));
    }

    #[test]
    fn the_fund_draft_has_no_valuations_section_and_ends_with_the_blank_sections_in_order() {
        let text = fund_md(&fund_input());
        assert!(!text.contains("Saved valuations"), "{text}");
        let positions: Vec<usize> = ["## Why own it", "## Risks", "## What would make me sell", "## Next review"]
            .iter()
            .map(|h| text.find(h).unwrap_or_else(|| panic!("faltou `{h}` em:\n{text}")))
            .collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
        assert!(positions[0] > text.find("- Avg dividend/share (5y)").unwrap());
    }
}
