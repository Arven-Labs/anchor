import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import { Cell, Legend, Pie, PieChart, ResponsiveContainer, Tooltip } from "recharts";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { AppError } from "../types";
import type { MonthlyReturn, PortfolioSummary, PositionView } from "./types";
import { ASSET_CLASS_LABELS, type AssetClass } from "./types";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { formatMoney } from "@/lib/utils";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

// Paleta categórica validada (skill dataviz, references/palette.md, passos
// "dark") contra a superfície fixa do app (--card, #111820 — Anchor não tem
// toggle claro/escuro, ver index.css). Rodado `validate_palette.js --mode
// dark --surface "#111820"`: 8/8 checks OK, pior par adjacente CVD ΔE 8.4
// (banda 6-8, exige rótulo direto — por isso a legenda sempre mostra %).
// Ordem fixa por classe (nunca por rank/posição ordenada), então um filtro
// ou uma carteira que muda de composição não repinta as classes que já
// apareciam antes. Só as 8 classes mais prováveis de aparecer múltiplas
// numa carteira BR ganham cor própria; o resto cai em "Outras" (cinza
// neutro, nunca uma cor categórica reciclada — regra da skill).
const CLASS_COLORS: Partial<Record<AssetClass, string>> = {
  acao_br: "#3987e5",
  fii: "#d95926",
  etf_br: "#199e70",
  cripto: "#c98500",
  bdr: "#d55181",
  metal: "#008300",
  acao_internacional: "#9085e9",
  reit: "#e66767",
};
const OTHER_COLOR = "#9fb1c2"; // --muted-foreground

function formatCurrency(value: number): string {
  return value.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

// `value` já em pontos percentuais (12.34 → "+12.34%") — mesma convenção de
// ProfitabilitySection.tsx::formatPct. Campos que o backend devolve como
// fração (unrealized_pl_pct, % da carteira) são multiplicados por 100 no
// call site antes de chegar aqui.
function formatPct(value: number): string {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

function plColorClass(value: number | null): string {
  if (value == null || value === 0) return "text-muted-foreground";
  return value > 0 ? "text-primary" : "text-destructive";
}

function KpiCard({
  title,
  value,
  valueClassName,
  subtext,
}: {
  title: string;
  value: string;
  valueClassName?: string;
  subtext?: string;
}) {
  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <p className="text-sm text-muted-foreground">{title}</p>
      <p className={`mt-1 text-2xl font-semibold ${valueClassName ?? ""}`}>{value}</p>
      {subtext && <p className="mt-1 text-xs text-muted-foreground">{subtext}</p>}
    </div>
  );
}

// Rebaseia a série encadeada de `get_portfolio_profitability` (Modified
// Dietz, já composto desde o primeiro mês — domain/twr.rs::chain) pra tirar
// um retorno "últimos 12 meses" sem pedir nada novo ao backend: como
// `r_cumulative_pct` é multiplicativo, dividir o índice do mês mais recente
// pelo índice de 12 meses atrás isola exatamente essa janela.
function trailingTwelveMonthPct(monthly: MonthlyReturn[]): number | null {
  if (monthly.length === 0) return null;
  if (monthly.length <= 12) return monthly[monthly.length - 1].r_cumulative_pct;
  const end = monthly[monthly.length - 1];
  const baseline = monthly[monthly.length - 13];
  const endIndex = 1 + end.r_cumulative_pct / 100;
  const baseIndex = 1 + baseline.r_cumulative_pct / 100;
  return (endIndex / baseIndex - 1) * 100;
}

// Mesmo padrão de MonthTooltip (ProfitabilitySection.tsx) — `content`
// customizado em vez de `formatter`, evita a tipagem genérica confusa do
// recharts pra `Tooltip`.
function AllocationTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { payload: { asset_class: string; market_value: number } }[];
}) {
  if (!active || !payload?.length) return null;
  const row = payload[0].payload;
  return (
    <div
      className="rounded-lg border border-border px-3 py-2 text-sm"
      style={{ background: "#111820" }}
    >
      <p className="font-medium">{ASSET_CLASS_LABELS[row.asset_class as AssetClass] ?? row.asset_class}</p>
      <p className="text-muted-foreground">{formatCurrency(row.market_value)}</p>
    </div>
  );
}

function priceSourceHint(source: PositionView["price_source"]): string | null {
  switch (source) {
    case "avg_buy_price":
      return "no live quote — using average buy price";
    case "manual_valuation":
      return "manual valuation (asset_valuations), not a per-unit price";
    case "none":
      return "no price/valuation registered";
    default:
      return null;
  }
}

function SummarySection({ portfolioId }: { portfolioId: number }) {
  const [collapsedClasses, setCollapsedClasses] = useState<Set<string>>(new Set());

  const summaryQuery = useQuery<PortfolioSummary, AppError>({
    queryKey: ["portfolio-summary", portfolioId],
    queryFn: () => invoke("get_portfolio_summary", { portfolioId }),
  });
  const positionsQuery = useQuery<PositionView[], AppError>({
    queryKey: ["positions", portfolioId],
    queryFn: () => invoke("get_portfolio_positions", { portfolioId }),
  });
  const profitabilityQuery = useQuery<MonthlyReturn[], AppError>({
    queryKey: ["profitability", portfolioId],
    queryFn: () => invoke("get_portfolio_profitability", { portfolioId }),
  });

  const summary = summaryQuery.data;
  const positions = positionsQuery.data ?? [];
  const monthly = profitabilityQuery.data ?? [];

  const totalReturnPct = monthly.length > 0 ? monthly[monthly.length - 1].r_cumulative_pct : null;
  const twelveMonthReturnPct = useMemo(() => trailingTwelveMonthPct(monthly), [monthly]);

  const groups = useMemo(() => {
    const byClass = new Map<string, PositionView[]>();
    for (const position of positions) {
      if (position.quantity === 0) continue;
      const list = byClass.get(position.asset_class) ?? [];
      list.push(position);
      byClass.set(position.asset_class, list);
    }
    return [...byClass.entries()]
      .map(([assetClass, items]) => {
        const marketValue = items.reduce((sum, p) => sum + (p.market_value ?? 0), 0);
        const costBasis = items.reduce(
          (sum, p) => sum + (p.average_buy_price ?? 0) * p.quantity,
          0,
        );
        const pl = marketValue - costBasis;
        return {
          assetClass,
          items: items.sort((a, b) => a.ticker.localeCompare(b.ticker)),
          marketValue,
          plPct: costBasis !== 0 ? pl / costBasis : null,
        };
      })
      .sort((a, b) => b.marketValue - a.marketValue);
  }, [positions]);

  function toggleClass(assetClass: string) {
    setCollapsedClasses((prev) => {
      const next = new Set(prev);
      if (next.has(assetClass)) next.delete(assetClass);
      else next.add(assetClass);
      return next;
    });
  }

  if (summaryQuery.isError) {
    return <p className="text-red-600">{summaryQuery.error.message}</p>;
  }
  if (summaryQuery.isLoading || !summary) {
    return <p className="text-muted-foreground">Loading...</p>;
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <KpiCard
          title="Total portfolio value"
          value={formatMoney(summary.total_market_value, summary.base_currency)}
          subtext={`Invested: ${formatMoney(summary.total_cost_basis, summary.base_currency)}`}
        />
        <KpiCard
          title="Profit"
          value={formatMoney(summary.unrealized_pl, summary.base_currency)}
          valueClassName={plColorClass(summary.unrealized_pl)}
          subtext={
            summary.unrealized_pl_pct != null
              ? formatPct(summary.unrealized_pl_pct * 100)
              : undefined
          }
        />
        <KpiCard
          title="Dividends received (12M)"
          value={formatMoney(summary.dividends_received_12m, summary.base_currency)}
        />
        <KpiCard
          title="Return"
          value={twelveMonthReturnPct != null ? formatPct(twelveMonthReturnPct) : "—"}
          valueClassName={plColorClass(twelveMonthReturnPct)}
          subtext={
            totalReturnPct != null
              ? `Total: ${formatPct(totalReturnPct)}`
              : "No return history yet"
          }
        />
      </div>

      {summary.positions_missing_fx > 0 && (
        <p className="text-sm text-muted-foreground" data-testid="summary-missing-fx">
          {summary.positions_missing_fx} position{summary.positions_missing_fx === 1 ? "" : "s"} without an
          exchange rate to {summary.base_currency} — the totals above don't include {summary.positions_missing_fx === 1 ? "it" : "them"}.
          Fetch the pair in Research → FX.
        </p>
      )}

      {summary.positions_missing_price > 0 && (
        <p className="text-sm text-muted-foreground">
          {summary.positions_missing_price} position{summary.positions_missing_price === 1 ? "" : "s"} without a
          registered price/valuation — the totals above don't include {summary.positions_missing_price === 1 ? "it" : "them"}.
        </p>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Allocation by class</CardTitle>
        </CardHeader>
        <CardContent>
          {summary.allocation_by_class.length === 0 ? (
            <p className="text-muted-foreground">
              No priced positions yet — log a purchase in the "Transactions &
              Positions" tab to get started.
            </p>
          ) : (
            <ResponsiveContainer width="100%" height={280}>
              <PieChart>
                <Pie
                  data={summary.allocation_by_class}
                  dataKey="market_value"
                  nameKey="asset_class"
                  innerRadius={64}
                  outerRadius={104}
                  paddingAngle={2}
                  label={(props: { percent?: number }) =>
                    `${((props.percent ?? 0) * 100).toFixed(0)}%`
                  }
                >
                  {summary.allocation_by_class.map((entry) => (
                    <Cell
                      key={entry.asset_class}
                      fill={CLASS_COLORS[entry.asset_class as AssetClass] ?? OTHER_COLOR}
                      stroke="#111820"
                      strokeWidth={2}
                    />
                  ))}
                </Pie>
                <Tooltip content={<AllocationTooltip />} />
                <Legend
                  formatter={(_value, entry) => {
                    const payload = (entry as { payload?: { asset_class: string } }).payload;
                    const assetClass = payload?.asset_class ?? "";
                    return ASSET_CLASS_LABELS[assetClass as AssetClass] ?? assetClass;
                  }}
                />
              </PieChart>
            </ResponsiveContainer>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>My Assets</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          {groups.length === 0 && (
            <p className="text-muted-foreground">No assets in the portfolio yet.</p>
          )}
          {groups.map((group) => {
            const isOpen = !collapsedClasses.has(group.assetClass);
            return (
              <Collapsible key={group.assetClass} open={isOpen} onOpenChange={() => toggleClass(group.assetClass)}>
                <CollapsibleTrigger className="flex w-full items-center justify-between rounded-lg border border-border bg-secondary px-4 py-3 text-left">
                  <span className="flex items-center gap-2 font-medium">
                    {isOpen ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                    {ASSET_CLASS_LABELS[group.assetClass as AssetClass] ?? group.assetClass}
                    <span className="text-sm font-normal text-muted-foreground">
                      ({group.items.length})
                    </span>
                  </span>
                  <span className="flex items-center gap-4 text-sm">
                    <span>{formatCurrency(group.marketValue)}</span>
                    <span className={plColorClass(group.plPct)}>
                      {group.plPct != null ? formatPct(group.plPct * 100) : "—"}
                    </span>
                  </span>
                </CollapsibleTrigger>
                <CollapsibleContent>
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Asset</TableHead>
                        <TableHead>Quantity</TableHead>
                        <TableHead>Avg. price</TableHead>
                        <TableHead>Current price</TableHead>
                        <TableHead>Market value</TableHead>
                        <TableHead>P&L %</TableHead>
                        <TableHead>% Portfolio</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {group.items.map((position) => {
                        const hint = priceSourceHint(position.price_source);
                        const pctOfPortfolio =
                          summary.total_market_value !== 0 && position.market_value != null
                            ? position.market_value / summary.total_market_value
                            : null;
                        return (
                          <TableRow key={position.asset_id}>
                            <TableCell>
                              <p className="font-medium">{position.ticker}</p>
                              <p className="text-xs text-muted-foreground">{position.name}</p>
                              {(position.fx_missing || position.native_currency !== position.currency) && (
                                <p className="text-xs text-muted-foreground">
                                  {position.fx_missing ? `No ${position.native_currency} rate` : `from ${position.native_currency}`}
                                </p>
                              )}
                            </TableCell>
                            <TableCell>{position.quantity}</TableCell>
                            <TableCell>
                              {position.average_buy_price != null
                                ? `${position.currency} ${position.average_buy_price.toFixed(2)}`
                                : "—"}
                            </TableCell>
                            <TableCell title={hint ?? undefined}>
                              {position.current_price != null
                                ? `${position.currency} ${position.current_price.toFixed(2)}`
                                : "—"}
                            </TableCell>
                            <TableCell>
                              {position.market_value != null
                                ? `${position.currency} ${position.market_value.toFixed(2)}`
                                : "—"}
                            </TableCell>
                            <TableCell className={plColorClass(position.unrealized_pl_pct)}>
                              {position.unrealized_pl_pct != null
                                ? formatPct(position.unrealized_pl_pct * 100)
                                : "—"}
                            </TableCell>
                            <TableCell>
                              {pctOfPortfolio != null ? formatPct(pctOfPortfolio * 100) : "—"}
                            </TableCell>
                          </TableRow>
                        );
                      })}
                    </TableBody>
                  </Table>
                </CollapsibleContent>
              </Collapsible>
            );
          })}
        </CardContent>
      </Card>
    </div>
  );
}

export default SummarySection;
