// Fase 17 — espelho de `commands::rates::InterestRatesView`.

export type SelicChange = { date: string; rate_pct: number };

export type SelicView = {
  current_pct: number;
  since: string;
  changes: SelicChange[];
};

export type CurveVertex = { years: number; rate_pct: number };

export type KeyRate = {
  years: number;
  rate_pct: number | null;
  rate_pct_month_ago: number | null;
};

export type DiCurveView = {
  reference_date: string;
  vertices: CurveVertex[];
  month_ago_reference_date: string | null;
  month_ago_vertices: CurveVertex[];
  key_rates: KeyRate[];
};

// Selic (BCB) e DI (B3) falham de forma independente — uma fora do ar não
// esconde a outra.
export type InterestRatesView = {
  selic: SelicView | null;
  selic_error: string | null;
  di: DiCurveView | null;
  di_error: string | null;
};
