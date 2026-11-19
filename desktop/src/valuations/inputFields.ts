// Field labels/formatting for each model's saved assumptions — mirrors the
// labels already used in the calculator forms (src/models/*Form.tsx). The
// `id`/`valuation_id` columns that come back from `get_valuation_inputs`
// are intentionally left out here; only fields listed below are rendered.
export type InputFieldFormat = "currency" | "percentage" | "number" | "integer";

export type InputField = {
  key: string;
  label: string;
  format: InputFieldFormat;
};

export const INPUT_FIELDS: Record<string, InputField[]> = {
  bazin: [
    {
      key: "average_dividend",
      label: "Average dividend per share (last 5 years)",
      format: "currency",
    },
    { key: "desired_yield", label: "Desired yield", format: "percentage" },
  ],
  graham: [
    { key: "eps", label: "EPS — earnings per share", format: "currency" },
    {
      key: "book_value_per_share",
      label: "Book value per share",
      format: "currency",
    },
  ],
  gordon: [
    {
      key: "current_dividend",
      label: "Current dividend — D0",
      format: "currency",
    },
    {
      key: "expected_growth",
      label: "Expected dividend growth — g",
      format: "percentage",
    },
    { key: "ke", label: "Required return — Ke", format: "percentage" },
  ],
  dcf: [
    { key: "ebit", label: "EBIT (R$ millions)", format: "number" },
    { key: "tax_rate", label: "Effective tax rate", format: "percentage" },
    {
      key: "depreciation_amortization",
      label: "D&A — depreciation/amortization (R$ millions)",
      format: "number",
    },
    { key: "capex", label: "Capex (R$ millions)", format: "number" },
    {
      key: "nwc_change",
      label: "ΔNWC — change in net working capital (R$ millions)",
      format: "number",
    },
    { key: "total_debt", label: "Total debt (R$ millions)", format: "number" },
    { key: "cash", label: "Cash (R$ millions)", format: "number" },
    {
      key: "shares_outstanding",
      label: "Shares outstanding (millions)",
      format: "number",
    },
    { key: "beta", label: "Beta", format: "number" },
    {
      key: "risk_free_rate",
      label: "Risk-free rate — Rf",
      format: "percentage",
    },
    {
      key: "market_risk_premium",
      label: "Market risk premium",
      format: "percentage",
    },
    { key: "kd", label: "Cost of debt — Kd", format: "percentage" },
    {
      key: "perpetuity_growth",
      label: "Perpetuity growth — g",
      format: "percentage",
    },
  ],
  banks: [
    {
      key: "book_value_per_share",
      label: "Book value per share",
      format: "currency",
    },
    { key: "roe", label: "ROE", format: "percentage" },
    { key: "payout", label: "Payout", format: "percentage" },
    { key: "ke", label: "Required return — Ke", format: "percentage" },
  ],
  rim: [
    {
      key: "book_value_per_share",
      label: "Book value per share",
      format: "currency",
    },
    { key: "roe_current", label: "Current ROE", format: "percentage" },
    { key: "payout", label: "Payout", format: "percentage" },
    { key: "ke", label: "Required return — Ke", format: "percentage" },
    { key: "fade_years", label: "Fade years — N", format: "integer" },
  ],
  rnav: [
    {
      key: "landbank",
      label: "Landbank at market value (R$ millions)",
      format: "number",
    },
    {
      key: "inventory_at_market_value",
      label: "Inventory at market value (R$ millions)",
      format: "number",
    },
    {
      key: "net_cash",
      label: "Net cash — cash minus debt (R$ millions)",
      format: "number",
    },
    {
      key: "shares_outstanding",
      label: "Shares outstanding (millions)",
      format: "number",
    },
  ],
  projected_ceiling: [
    {
      key: "current_dividend",
      label: "Current dividend — D0",
      format: "currency",
    },
    { key: "expected_growth", label: "Expected growth — g", format: "percentage" },
    { key: "projection_years", label: "Projection years — N", format: "integer" },
    { key: "desired_yield", label: "Desired yield", format: "percentage" },
    { key: "ke", label: "Discount rate — Ke", format: "percentage" },
  ],
};

export function formatInputValue(value: unknown, format: InputFieldFormat): string {
  const num = Number(value);
  switch (format) {
    case "currency":
      return `R$ ${num.toFixed(2)}`;
    case "percentage":
      return `${(num * 100).toFixed(1)}%`;
    case "integer":
      return String(Math.round(num));
    case "number":
      return num.toFixed(2);
  }
}

// Round-trip pair for an editable <input> — same convention the calculator
// forms already use (type "6" for 6%, not "0.06"). formatInputValue above is
// display-only (adds "R$"/"%" text), these two keep the value plain so it
// can go back into a number input and back out again.
export function toEditableString(value: unknown, format: InputFieldFormat): string {
  const num = Number(value);
  // 0.14 * 100 = 14.000000000000002 em ponto flutuante; 12 dígitos significativos cortam o ruído
  // (que fica perto do 16º) sem apagar premissas reais como 7.125%. Number() tira zeros à direita.
  if (format === "percentage") return String(Number((num * 100).toPrecision(12)));
  if (format === "integer") return String(Math.round(num));
  return String(num);
}

export function fromEditableString(value: string, format: InputFieldFormat): number {
  const num = Number(value);
  if (format === "percentage") return num / 100;
  if (format === "integer") return Math.round(num);
  return num;
}
