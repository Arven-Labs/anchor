// The 20 Ethereum score indicators (Fase 3 + Fase 31) — keys match the
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
  fear_greed_index: "Fear & Greed Index (0-100)",
  mayer_multiple: "Mayer Multiple (price / 200d average)",
  btc_dominance: "BTC Dominance (% of total market cap)",
} as const;

export type IndicatorKey = keyof typeof INDICATORS;

export const INDICATOR_KEYS = Object.keys(INDICATORS) as IndicatorKey[];

// Bitcoin has its own score (Sessão 112) with the indicators that have a direct
// BTC counterpart: no staking, TVL, net issuance, fees vs emission, ETH/BTC or
// BTC dominance.
// Cardano has its own score too, with 10 indicators: on top of the BTC
// exclusions it has no exchange netflow or exchange supply (CoinMetrics
// Community does not serve them for ADA) and no DVOL or put/call (Deribit does
// not list ADA).
// Any other coin is scored with the Ethereum set, as before. Must match
// `domain::crypto_score::IndicatorGroup::indicators` in the backend.
const ETH_ONLY_KEYS: IndicatorKey[] = [
  "staking_yield",
  "tvl_trend",
  "net_issuance",
  "fees_vs_emission",
  "eth_btc_trend_30d",
  "btc_dominance",
];

const ADA_ONLY_EXCLUDED_KEYS: IndicatorKey[] = [
  "exchange_netflow",
  "exchange_supply_change_30d",
  "dvol",
  "put_call_ratio",
];

/** Coins that have their own collector, score and thresholds. */
export const SCORED_COINS = ["ETH", "BTC", "ADA"];

export function hasOwnScore(coin: string): boolean {
  return SCORED_COINS.includes(coin.toUpperCase());
}

export function indicatorKeysFor(coin: string): IndicatorKey[] {
  const btcKeys = INDICATOR_KEYS.filter((key) => !ETH_ONLY_KEYS.includes(key));
  switch (coin.toUpperCase()) {
    case "BTC":
      return btcKeys;
    case "ADA":
      return btcKeys.filter((key) => !ADA_ONLY_EXCLUDED_KEYS.includes(key));
    default:
      return INDICATOR_KEYS;
  }
}
