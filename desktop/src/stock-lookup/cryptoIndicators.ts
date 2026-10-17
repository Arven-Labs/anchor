// The 17 Ethereum score indicators (Fase 3 + Fase 31) — keys match the
// `indicator` column in `indicator_thresholds`. The grouping into subscores
// lives in the backend (`domain::crypto_score`) and comes with
// `get_crypto_score`; this list only labels the indicators and drives the
// manual-entry form.
export const INDICATORS = {
  mvrv_z_score: "MVRV Z-Score",
  nvt_ratio: "NVT Ratio (vs 90d moving average)",
  puell_multiple: "Puell Multiple",
  net_issuance: "Net Issuance (issuance - burn)",
  staking_yield: "Staking Yield (real, %)",
  tvl_trend: "TVL Trend (MoM %)",
  active_addresses_trend: "Active Addresses Trend (%)",
  exchange_netflow: "Exchange Netflow",
  fees_vs_emission: "Fees vs Emission",
  funding_rate_7d: "Perp Funding, 7d avg (annualized %)",
  open_interest_change_30d: "Open Interest Change, 30d (% in coin units)",
  long_short_ratio: "Long/Short Account Ratio, 7d avg",
  dvol: "DVOL (implied volatility index)",
  put_call_ratio: "Options Put/Call (open interest)",
  stablecoin_supply_change_30d: "Stablecoin Supply Change, 30d (%)",
  exchange_supply_change_30d: "Exchange Supply Change, 30d (% of coins held)",
  eth_btc_trend_30d: "ETH/BTC Trend, 30d (%)",
} as const;

export type IndicatorKey = keyof typeof INDICATORS;

export const INDICATOR_KEYS = Object.keys(INDICATORS) as IndicatorKey[];

// Bitcoin has its own score (Sessão 112) with the indicators that have a direct
// BTC counterpart: no staking, TVL, net issuance, fees vs emission or ETH/BTC.
// Any other coin is scored with the Ethereum set, as before. Must match
// `domain::crypto_score::IndicatorGroup::indicators` in the backend.
const ETH_ONLY_KEYS: IndicatorKey[] = [
  "staking_yield",
  "tvl_trend",
  "net_issuance",
  "fees_vs_emission",
  "eth_btc_trend_30d",
];

/** Coins that have their own collector, score and thresholds. */
export const SCORED_COINS = ["ETH", "BTC"];

export function hasOwnScore(coin: string): boolean {
  return SCORED_COINS.includes(coin.toUpperCase());
}

export function indicatorKeysFor(coin: string): IndicatorKey[] {
  return coin.toUpperCase() === "BTC"
    ? INDICATOR_KEYS.filter((key) => !ETH_ONLY_KEYS.includes(key))
    : INDICATOR_KEYS;
}
