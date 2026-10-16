// The 13 Ethereum score indicators (Fase 3 + Fase 31) — keys match the
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
  open_interest_change_30d: "Open Interest Change, 30d (% in ETH)",
  long_short_ratio: "Long/Short Account Ratio, 7d avg",
} as const;

export type IndicatorKey = keyof typeof INDICATORS;

export const INDICATOR_KEYS = Object.keys(INDICATORS) as IndicatorKey[];
