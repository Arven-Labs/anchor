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
// Thresholds editáveis (Fase 31). Os valores de fábrica espelham o que as
// migrations semeiam em `indicator_thresholds` (`m20260709_212958`,
// `m20261016_130000`, `m20261016_140000`, `m20261017_100000` e, os de
// sentimento, `m20261115_100000`; os do BTC em `m20261017_110000`) —
// duplicados aqui só pra "Reset" funcionar sem consultar
// o histórico de migrations. Ao mudar um seed, mude os dois.
//
// Limites são por moeda (`indicator_thresholds.coin`). Só ETH, BTC e ADA têm
// conjunto próprio; qualquer outra moeda (registro manual, ex. SOL) usa o do
// ETH, como era antes de existir o do BTC.
// ---------------------------------------------------------------------------

pub const COIN_ETH: &str = "ETH";
pub const COIN_BTC: &str = "BTC";
pub const COIN_ADA: &str = "ADA";

/// Moeda cujo conjunto de indicadores e limites vale para `coin`.
pub fn threshold_coin(coin: &str) -> &'static str {
    if coin.eq_ignore_ascii_case(COIN_BTC) {
        COIN_BTC
    } else if coin.eq_ignore_ascii_case(COIN_ADA) {
        COIN_ADA
    } else {
        COIN_ETH
    }
}

pub const ETH_DEFAULT_THRESHOLDS: [(&str, f64, f64); 20] = [
    ("mvrv_z_score", 0.0, 7.0),
    ("puell_multiple", 0.5, 4.0),
    ("staking_yield", 2.0, 0.0),
    ("nvt_ratio", 0.9, 1.3),
    ("net_issuance", 0.0, 2.0),
    ("tvl_trend", 0.0, -10.0),
    ("active_addresses_trend", 0.0, -10.0),
    ("exchange_netflow", 0.0, 0.5),
    ("fees_vs_emission", 0.5, 0.1),
    ("funding_rate_7d", 5.0, 20.0),
    ("open_interest_change_30d", 0.0, 25.0),
    ("long_short_ratio", 2.0, 3.5),
    ("dvol", 55.0, 75.0),
    ("put_call_ratio", 0.7, 0.4),
    ("stablecoin_supply_change_30d", 2.0, 0.0),
    ("exchange_supply_change_30d", -1.0, 1.5),
    ("eth_btc_trend_30d", 3.0, -3.0),
    ("fear_greed_index", 25.0, 75.0),
    ("mayer_multiple", 1.3, 2.0),
    ("btc_dominance", 55.0, 65.0),
];

/// Calibrados com dados reais do BTC em 2026-10-17 (ver a migration
/// `m20261017_110000`). Sem staking, TVL, net issuance, fees vs emission nem
/// ETH/BTC: não têm análogo no BTC. Os dois de sentimento (lote 4) vêm da
/// migration `m20261115_100000`.
pub const BTC_DEFAULT_THRESHOLDS: [(&str, f64, f64); 14] = [
    ("mvrv_z_score", 0.0, 7.0),
    ("nvt_ratio", 0.9, 1.3),
    ("puell_multiple", 0.5, 4.0),
    ("active_addresses_trend", 0.0, -10.0),
    ("exchange_netflow", -0.02, 0.03),
    ("exchange_supply_change_30d", -1.0, 2.0),
    ("funding_rate_7d", 5.0, 15.0),
    ("open_interest_change_30d", 0.0, 25.0),
    ("long_short_ratio", 1.3, 2.0),
    ("dvol", 40.0, 55.0),
    ("put_call_ratio", 0.7, 0.4),
    ("stablecoin_supply_change_30d", 2.0, 0.0),
    ("fear_greed_index", 25.0, 75.0),
    ("mayer_multiple", 1.3, 2.0),
];

/// Calibrados com dados reais do ADA em 2026-10-26 (ver a migration
/// `m20261116_100000`). Sem exchange netflow nem exchange supply (o CoinMetrics
/// Community não tem `FlowInExUSD`/`FlowOutExUSD`/`SplyExNtv` para ADA), sem
/// DVOL nem put/call (a Deribit não lista ADA), sem fundamentos de rede nem
/// dominância do BTC (são do ETH).
pub const ADA_DEFAULT_THRESHOLDS: [(&str, f64, f64); 10] = [
    ("mvrv_z_score", 0.0, 3.5),
    ("nvt_ratio", 0.9, 1.3),
    ("puell_multiple", 0.5, 2.5),
    ("active_addresses_trend", 0.0, -20.0),
    ("funding_rate_7d", 5.0, 10.0),
    ("open_interest_change_30d", 0.0, 25.0),
    ("long_short_ratio", 1.8, 2.6),
    ("stablecoin_supply_change_30d", 2.0, 0.0),
    ("fear_greed_index", 25.0, 75.0),
    ("mayer_multiple", 1.3, 2.0),
];

/// Limites de fábrica de uma moeda: `(indicador, verde, vermelho)`.
pub fn default_thresholds(coin: &str) -> &'static [(&'static str, f64, f64)] {
    match threshold_coin(coin) {
        COIN_BTC => &BTC_DEFAULT_THRESHOLDS,
        COIN_ADA => &ADA_DEFAULT_THRESHOLDS,
        _ => &ETH_DEFAULT_THRESHOLDS,
    }
}

/// `(green, red)` de fábrica de um indicador numa moeda.
pub fn default_threshold(coin: &str, indicator: &str) -> Option<(f64, f64)> {
    default_thresholds(coin)
        .iter()
        .find(|(name, _, _)| *name == indicator)
        .map(|(_, green, red)| (*green, *red))
}

/// Direção inferida dos limites (mesma regra de `classify`).
pub fn higher_is_better(green_boundary: f64, red_boundary: f64) -> bool {
    green_boundary > red_boundary
}

/// Limites válidos: números finitos e diferentes (iguais não definem direção).
pub fn validate_boundaries(green_boundary: f64, red_boundary: f64) -> Result<(), AppError> {
    if !green_boundary.is_finite() || !red_boundary.is_finite() {
        return Err(AppError::InvalidGuard("thresholds must be finite numbers".to_string()));
    }
    if green_boundary == red_boundary {
        return Err(AppError::EqualThresholds);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Score por grupo (Fase 31). Antes a contagem "verdes / 9" só existia no
// frontend; agora mora aqui, com teste, e cada grupo ganha o próprio subscore
// pra 15 indicadores não diluírem o sinal.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndicatorGroup {
    OnChain,
    Fundamentals,
    Derivatives,
    Liquidity,
    Sentiment,
}

impl IndicatorGroup {
    pub const ALL: [IndicatorGroup; 5] = [
        IndicatorGroup::OnChain,
        IndicatorGroup::Fundamentals,
        IndicatorGroup::Derivatives,
        IndicatorGroup::Liquidity,
        IndicatorGroup::Sentiment,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            IndicatorGroup::OnChain => "On-chain",
            IndicatorGroup::Fundamentals => "Network fundamentals",
            IndicatorGroup::Derivatives => "Derivatives & options",
            IndicatorGroup::Liquidity => "Market liquidity",
            IndicatorGroup::Sentiment => "Market sentiment",
        }
    }

    /// Grupos que a moeda tem (o BTC não tem fundamentos de rede), na ordem da tela.
    pub fn for_coin(coin: &str) -> Vec<IndicatorGroup> {
        IndicatorGroup::ALL.into_iter().filter(|g| !g.indicators(coin).is_empty()).collect()
    }

    /// Chaves de `indicator_thresholds`/`crypto_indicators` de cada grupo.
    pub fn indicators(&self, coin: &str) -> &'static [&'static str] {
        if threshold_coin(coin) == COIN_ADA {
            return match self {
                IndicatorGroup::OnChain => {
                    &["mvrv_z_score", "nvt_ratio", "puell_multiple", "active_addresses_trend"]
                }
                IndicatorGroup::Fundamentals => &[],
                IndicatorGroup::Derivatives => {
                    &["funding_rate_7d", "open_interest_change_30d", "long_short_ratio"]
                }
                IndicatorGroup::Liquidity => &["stablecoin_supply_change_30d"],
                IndicatorGroup::Sentiment => &["fear_greed_index", "mayer_multiple"],
            };
        }
        if threshold_coin(coin) == COIN_BTC {
            return match self {
                IndicatorGroup::OnChain => &[
                    "mvrv_z_score",
                    "nvt_ratio",
                    "puell_multiple",
                    "active_addresses_trend",
                    "exchange_netflow",
                    "exchange_supply_change_30d",
                ],
                IndicatorGroup::Fundamentals => &[],
                IndicatorGroup::Derivatives => &[
                    "funding_rate_7d",
                    "open_interest_change_30d",
                    "long_short_ratio",
                    "dvol",
                    "put_call_ratio",
                ],
                IndicatorGroup::Liquidity => &["stablecoin_supply_change_30d"],
                IndicatorGroup::Sentiment => &["fear_greed_index", "mayer_multiple"],
            };
        }
        match self {
            IndicatorGroup::OnChain => &[
                "mvrv_z_score",
                "nvt_ratio",
                "puell_multiple",
                "active_addresses_trend",
                "exchange_netflow",
                "exchange_supply_change_30d",
            ],
            IndicatorGroup::Fundamentals => {
                &["net_issuance", "fees_vs_emission", "staking_yield", "tvl_trend"]
            }
            IndicatorGroup::Derivatives => &[
                "funding_rate_7d",
                "open_interest_change_30d",
                "long_short_ratio",
                "dvol",
                "put_call_ratio",
            ],
            IndicatorGroup::Liquidity => &["stablecoin_supply_change_30d", "eth_btc_trend_30d"],
            IndicatorGroup::Sentiment => &["fear_greed_index", "mayer_multiple", "btc_dominance"],
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
pub fn compute_score(coin: &str, latest: &[LatestReading]) -> CryptoScore {
    let mut groups = Vec::with_capacity(IndicatorGroup::ALL.len());
    let (mut green, mut logged, mut total) = (0, 0, 0);

    for group in IndicatorGroup::for_coin(coin) {
        let indicators: Vec<GroupIndicator> = group
            .indicators(coin)
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

    fn grouped(coin: &str) -> Vec<&'static str> {
        IndicatorGroup::ALL.iter().flat_map(|g| g.indicators(coin).iter().copied()).collect()
    }

    #[test]
    fn groups_cover_distinct_indicators_per_coin() {
        for (coin, expected) in [("ETH", 20), ("BTC", 14), ("ADA", 10)] {
            let all = grouped(coin);
            assert_eq!(all.len(), expected, "{coin}");
            let mut unique = all.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), expected, "{coin}");
        }
    }

    #[test]
    fn btc_drops_the_fundamentals_group_and_the_eth_only_indicators() {
        let groups = IndicatorGroup::for_coin("BTC");
        assert_eq!(
            groups,
            [
                IndicatorGroup::OnChain,
                IndicatorGroup::Derivatives,
                IndicatorGroup::Liquidity,
                IndicatorGroup::Sentiment
            ]
        );
        let btc = grouped("BTC");
        for eth_only in [
            "staking_yield",
            "tvl_trend",
            "net_issuance",
            "fees_vs_emission",
            "eth_btc_trend_30d",
            "btc_dominance",
        ] {
            assert!(!btc.contains(&eth_only), "{eth_only}");
        }
        assert_eq!(IndicatorGroup::for_coin("ETH").len(), 5);
    }

    // O ADA é o BTC sem o que o CoinMetrics Community e a Deribit não servem
    // para ele: netflow e supply das exchanges, DVOL e put/call.
    #[test]
    fn ada_is_the_btc_set_minus_what_its_sources_do_not_serve() {
        assert_eq!(
            IndicatorGroup::for_coin("ADA"),
            [
                IndicatorGroup::OnChain,
                IndicatorGroup::Derivatives,
                IndicatorGroup::Liquidity,
                IndicatorGroup::Sentiment
            ]
        );
        let (btc, ada) = (grouped("BTC"), grouped("ADA"));
        assert!(ada.iter().all(|name| btc.contains(name)));
        let dropped: Vec<&str> = btc.iter().copied().filter(|name| !ada.contains(name)).collect();
        assert_eq!(dropped, ["exchange_netflow", "exchange_supply_change_30d", "dvol", "put_call_ratio"]);
        assert_eq!(threshold_coin("ada"), "ADA");
    }

    #[test]
    fn sentiment_group_has_fear_greed_and_mayer_for_both_and_dominance_only_for_eth() {
        assert_eq!(
            IndicatorGroup::Sentiment.indicators("ETH"),
            ["fear_greed_index", "mayer_multiple", "btc_dominance"]
        );
        assert_eq!(IndicatorGroup::Sentiment.indicators("BTC"), ["fear_greed_index", "mayer_multiple"]);
        assert_eq!(IndicatorGroup::Sentiment.label(), "Market sentiment");
    }

    // Os 3 seeds de sentimento (lote 4): menor é melhor.
    #[test]
    fn sentiment_seeds_are_lower_is_better() {
        // Fear & Greed: medo extremo (≤25) é verde, ganância extrema (≥75) é vermelha.
        let fear_greed = Threshold { green_boundary: 25.0, red_boundary: 75.0 };
        assert_eq!(classify(12.0, &fear_greed).unwrap().as_str(), "GREEN");
        assert_eq!(classify(50.0, &fear_greed).unwrap().as_str(), "NEUTRAL");
        assert_eq!(classify(88.0, &fear_greed).unwrap().as_str(), "RED");

        // Mayer: hoje BTC 1,10 e ETH 1,08 → verde; 1,6 → neutro; 2,4 → vermelho.
        let mayer = Threshold { green_boundary: 1.3, red_boundary: 2.0 };
        assert_eq!(classify(1.1, &mayer).unwrap().as_str(), "GREEN");
        assert_eq!(classify(1.6, &mayer).unwrap().as_str(), "NEUTRAL");
        assert_eq!(classify(2.4, &mayer).unwrap().as_str(), "RED");

        // Dominância do BTC (só ETH): hoje 60% → neutro.
        let dominance = Threshold { green_boundary: 55.0, red_boundary: 65.0 };
        assert_eq!(classify(60.0, &dominance).unwrap().as_str(), "NEUTRAL");
        assert_eq!(classify(50.0, &dominance).unwrap().as_str(), "GREEN");
        assert_eq!(classify(70.0, &dominance).unwrap().as_str(), "RED");
    }

    #[test]
    fn other_coins_use_the_eth_set_and_thresholds() {
        assert_eq!(threshold_coin("SOL"), "ETH");
        assert_eq!(threshold_coin("btc"), "BTC");
        assert_eq!(grouped("SOL"), grouped("ETH"));
        assert_eq!(default_threshold("SOL", "dvol"), default_threshold("ETH", "dvol"));
        assert_ne!(default_threshold("BTC", "dvol"), default_threshold("ETH", "dvol"));
    }

    #[test]
    fn empty_score_has_full_total_and_nothing_logged() {
        let score = compute_score("ETH", &[]);
        assert_eq!((score.green, score.logged, score.total), (0, 0, 20));
        assert_eq!(score.verdict, ScoreVerdict::ReduceRisk);
        assert!(score.groups.iter().all(|g| g.indicators.iter().all(|i| i.reading.is_none())));

        let btc = compute_score("BTC", &[]);
        assert_eq!((btc.green, btc.logged, btc.total), (0, 0, 14));
        assert_eq!(btc.groups.len(), 4);

        let ada = compute_score("ADA", &[]);
        assert_eq!((ada.green, ada.logged, ada.total), (0, 0, 10));
        assert_eq!(ada.groups.len(), 4);
    }

    #[test]
    fn subscores_count_greens_per_group() {
        let score = compute_score("ETH", &[
            reading("mvrv_z_score", "GREEN"),
            reading("nvt_ratio", "RED"),
            reading("funding_rate_7d", "GREEN"),
            reading("long_short_ratio", "NEUTRAL"),
        ]);
        let on_chain = &score.groups[0];
        assert_eq!((on_chain.green, on_chain.logged, on_chain.total), (1, 2, 6));
        let fundamentals = &score.groups[1];
        assert_eq!((fundamentals.green, fundamentals.logged, fundamentals.total), (0, 0, 4));
        let derivatives = &score.groups[2];
        assert_eq!((derivatives.green, derivatives.logged, derivatives.total), (1, 2, 5));
        let liquidity = &score.groups[3];
        assert_eq!((liquidity.green, liquidity.logged, liquidity.total), (0, 0, 2));
        let sentiment = &score.groups[4];
        assert_eq!((sentiment.green, sentiment.logged, sentiment.total), (0, 0, 3));
        assert_eq!((score.green, score.logged), (2, 4));
    }

    #[test]
    fn unknown_indicators_are_ignored() {
        let score = compute_score("ETH", &[reading("made_up", "GREEN")]);
        assert_eq!((score.green, score.logged), (0, 0));
    }

    #[test]
    fn verdict_bands_match_the_original_nine_indicator_ranges() {
        // Com 15 indicadores: 11/15 = 73% → tese intacta; 10/15 = 67% e
        // 7/15 = 47% → neutro; 5/15 = 33% → reduzir risco.
        assert_eq!(verdict_for(11, 15), ScoreVerdict::Intact);
        assert_eq!(verdict_for(10, 15), ScoreVerdict::Neutral);
        assert_eq!(verdict_for(7, 15), ScoreVerdict::Neutral);
        assert_eq!(verdict_for(5, 15), ScoreVerdict::ReduceRisk);
        assert_eq!(verdict_for(0, 0), ScoreVerdict::ReduceRisk);
    }

    #[test]
    fn every_grouped_indicator_has_a_valid_default() {
        for coin in ["ETH", "BTC", "ADA"] {
            for name in grouped(coin) {
                let (green, red) = default_threshold(coin, name)
                    .unwrap_or_else(|| panic!("no default threshold for {coin} {name}"));
                assert!(validate_boundaries(green, red).is_ok(), "{coin} {name}");
            }
            assert!(default_threshold(coin, "made_up").is_none());
        }
    }

    #[test]
    fn defaults_have_no_unknown_indicator() {
        for coin in ["ETH", "BTC", "ADA"] {
            let grouped = grouped(coin);
            assert!(default_thresholds(coin).iter().all(|(name, _, _)| grouped.contains(name)), "{coin}");
            assert_eq!(default_thresholds(coin).len(), grouped.len(), "{coin}");
        }
    }

    #[test]
    fn validate_boundaries_rejects_equal_and_non_finite() {
        assert!(validate_boundaries(1.0, 2.0).is_ok());
        assert!(validate_boundaries(2.0, 1.0).is_ok());
        assert!(matches!(validate_boundaries(3.0, 3.0), Err(AppError::EqualThresholds)));
        assert!(validate_boundaries(f64::NAN, 1.0).is_err());
        assert!(validate_boundaries(1.0, f64::INFINITY).is_err());
    }

    #[test]
    fn direction_follows_the_boundaries() {
        assert!(higher_is_better(2.0, 0.0));
        assert!(!higher_is_better(0.0, 7.0));
    }

    // Os 3 seeds de derivativos (migration da Fase 31): direção e sinais.
    #[test]
    fn options_and_liquidity_seeds_have_the_documented_direction() {
        // DVOL: menor é melhor. Hoje 51 → verde; 60 → neutro; 80 → vermelho.
        let dvol = Threshold { green_boundary: 55.0, red_boundary: 75.0 };
        assert_eq!(classify(51.0, &dvol).unwrap().as_str(), "GREEN");
        assert_eq!(classify(60.0, &dvol).unwrap().as_str(), "NEUTRAL");
        assert_eq!(classify(80.0, &dvol).unwrap().as_str(), "RED");

        // Put/call: maior é melhor (medo é oportunidade, calma demais é topo).
        let put_call = Threshold { green_boundary: 0.7, red_boundary: 0.4 };
        assert_eq!(classify(0.516, &put_call).unwrap().as_str(), "NEUTRAL");
        assert_eq!(classify(0.8, &put_call).unwrap().as_str(), "GREEN");
        assert_eq!(classify(0.3, &put_call).unwrap().as_str(), "RED");

        // Stablecoins: maior é melhor (liquidez entrando).
        let stable = Threshold { green_boundary: 2.0, red_boundary: 0.0 };
        assert_eq!(classify(0.77, &stable).unwrap().as_str(), "NEUTRAL");
        assert_eq!(classify(3.0, &stable).unwrap().as_str(), "GREEN");
        assert_eq!(classify(-1.0, &stable).unwrap().as_str(), "RED");
    }

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
