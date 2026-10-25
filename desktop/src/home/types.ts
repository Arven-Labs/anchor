// Home do Workspace, fatia 1 (layout fixo) — espelha
// `commands/workspace_summary.rs`. `ClassAllocation`/`MonthlyReturn` são os
// mesmos tipos já usados por `portfolio/SummarySection.tsx`/`ProfitabilitySection.tsx`,
// só que agregados no Workspace inteiro em vez de 1 portfolio só.
import type { ClassAllocation, MonthlyReturn } from "../portfolio/types";

export type PortfolioBreakdown = {
  portfolio_id: number;
  name: string;
  market_value: number;
};

export type WorkspaceSummary = {
  total_market_value: number;
  total_cost_basis: number;
  unrealized_pl: number;
  unrealized_pl_pct: number | null;
  dividends_received_12m: number;
  allocation_by_class: ClassAllocation[];
  positions_missing_price: number;
  base_currency: string;
  positions_missing_fx: number;
  portfolios: PortfolioBreakdown[];
};

export type { MonthlyReturn };

// Fase 10.8 fatia 2 (personalização de layout) — espelha
// `commands/home_layout.rs::HomeWidgetLayout`/`HomeWidgetLayoutInput`. Um id
// fixo por widget da Home; a lista completa vive em `HomeSection.tsx`
// (`WIDGET_TITLES`/`DEFAULT_LAYOUT`).
export type WidgetKey =
  | "kpi_total_value"
  | "kpi_profit"
  | "kpi_dividends_12m"
  | "kpi_return"
  | "allocation_donut"
  | "portfolios_table"
  | "market_panel"
  | "interest_rates"
  | "losses_to_offset"
  | "net_worth";

export type HomeWidgetLayout = {
  widget_key: WidgetKey;
  position: number;
  w: number;
  h: number;
  visible: boolean;
};
