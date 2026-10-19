import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery } from "@tanstack/react-query";
import { CartesianGrid, Legend, Line, LineChart, ReferenceLine, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { AppError } from "../types";
import { ASSET_CLASSES_WITH_AUTO_QUOTE, type PositionView } from "./types";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { formatMoney } from "@/lib/utils";

// Fase 22 — backtest da carteira atual no passado (commands/backtest.rs): pesos fixos, rebalanceamento periódico,
// aporte mensal, retorno total (fechamento ajustado), corretagem por ordem, IR de renda variável opcional e comparação
// com CDI, IPCA e IBOV.

type BacktestResponse = {
  start: string;
  end: string;
  assets: { asset_id: number; ticker: string; target_weight: number; final_weight: number; price_only: boolean }[];
  metrics: {
    total_invested: number;
    final_value: number;
    twr_pct: number;
    cagr_pct: number;
    volatility_pct: number;
    max_drawdown_pct: number;
    max_drawdown_date: string | null;
    sharpe: number | null;
    rebalance_count: number;
    total_fees: number;
    total_tax_paid: number;
    tax_pending: number;
    risk_free_pct: number;
  };
  months: {
    date: string;
    value: number;
    invested: number;
    portfolio_index: number;
    cdi_index: number | null;
    ipca_index: number | null;
    ibov_index: number | null;
  }[];
  benchmarks: { code: string; label: string; total_return_pct: number; cagr_pct: number }[];
  names: string[];
  correlation: number[][];
  warnings: string[];
};

const REBALANCE_OPTIONS = [
  { value: "never", label: "Never (buy and hold)" },
  { value: "monthly", label: "Monthly" },
  { value: "quarterly", label: "Quarterly" },
  { value: "yearly", label: "Yearly" },
];

// Mesmas cores de "Return vs. benchmarks" (ProfitabilitySection), as validadas para daltonismo.
const GRID_COLOR = "#1f2630";
const AXIS_COLOR = "#9fb1c2";
const REFERENCE_COLOR = "#3a4553";
const SERIES = [
  { key: "portfolio", label: "Portfolio", color: "#3987e5", dash: undefined },
  { key: "cdi", label: "CDI", color: "#d95926", dash: undefined },
  { key: "ipca", label: "IPCA", color: "#d95926", dash: "4 3" },
  { key: "ibov", label: "IBOV", color: "#199e70", dash: undefined },
];

const pct = (value: number) => `${value.toFixed(2)}%`;

function BacktestSection({ portfolioId }: { portfolioId: number }) {
  const [rebalance, setRebalance] = useState("monthly");
  const [initial, setInitial] = useState("10000");
  const [contribution, setContribution] = useState("0");
  const [startDate, setStartDate] = useState("");
  const [brokerage, setBrokerage] = useState("0");
  const [applyTax, setApplyTax] = useState(false);
  const [weightEdits, setWeightEdits] = useState<Record<number, string>>({});

  const positionsQuery = useQuery<PositionView[], AppError>({
    queryKey: ["backtest-positions", portfolioId],
    queryFn: () => invoke("get_portfolio_positions", { portfolioId }),
  });

  // Só o que o motor consegue simular; o resto aparece como aviso no resultado.
  const eligible = useMemo(
    () =>
      (positionsQuery.data ?? []).filter(
        (p) => p.quantity > 0 && (p.native_currency === "BRL" || p.asset_class === "cripto") && (ASSET_CLASSES_WITH_AUTO_QUOTE as string[]).includes(p.asset_class),
      ),
    [positionsQuery.data],
  );
  const defaultWeights = useMemo(() => {
    const total = eligible.reduce((sum, p) => sum + (p.market_value ?? 0), 0);
    return Object.fromEntries(eligible.map((p) => [p.asset_id, total > 0 ? ((p.market_value ?? 0) / total) * 100 : 0]));
  }, [eligible]);
  const weightOf = (assetId: number) => {
    const edited = weightEdits[assetId];
    return edited !== undefined ? Number(edited) : (defaultWeights[assetId] ?? 0);
  };

  const run = useMutation<BacktestResponse, AppError, void>({
    mutationFn: () =>
      invoke("run_backtest", {
        request: {
          portfolio_id: portfolioId,
          weights: eligible.map((p) => ({ asset_id: p.asset_id, weight: weightOf(p.asset_id) })),
          rebalance,
          initial: Number(initial),
          monthly_contribution: Number(contribution) || 0,
          start_date: startDate === "" ? null : startDate,
          brokerage_per_order: Number(brokerage) || 0,
          apply_tax: applyTax,
        },
      }),
  });
  const result = run.data;

  const chartData = useMemo(
    () =>
      (result?.months ?? []).map((m) => ({
        date: m.date,
        portfolio: (m.portfolio_index - 1) * 100,
        cdi: m.cdi_index === null ? null : (m.cdi_index - 1) * 100,
        ipca: m.ipca_index === null ? null : (m.ipca_index - 1) * 100,
        ibov: m.ibov_index === null ? null : (m.ibov_index - 1) * 100,
      })),
    [result],
  );

  const ready = Number(initial) > 0 && Number(brokerage) >= 0 && eligible.length > 0 && eligible.some((p) => weightOf(p.asset_id) > 0);

  return (
    <div className="flex flex-col gap-4" data-testid="backtest-section">
      <Card>
        <CardHeader>
          <CardTitle>Backtest</CardTitle>
          <CardDescription>
            Replays your current holdings in the past with fixed weights, optional rebalancing and monthly
            contributions. Returns include dividends (adjusted close). Brokerage and capital gains tax are optional
            costs. Past performance does not predict future results.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <Field label="Rebalance">
              <Select value={rebalance} onValueChange={setRebalance}>
                <SelectTrigger className="w-full" data-testid="backtest-rebalance">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {REBALANCE_OPTIONS.map((o) => (
                    <SelectItem key={o.value} value={o.value}>
                      {o.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <Field label="Initial amount (BRL)">
              <Input type="number" min="0" step="100" data-testid="backtest-initial" value={initial} onChange={(e) => setInitial(e.currentTarget.value)} />
            </Field>
            <Field label="Monthly contribution (BRL)">
              <Input type="number" min="0" step="100" data-testid="backtest-contribution" value={contribution} onChange={(e) => setContribution(e.currentTarget.value)} />
            </Field>
            <Field label="Start date (optional)">
              <Input type="date" data-testid="backtest-start" value={startDate} onChange={(e) => setStartDate(e.currentTarget.value)} />
            </Field>
            <Field label="Brokerage per order (BRL)">
              <Input type="number" min="0" step="0.01" data-testid="backtest-brokerage" value={brokerage} onChange={(e) => setBrokerage(e.currentTarget.value)} />
            </Field>
            <label className="flex items-center gap-2 self-end pb-2 text-sm">
              <input type="checkbox" data-testid="backtest-apply-tax" checked={applyTax} onChange={(e) => setApplyTax(e.currentTarget.checked)} />
              Apply capital gains tax (paid the month after each sale)
            </label>
          </div>

          {positionsQuery.isLoading ? (
            <p className="text-sm text-muted-foreground">Loading positions...</p>
          ) : eligible.length === 0 ? (
            <p className="text-sm text-muted-foreground" data-testid="backtest-empty">
              No open position can be simulated: it takes stocks, FIIs, ETFs, BDRs or crypto in BRL with price history.
            </p>
          ) : (
            <div className="flex flex-col gap-2">
              <h3 className="text-sm font-medium">Target weights (%)</h3>
              <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
                {eligible.map((p) => (
                  <Field key={p.asset_id} label={p.ticker}>
                    <Input
                      type="number"
                      min="0"
                      step="1"
                      data-testid={`backtest-weight-${p.ticker}`}
                      value={weightEdits[p.asset_id] ?? defaultWeights[p.asset_id].toFixed(1)}
                      onChange={(e) => setWeightEdits({ ...weightEdits, [p.asset_id]: e.currentTarget.value })}
                    />
                  </Field>
                ))}
              </div>
              <p className="text-xs text-muted-foreground">Start from your current market value. The weights are normalized to 100%.</p>
            </div>
          )}

          <div>
            <Button type="button" data-testid="backtest-run" disabled={!ready || run.isPending} onClick={() => run.mutate()}>
              {run.isPending ? "Simulating..." : "Run backtest"}
            </Button>
          </div>
          {run.isError && (
            <p className="text-sm text-destructive" data-testid="backtest-error">
              {run.error.message}
            </p>
          )}
        </CardContent>
      </Card>

      {result && (
        <div className="flex flex-col gap-4" data-testid="backtest-result">
          {result.warnings.length > 0 && (
            <Card>
              <CardContent className="pt-6">
                <ul className="list-disc pl-5 text-sm text-muted-foreground" data-testid="backtest-warnings">
                  {result.warnings.map((w) => (
                    <li key={w}>{w}</li>
                  ))}
                </ul>
              </CardContent>
            </Card>
          )}

          <Card>
            <CardHeader>
              <CardTitle>
                {result.start} to {result.end}
              </CardTitle>
              <CardDescription>Cumulative return since the start of the window.</CardDescription>
            </CardHeader>
            <CardContent>
              <div data-testid="backtest-chart">
                <ResponsiveContainer width="100%" height={300}>
                  <LineChart data={chartData} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
                    <CartesianGrid stroke={GRID_COLOR} vertical={false} />
                    <XAxis dataKey="date" stroke={AXIS_COLOR} fontSize={12} tickLine={false} minTickGap={32} />
                    <YAxis stroke={AXIS_COLOR} fontSize={12} tickLine={false} axisLine={false} width={52} tickFormatter={(v: number) => `${v.toFixed(0)}%`} />
                    <ReferenceLine y={0} stroke={REFERENCE_COLOR} />
                    <Tooltip
                      contentStyle={{ background: "#111820", border: `1px solid ${GRID_COLOR}` }}
                      formatter={(value) => (typeof value === "number" ? `${value.toFixed(2)}%` : "")}
                    />
                    <Legend />
                    {SERIES.map((s) => (
                      <Line key={s.key} type="monotone" dataKey={s.key} name={s.label} stroke={s.color} strokeDasharray={s.dash} strokeWidth={s.key === "portfolio" ? 2.5 : 1.5} dot={false} connectNulls />
                    ))}
                  </LineChart>
                </ResponsiveContainer>
              </div>
            </CardContent>
          </Card>

          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle>Result</CardTitle>
              </CardHeader>
              <CardContent>
                <Table data-testid="backtest-metrics">
                  <TableBody>
                    <TableRow><TableCell>Invested</TableCell><TableCell className="text-right">{formatMoney(result.metrics.total_invested, "BRL")}</TableCell></TableRow>
                    <TableRow><TableCell>Final value</TableCell><TableCell className="text-right" data-testid="backtest-final-value">{formatMoney(result.metrics.final_value, "BRL")}</TableCell></TableRow>
                    <TableRow><TableCell>Return (time-weighted)</TableCell><TableCell className="text-right">{pct(result.metrics.twr_pct)}</TableCell></TableRow>
                    <TableRow><TableCell>Annualized return</TableCell><TableCell className="text-right">{pct(result.metrics.cagr_pct)}</TableCell></TableRow>
                    <TableRow><TableCell>Volatility (annual)</TableCell><TableCell className="text-right">{pct(result.metrics.volatility_pct)}</TableCell></TableRow>
                    <TableRow>
                      <TableCell>Max drawdown</TableCell>
                      <TableCell className="text-right">
                        {pct(result.metrics.max_drawdown_pct)}
                        {result.metrics.max_drawdown_date ? ` (${result.metrics.max_drawdown_date})` : ""}
                      </TableCell>
                    </TableRow>
                    <TableRow>
                      <TableCell>Sharpe (vs CDI {pct(result.metrics.risk_free_pct)})</TableCell>
                      <TableCell className="text-right">{result.metrics.sharpe === null ? "-" : result.metrics.sharpe.toFixed(2)}</TableCell>
                    </TableRow>
                    <TableRow><TableCell>Rebalances</TableCell><TableCell className="text-right">{result.metrics.rebalance_count}</TableCell></TableRow>
                    {result.metrics.total_fees > 0 && (
                      <TableRow><TableCell>Brokerage paid</TableCell><TableCell className="text-right" data-testid="backtest-fees">{formatMoney(result.metrics.total_fees, "BRL")}</TableCell></TableRow>
                    )}
                    {(result.metrics.total_tax_paid > 0 || result.metrics.tax_pending > 0) && (
                      <>
                        <TableRow><TableCell>Tax paid</TableCell><TableCell className="text-right" data-testid="backtest-tax-paid">{formatMoney(result.metrics.total_tax_paid, "BRL")}</TableCell></TableRow>
                        <TableRow><TableCell>Tax still due</TableCell><TableCell className="text-right" data-testid="backtest-tax-pending">{formatMoney(result.metrics.tax_pending, "BRL")}</TableCell></TableRow>
                      </>
                    )}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Benchmarks</CardTitle>
              </CardHeader>
              <CardContent>
                <Table data-testid="backtest-benchmarks">
                  <TableHeader>
                    <TableRow>
                      <TableHead />
                      <TableHead className="text-right">Total</TableHead>
                      <TableHead className="text-right">Per year</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    <TableRow>
                      <TableCell>Portfolio</TableCell>
                      <TableCell className="text-right">{pct(result.metrics.twr_pct)}</TableCell>
                      <TableCell className="text-right">{pct(result.metrics.cagr_pct)}</TableCell>
                    </TableRow>
                    {result.benchmarks.map((b) => (
                      <TableRow key={b.code}>
                        <TableCell>{b.label}</TableCell>
                        <TableCell className="text-right">{pct(b.total_return_pct)}</TableCell>
                        <TableCell className="text-right">{pct(b.cagr_pct)}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </div>

          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle>Weights</CardTitle>
              </CardHeader>
              <CardContent>
                <Table data-testid="backtest-assets">
                  <TableHeader>
                    <TableRow>
                      <TableHead>Asset</TableHead>
                      <TableHead className="text-right">Target</TableHead>
                      <TableHead className="text-right">At the end</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {result.assets.map((a) => (
                      <TableRow key={a.asset_id}>
                        <TableCell>{a.ticker}{a.price_only ? " (price only)" : ""}</TableCell>
                        <TableCell className="text-right">{pct(a.target_weight * 100)}</TableCell>
                        <TableCell className="text-right">{pct(a.final_weight * 100)}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>

            {result.names.length > 1 && (
              <Card>
                <CardHeader>
                  <CardTitle>Correlation</CardTitle>
                  <CardDescription>Daily returns, 1.00 moves together, -1.00 in opposite directions.</CardDescription>
                </CardHeader>
                <CardContent className="overflow-x-auto">
                  <Table data-testid="backtest-correlation">
                    <TableHeader>
                      <TableRow>
                        <TableHead />
                        {result.names.map((n) => (
                          <TableHead key={n} className="text-right">{n}</TableHead>
                        ))}
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {result.correlation.map((row, i) => (
                        <TableRow key={result.names[i]}>
                          <TableCell>{result.names[i]}</TableCell>
                          {row.map((value, j) => (
                            <TableCell key={result.names[j]} className="text-right">{value.toFixed(2)}</TableCell>
                          ))}
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </CardContent>
              </Card>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default BacktestSection;
