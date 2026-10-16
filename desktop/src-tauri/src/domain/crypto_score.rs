use serde::Serialize;

use crate::error::AppError;

pub struct Threshold {
    pub green_boundary: f64,
    pub red_boundary: f64,
}

pub enum Signal {
    Green,
    Neutral,
    Red,
}

impl Signal {
    pub fn as_str(&self) -> &'static str {
        match self {
            Signal::Green => "GREEN",
            Signal::Neutral => "NEUTRAL",
            Signal::Red => "RED",
        }
    }
}

// Direction is inferred from the boundaries themselves instead of a stored
// flag: green_boundary > red_boundary means "higher is better" (ex.: Staking
// Yield, green at 2%, red at 0%); otherwise "lower is better" (ex.: MVRV
// Z-Score, green below 0, red above 7). Values strictly between the two
// boundaries are Neutral — the spec gives two cutoffs per indicator but no
// rule for the gap between them.
pub fn classify(raw_value: f64, threshold: &Threshold) -> Result<Signal, AppError> {
    if threshold.green_boundary == threshold.red_boundary {
        return Err(AppError::EqualThresholds);
    }

    let higher_is_better = threshold.green_boundary > threshold.red_boundary;

    let signal = if higher_is_better {
        if raw_value >= threshold.green_boundary {
            Signal::Green
        } else if raw_value <= threshold.red_boundary {
            Signal::Red
        } else {
            Signal::Neutral
        }
    } else if raw_value <= threshold.green_boundary {
        Signal::Green
    } else if raw_value >= threshold.red_boundary {
        Signal::Red
    } else {
        Signal::Neutral
    };

    Ok(signal)
}

// ---------------------------------------------------------------------------
// Score por grupo (Fase 31). Antes a contagem "verdes / 9" só existia no
// frontend; agora mora aqui, com teste, e cada grupo ganha o próprio subscore
// pra 12 indicadores não diluírem o sinal.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndicatorGroup {
    OnChain,
    Fundamentals,
    Derivatives,
}

impl IndicatorGroup {
    pub const ALL: [IndicatorGroup; 3] =
        [IndicatorGroup::OnChain, IndicatorGroup::Fundamentals, IndicatorGroup::Derivatives];

    pub fn label(&self) -> &'static str {
        match self {
            IndicatorGroup::OnChain => "On-chain",
            IndicatorGroup::Fundamentals => "Network fundamentals",
            IndicatorGroup::Derivatives => "Derivatives",
        }
    }

    /// Chaves de `indicator_thresholds`/`crypto_indicators` de cada grupo.
    pub fn indicators(&self) -> &'static [&'static str] {
        match self {
            IndicatorGroup::OnChain => &[
                "mvrv_z_score",
                "nvt_ratio",
                "puell_multiple",
                "active_addresses_trend",
                "exchange_netflow",
            ],
            IndicatorGroup::Fundamentals => {
                &["net_issuance", "fees_vs_emission", "staking_yield", "tvl_trend"]
            }
            IndicatorGroup::Derivatives => {
                &["funding_rate_7d", "open_interest_change_30d", "long_short_ratio"]
            }
        }
    }
}

/// Última leitura de um indicador (a que vale pro score).
#[derive(Clone, Debug, Serialize)]
pub struct LatestReading {
    pub indicator: String,
    pub raw_value: f64,
    /// `GREEN`, `NEUTRAL` ou `RED`.
    pub signal: String,
    pub reading_date: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct GroupIndicator {
    pub indicator: &'static str,
    /// `None` = nunca coletado/registrado.
    pub reading: Option<LatestReading>,
}

#[derive(Debug, Serialize)]
pub struct GroupScore {
    pub group: IndicatorGroup,
    pub label: &'static str,
    pub green: usize,
    pub logged: usize,
    pub total: usize,
    pub indicators: Vec<GroupIndicator>,
}

/// Leitura sugerida do score total (faixas equivalentes às originais de
/// 9 indicadores: 7-9 verdes, 4-6, 0-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreVerdict {
    Intact,
    Neutral,
    ReduceRisk,
}

#[derive(Debug, Serialize)]
pub struct CryptoScore {
    pub groups: Vec<GroupScore>,
    pub green: usize,
    pub logged: usize,
    pub total: usize,
    pub verdict: ScoreVerdict,
}

const VERDICT_INTACT_MIN_PCT: f64 = 70.0;
const VERDICT_NEUTRAL_MIN_PCT: f64 = 40.0;

fn verdict_for(green: usize, total: usize) -> ScoreVerdict {
    if total == 0 {
        return ScoreVerdict::ReduceRisk;
    }
    let pct = green as f64 / total as f64 * 100.0;
    if pct >= VERDICT_INTACT_MIN_PCT {
        ScoreVerdict::Intact
    } else if pct >= VERDICT_NEUTRAL_MIN_PCT {
        ScoreVerdict::Neutral
    } else {
        ScoreVerdict::ReduceRisk
    }
}

/// Monta o score a partir da última leitura de cada indicador. Indicador
/// sem leitura conta no `total` mas não como verde, e leitura de indicador
/// fora dos grupos é ignorada.
pub fn compute_score(latest: &[LatestReading]) -> CryptoScore {
    let mut groups = Vec::with_capacity(IndicatorGroup::ALL.len());
    let (mut green, mut logged, mut total) = (0, 0, 0);

    for group in IndicatorGroup::ALL {
        let indicators: Vec<GroupIndicator> = group
            .indicators()
            .iter()
            .map(|name| GroupIndicator {
                indicator: name,
                reading: latest.iter().find(|r| r.indicator == *name).cloned(),
            })
            .collect();
        let group_green = indicators
            .iter()
            .filter(|i| i.reading.as_ref().is_some_and(|r| r.signal == "GREEN"))
            .count();
        let group_logged = indicators.iter().filter(|i| i.reading.is_some()).count();

        green += group_green;
        logged += group_logged;
        total += indicators.len();
        groups.push(GroupScore {
            group,
            label: group.label(),
            green: group_green,
            logged: group_logged,
            total: indicators.len(),
            indicators,
        });
    }

    CryptoScore { groups, green, logged, total, verdict: verdict_for(green, total) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(indicator: &str, signal: &str) -> LatestReading {
        LatestReading {
            indicator: indicator.to_string(),
            raw_value: 1.0,
            signal: signal.to_string(),
            reading_date: "2026-10-16".to_string(),
            source: "test".to_string(),
        }
    }

    #[test]
    fn groups_cover_twelve_distinct_indicators() {
        let all: Vec<&str> =
            IndicatorGroup::ALL.iter().flat_map(|g| g.indicators().iter().copied()).collect();
        assert_eq!(all.len(), 12);
        let mut unique = all.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 12);
    }

    #[test]
    fn empty_score_has_full_total_and_nothing_logged() {
        let score = compute_score(&[]);
        assert_eq!((score.green, score.logged, score.total), (0, 0, 12));
        assert_eq!(score.verdict, ScoreVerdict::ReduceRisk);
        assert!(score.groups.iter().all(|g| g.indicators.iter().all(|i| i.reading.is_none())));
    }

    #[test]
    fn subscores_count_greens_per_group() {
        let score = compute_score(&[
            reading("mvrv_z_score", "GREEN"),
            reading("nvt_ratio", "RED"),
            reading("funding_rate_7d", "GREEN"),
            reading("long_short_ratio", "NEUTRAL"),
        ]);
        let on_chain = &score.groups[0];
        assert_eq!((on_chain.green, on_chain.logged, on_chain.total), (1, 2, 5));
        let fundamentals = &score.groups[1];
        assert_eq!((fundamentals.green, fundamentals.logged, fundamentals.total), (0, 0, 4));
        let derivatives = &score.groups[2];
        assert_eq!((derivatives.green, derivatives.logged, derivatives.total), (1, 2, 3));
        assert_eq!((score.green, score.logged), (2, 4));
    }

    #[test]
    fn unknown_indicators_are_ignored() {
        let score = compute_score(&[reading("made_up", "GREEN")]);
        assert_eq!((score.green, score.logged), (0, 0));
    }

    #[test]
    fn verdict_bands_match_the_original_nine_indicator_ranges() {
        // Com 12 indicadores: 9/12 = 75% → tese intacta; 8/12 = 67% e
        // 5/12 = 42% → neutro; 4/12 = 33% → reduzir risco.
        assert_eq!(verdict_for(9, 12), ScoreVerdict::Intact);
        assert_eq!(verdict_for(8, 12), ScoreVerdict::Neutral);
        assert_eq!(verdict_for(5, 12), ScoreVerdict::Neutral);
        assert_eq!(verdict_for(4, 12), ScoreVerdict::ReduceRisk);
        assert_eq!(verdict_for(0, 0), ScoreVerdict::ReduceRisk);
    }

    // Os 3 seeds de derivativos (migration da Fase 31): direção e sinais.
    #[test]
    fn derivatives_seeds_are_lower_is_better() {
        let funding = Threshold { green_boundary: 5.0, red_boundary: 20.0 };
        assert_eq!(classify(1.6, &funding).unwrap().as_str(), "GREEN");
        assert_eq!(classify(12.0, &funding).unwrap().as_str(), "NEUTRAL");
        assert_eq!(classify(35.0, &funding).unwrap().as_str(), "RED");

        let long_short = Threshold { green_boundary: 2.0, red_boundary: 3.5 };
        assert_eq!(classify(2.65, &long_short).unwrap().as_str(), "NEUTRAL");
        assert_eq!(classify(3.8, &long_short).unwrap().as_str(), "RED");
    }

    // Lower is better (ex.: MVRV Z-Score — green_boundary 0, red_boundary 7).
    #[test]
    fn lower_is_better_green_below_green_boundary() {
        let threshold = Threshold {
            green_boundary: 0.0,
            red_boundary: 7.0,
        };
        assert_eq!(classify(-1.0, &threshold).unwrap().as_str(), "GREEN");
    }

    #[test]
    fn lower_is_better_red_above_red_boundary() {
        let threshold = Threshold {
            green_boundary: 0.0,
            red_boundary: 7.0,
        };
        assert_eq!(classify(8.0, &threshold).unwrap().as_str(), "RED");
    }

    #[test]
    fn lower_is_better_neutral_in_the_gap() {
        let threshold = Threshold {
            green_boundary: 0.0,
            red_boundary: 7.0,
        };
        assert_eq!(classify(3.0, &threshold).unwrap().as_str(), "NEUTRAL");
    }

    // Higher is better (ex.: Staking Yield — green_boundary 2, red_boundary 0).
    #[test]
    fn higher_is_better_green_above_green_boundary() {
        let threshold = Threshold {
            green_boundary: 2.0,
            red_boundary: 0.0,
        };
        assert_eq!(classify(3.0, &threshold).unwrap().as_str(), "GREEN");
    }

    #[test]
    fn higher_is_better_red_below_red_boundary() {
        let threshold = Threshold {
            green_boundary: 2.0,
            red_boundary: 0.0,
        };
        assert_eq!(classify(-0.5, &threshold).unwrap().as_str(), "RED");
    }

    #[test]
    fn higher_is_better_neutral_in_the_gap() {
        let threshold = Threshold {
            green_boundary: 2.0,
            red_boundary: 0.0,
        };
        assert_eq!(classify(1.0, &threshold).unwrap().as_str(), "NEUTRAL");
    }

    #[test]
    fn rejects_equal_boundaries() {
        let threshold = Threshold {
            green_boundary: 5.0,
            red_boundary: 5.0,
        };
        assert!(matches!(
            classify(5.0, &threshold),
            Err(AppError::EqualThresholds)
        ));
    }
}
