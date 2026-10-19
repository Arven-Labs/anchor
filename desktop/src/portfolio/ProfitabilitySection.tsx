import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { AppError } from "../types";
import type { MonthlyReturn, PositionView, ProfitabilityComparison } from "./types";
import { Button } from "@/components/ui/button";
import { formatMoney } from "@/lib/utils";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

type CollectorSummary = { success: boolean; output: string };

// Subconjunto de ASSET_CLASSES_WITH_AUTO_QUOTE (portfolio/types.ts) elegível
// pro botão de backfill manual em lote abaixo — só as classes que
// compartilham o endpoint Yahoo `.SA` via `run_price_history_backfill`.
// Cripto/Metal/International stock/REIT/ETF (US) ficam de fora de
// propósito: `stock_price_history` delas já é preenchida sozinha a cada
// busca em Research (CoinGecko pra cripto, Yahoo sem `.SA` pras demais —
// ver `collect_crypto_ticker`/`collect_us_price_history` em
// data-collector/main.py) — sem o filtro aqui, mandar um desses tickers pro
// coletor `.SA` genérico tentaria algo como `CRIPTO.SA`/`SPY.SA`, errado.
const YAHOO_BACKFILL_CLASSES = ["acao_br", "fii", "etf_br", "etf_renda_fixa", "bdr"];

// Mesma paleta/convenção de DividendHistoryChart.tsx (rampa de verde do
// tema, nunca dois eixos Y na mesma barra — ver skill dataviz). A polaridade
// (mês positivo/negativo) já é lida pela posição da barra em torno da linha
// de referência zero, não pela cor — evita o par verde/vermelho clássico
// (falha de separação pra daltonismo, confirmado pelo validador da skill).
const BAR_COLOR = "#4ade80"; // --chart-1
const LINE_COLOR = "#22c55e"; // --chart-3
const GRID_COLOR = "#1f2630"; // --border
const AXIS_COLOR = "#9fb1c2"; // --muted-foreground
const TOOLTIP_BG = "#111820"; // --card
const REFERENCE_COLOR = "#3a4553";

// Fase 13.5 — paleta do gráfico "Return vs. benchmarks". Rodei o validador
// de verdade (skill dataviz, `validate_palette.js --pairs all`, não só
// pares adjacentes) contra as 8 cores categóricas já usadas no app: nenhuma
// combinação de mais de 3 delas passa a checagem completa (confirmado pelo
// próprio `references/palette.md` do skill — só os 3 primeiros slots
// validam `--pairs all`). Solução: 3 matizes de verdade (os únicos 3
// validados, ΔE mínimo 9.4 CVD / 20.9 visão normal, PASS limpo). IFIX/SMLL/
// IDIV entraram na Sessão 83 reaproveitando o MESMO matiz do grupo
// "mercado" (IBOV/IVVB11) — decisão explícita do dono do projeto pra não
// estourar a paleta validada com um 4º matiz — diferenciados só por
// `dashArray` (padrão de traço), não por cor.
const PORTFOLIO_COLOR = "#3987e5"; // azul — herói do gráfico
const CDI_COLOR = "#d95926"; // laranja — benchmark mais importante (base do "vs CDI")
const IPCA_COLOR = "#d95926"; // mesmo laranja do CDI, tracejado — referência macro/taxa
const IBOV_COLOR = "#199e70"; // aqua — benchmark de mercado mais importante, reaproveitado por IVVB11/IFIX/SMLL/IDIV (mesmo grupo "mercado", diferenciados por dashArray)

type SeriesConfig = { key: string; label: string; color: string; dashArray?: string };
const PORTFOLIO_SERIES: SeriesConfig = { key: "portfolio", label: "Portfolio", color: PORTFOLIO_COLOR };
const BENCHMARK_SERIES: Record<string, SeriesConfig> = {
  cdi: { key: "cdi", label: "CDI", color: CDI_COLOR },
  ipca: { key: "ipca", label: "IPCA", color: IPCA_COLOR, dashArray: "4 3" },
  // Fase 17 — Meta Selic capitalizada por dia útil. Mesmo grupo laranja
  // "juros/macro" de CDI/IPCA (sem 4º matiz), diferenciada só pelo traço.
  selic: { key: "selic", label: "Selic", color: CDI_COLOR, dashArray: "1 3" },
  "^BVSP": { key: "^BVSP", label: "IBOV", color: IBOV_COLOR },
  IVVB11: { key: "IVVB11", label: "IVVB11", color: IBOV_COLOR, dashArray: "4 3" },
  IFIX: { key: "IFIX", label: "IFIX", color: IBOV_COLOR, dashArray: "1 3" },
  SMLL: { key: "SMLL", label: "SMLL", color: IBOV_COLOR, dashArray: "9 3 2 3" },
  IDIV: { key: "IDIV", label: "IDIV", color: IBOV_COLOR, dashArray: "9 2" },
};

const MONTH_LABELS = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun",
  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

function monthLabel(yearMonth: string): string {
  const [year, month] = yearMonth.split("-");
  return `${MONTH_LABELS[Number(month) - 1]}/${year.slice(2)}`;
}

function formatPct(value: number): string {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

function KpiCard({ title, value, subtext }: { title: string; value: string; subtext?: string }) {
  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <p className="text-sm text-muted-foreground">{title}</p>
      <p className="mt-1 text-2xl font-semibold">{value}</p>
      {subtext && <p className="mt-1 text-xs text-muted-foreground">{subtext}</p>}
    </div>
  );
}

function pctColorClass(value: number | null): string {
  if (value == null || value === 0) return "text-muted-foreground";
  return value > 0 ? "text-primary" : "text-destructive";
}

// Mesma técnica de `trailingTwelveMonthPct` (SummarySection.tsx): rebaseia o
// índice já encadeado (`r_cumulative_pct`, composto desde o início) pra
// isolar um retorno de janela fixa sem pedir nada novo ao backend. Usada
// tanto pra carteira quanto pro CDI, pra manter o "vs CDI" comparando maçã
// com maçã.
function rebasedPct(series: { r_cumulative_pct: number }[], monthsBack: number): number | null {
  if (series.length === 0) return null;
  if (series.length <= monthsBack) return series[series.length - 1].r_cumulative_pct;
  const end = series[series.length - 1];
  const base = series[series.length - 1 - monthsBack];
  return ((1 + end.r_cumulative_pct / 100) / (1 + base.r_cumulative_pct / 100) - 1) * 100;
}

type ProfitabilityYearRow = {
  year: string;
  months: (number | null)[];
  annualReturnPct: number | null;
  cumulativePct: number | null;
};

// Pivota Ano × Jan-Dez (mesma técnica de `pivotByYear` em
// DividendsSummarySection.tsx). "Annual return" é um encadeamento client-side
// só dos meses daquele ano (reimplementação de uma linha de `twr::chain`,
// duplicação aceitável pela convenção já estabelecida); "Cumulative" já vem
// pronto (`r_cumulative_pct` do último mês do ano), sem recálculo.
function pivotProfitabilityByYear(monthly: MonthlyReturn[]): ProfitabilityYearRow[] {
  const byYear = new Map<string, MonthlyReturn[]>();
  for (const row of monthly) {
    const year = row.year_month.slice(0, 4);
    const list = byYear.get(year) ?? [];
    list.push(row);
    byYear.set(year, list);
  }
  return [...byYear.entries()]
    .sort(([a], [b]) => b.localeCompare(a))
    .map(([year, rows]) => {
      const months: (number | null)[] = new Array(12).fill(null);
      for (const row of rows) {
        months[Number(row.year_month.slice(5, 7)) - 1] = row.r_month_pct;
      }
      const annualReturnPct =
        rows.reduce((acc, r) => acc * (1 + r.r_month_pct / 100), 1) - 1;
      return {
        year,
        months,
        annualReturnPct: rows.length > 0 ? annualReturnPct * 100 : null,
        cumulativePct: rows.length > 0 ? rows[rows.length - 1].r_cumulative_pct : null,
      };
    });
}

// Tooltip dedicado do gráfico "Return vs. benchmarks" — ao contrário de
// `MonthTooltip` (uma série só, formato `MonthlyReturn`), aqui cada ponto
// mesclado tem uma chave por série visível (`portfolio`/`cdi`/...), então o
// tooltip lista todas as presentes naquele mês em vez de um campo fixo.
function ComparisonTooltip({
  active,
  payload,
  label,
}: {
  active?: boolean;
  payload?: { dataKey?: string; value?: number; color?: string }[];
  label?: string;
}) {
  if (!active || !payload?.length) return null;
  return (
    <div
      className="rounded-lg border border-border px-3 py-2 text-sm"
      style={{ background: TOOLTIP_BG }}
    >
      <p className="font-medium">{label}</p>
      {payload.map((entry) => {
        const key = String(entry.dataKey ?? "");
        if (entry.value == null) return null;
        const config = key === "portfolio" ? PORTFOLIO_SERIES : BENCHMARK_SERIES[key];
        return (
          <p key={key} style={{ color: entry.color }}>
            {config?.label ?? key}: {formatPct(entry.value)}
          </p>
        );
      })}
    </div>
  );
}

function MonthTooltip({
  active,
  payload,
  field,
}: {
  active?: boolean;
  payload?: { payload: MonthlyReturn }[];
  field: "r_month_pct" | "r_cumulative_pct";
}) {
  if (!active || !payload?.length) return null;
  const row = payload[0].payload;
  return (
    <div
      className="rounded-lg border border-border px-3 py-2 text-sm"
      style={{ background: TOOLTIP_BG }}
    >
      <p className="font-medium">{monthLabel(row.year_month)}</p>
      <p className="text-muted-foreground">{formatPct(row[field])}</p>
    </div>
  );
}

function ProfitabilitySection({ portfolioId }: { portfolioId: number }) {
  const queryClient = useQueryClient();
  const [hiddenSeries, setHiddenSeries] = useState<Set<string>>(new Set());

  const positionsQuery = useQuery<PositionView[], AppError>({
    queryKey: ["positions", portfolioId],
    queryFn: () => invoke("get_portfolio_positions", { portfolioId }),
  });

  const comparisonQuery = useQuery<ProfitabilityComparison, AppError>({
    queryKey: ["profitability-comparison", portfolioId],
    queryFn: () => invoke("get_profitability_comparison", { portfolioId }),
  });

  // Ativos zerados (já vendidos por completo) continuam em
  // get_portfolio_positions — precisam do backfill também, pra precificar
  // os meses passados em que ainda eram carregados.
  const autoQuoteTickers = useMemo(() => {
    const positions = positionsQuery.data ?? [];
    const tickers = positions
      .filter((p) => YAHOO_BACKFILL_CLASSES.includes(p.asset_class))
      .map((p) => p.ticker);
    return [...new Set(tickers)];
  }, [positionsQuery.data]);

  // Cripto não passa pelo backfill do Yahoo `.SA`: o histórico longo em BRL (par `-BRL`) alimenta só o backtest.
  const cryptoTickers = useMemo(
    () => [...new Set((positionsQuery.data ?? []).filter((p) => p.asset_class === "cripto").map((p) => p.ticker))],
    [positionsQuery.data],
  );

  const backfillMutation = useMutation<CollectorSummary, AppError, string[]>({
    mutationFn: async (tickers) => {
      const summary = await invoke<CollectorSummary>("run_price_history_backfill", { tickers });
      if (cryptoTickers.length > 0) {
        await invoke("run_adjusted_close_backfill", { tickers: cryptoTickers, global: true, crypto: true });
      }
      return summary;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["profitability-comparison", portfolioId] });
    },
  });

  const benchmarkBackfillMutation = useMutation<CollectorSummary, AppError, void>({
    mutationFn: () => invoke("run_benchmark_backfill"),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["profitability-comparison", portfolioId] });
    },
  });

  const monthlyReturns = comparisonQuery.data?.portfolio ?? [];
  const benchmarks = comparisonQuery.data?.benchmarks ?? [];
  const baseCurrency = comparisonQuery.data?.base_currency ?? "BRL";
  const chartData = monthlyReturns.map((row) => ({ ...row, label: monthLabel(row.year_month) }));

  const cdiSeries = benchmarks.find((b) => b.code === "cdi");
  const yearRows = useMemo(() => pivotProfitabilityByYear(monthlyReturns), [monthlyReturns]);

  // Gráfico "Return vs. benchmarks": rentabilidade acumulada da carteira
  // sobreposta às dos benchmarks disponíveis, mesclado por year_month (a
  // janela do portfolio já é a certa — get_profitability_comparison recorta
  // os benchmarks pra ela).
  const comparisonChartData = useMemo(() => {
    const byCode = new Map(benchmarks.map((b) => [b.code, new Map(b.monthly.map((p) => [p.year_month, p.r_cumulative_pct]))]));
    return monthlyReturns.map((row) => {
      const point: Record<string, number | string> = {
        year_month: row.year_month,
        label: monthLabel(row.year_month),
        portfolio: row.r_cumulative_pct,
      };
      for (const b of benchmarks) {
        const value = byCode.get(b.code)?.get(row.year_month);
        if (value != null) point[b.code] = value;
      }
      return point;
    });
  }, [monthlyReturns, benchmarks]);

  function toggleSeries(key: string) {
    setHiddenSeries((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  function vsCdiSubtext(portfolioValue: number | null, cdiValue: number | null | undefined): string {
    if (portfolioValue == null || cdiValue == null) return "vs CDI: no data yet";
    const delta = portfolioValue - cdiValue;
    return `vs CDI: ${delta >= 0 ? "+" : ""}${delta.toFixed(2)} pp`;
  }

  const totalReturnPct = monthlyReturns.length > 0 ? monthlyReturns[monthlyReturns.length - 1].r_cumulative_pct : null;
  const last12MonthsPct = rebasedPct(monthlyReturns, 12);
  const lastMonthPct = monthlyReturns.length > 0 ? monthlyReturns[monthlyReturns.length - 1].r_month_pct : null;

  const cdiTotalPct = cdiSeries ? cdiSeries.monthly[cdiSeries.monthly.length - 1]?.r_cumulative_pct ?? null : null;
  const cdiLast12MonthsPct = cdiSeries ? rebasedPct(cdiSeries.monthly, 12) : null;
  const cdiLastMonthPct = cdiSeries ? cdiSeries.monthly[cdiSeries.monthly.length - 1]?.r_month_pct ?? null : null;

  return (
    <div className="flex flex-col gap-6">
      {chartData.length > 0 && (
        <div className="grid grid-cols-1 gap-6 lg:grid-cols-[320px_1fr]">
          <div className="flex flex-col gap-4">
            <KpiCard
              title="Total return"
              value={totalReturnPct != null ? formatPct(totalReturnPct) : "—"}
              subtext={vsCdiSubtext(totalReturnPct, cdiTotalPct)}
            />
            <KpiCard
              title="Last 12 months"
              value={last12MonthsPct != null ? formatPct(last12MonthsPct) : "—"}
              subtext={vsCdiSubtext(last12MonthsPct, cdiLast12MonthsPct)}
            />
            <KpiCard
              title="Last month"
              value={lastMonthPct != null ? formatPct(lastMonthPct) : "—"}
              subtext={vsCdiSubtext(lastMonthPct, cdiLastMonthPct)}
            />
          </div>

          {baseCurrency !== "BRL" && (
            <p className="text-sm text-muted-foreground" data-testid="benchmarks-hidden">
              Returns are in {baseCurrency}. The benchmarks (CDI, IPCA, Selic and the Brazilian indices) are in
              BRL, so they are hidden while the base currency is {baseCurrency}.
            </p>
          )}

          <Card>
            <CardHeader>
              <CardTitle>Return vs. benchmarks</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-2">
              <ResponsiveContainer width="100%" height={280}>
                <LineChart data={comparisonChartData} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
                  <CartesianGrid stroke={GRID_COLOR} vertical={false} />
                  <XAxis dataKey="label" stroke={AXIS_COLOR} fontSize={12} tickLine={false} />
                  <YAxis
                    stroke={AXIS_COLOR}
                    fontSize={12}
                    tickLine={false}
                    axisLine={false}
                    width={48}
                    tickFormatter={(value: number) => `${value}%`}
                  />
                  <ReferenceLine y={0} stroke={REFERENCE_COLOR} />
                  <Tooltip content={<ComparisonTooltip />} cursor={{ stroke: GRID_COLOR }} />
                  <Legend
                    onClick={(entry) => toggleSeries(String(entry.dataKey))}
                    formatter={(_value, entry) => {
                      const key = String((entry as { dataKey?: string }).dataKey ?? "");
                      const config = key === "portfolio" ? PORTFOLIO_SERIES : BENCHMARK_SERIES[key];
                      const label = config?.label ?? key;
                      return hiddenSeries.has(key) ? `${label} (hidden)` : label;
                    }}
                  />
                  <Line
                    type="monotone"
                    dataKey="portfolio"
                    name="portfolio"
                    stroke={PORTFOLIO_COLOR}
                    strokeWidth={2.5}
                    dot={false}
                    hide={hiddenSeries.has("portfolio")}
                  />
                  {benchmarks.map((b) => {
                    const config = BENCHMARK_SERIES[b.code];
                    if (!config) return null;
                    return (
                      <Line
                        key={b.code}
                        type="monotone"
                        dataKey={b.code}
                        name={b.code}
                        stroke={config.color}
                        strokeWidth={1.5}
                        strokeDasharray={config.dashArray}
                        dot={false}
                        hide={hiddenSeries.has(b.code)}
                      />
                    );
                  })}
                </LineChart>
              </ResponsiveContainer>
            </CardContent>
          </Card>
        </div>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Profitability (TWR / Modified Dietz)</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <p className="text-sm text-muted-foreground">
            Covers Stock (B3), FII, ETF (B3 and US), Crypto, BDR, Metal, International stock and
            REIT in this slice — contributions/withdrawals and the other classes (Tesouro Direto,
            Fixed income) don't factor into this calculation yet, for lack of automated
            historical prices for them.
          </p>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => backfillMutation.mutate(autoQuoteTickers)}
              disabled={backfillMutation.isPending || (autoQuoteTickers.length === 0 && cryptoTickers.length === 0)}
            >
              {backfillMutation.isPending ? "Updating..." : "Update price history"}
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => benchmarkBackfillMutation.mutate()}
              disabled={benchmarkBackfillMutation.isPending}
            >
              {benchmarkBackfillMutation.isPending ? "Updating..." : "Update benchmark data"}
            </Button>
            {autoQuoteTickers.length === 0 && (
              <p className="w-full text-sm text-muted-foreground">
                No Stock (B3), FII, ETF (B3) or BDR assets in the portfolio yet. (Crypto, Metal,
                International stock, REIT and ETF (US) price history update automatically when
                their quote is fetched — no button needed for them.)
              </p>
            )}
          </div>

          {backfillMutation.isError && (
            <p className="text-red-600">{backfillMutation.error.message}</p>
          )}
          {benchmarkBackfillMutation.isError && (
            <p className="text-red-600">{benchmarkBackfillMutation.error.message}</p>
          )}
          {comparisonQuery.isError && (
            <p className="text-red-600">
              {comparisonQuery.error.message} — if it's a missing price, run "Update price
              history" above.
            </p>
          )}

          {chartData.length > 0 && (
            <>
              <div>
                <p className="mb-1 text-sm text-muted-foreground">Monthly return</p>
                <ResponsiveContainer width="100%" height={200}>
                  <BarChart data={chartData} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
                    <CartesianGrid stroke={GRID_COLOR} vertical={false} />
                    <XAxis dataKey="label" stroke={AXIS_COLOR} fontSize={12} tickLine={false} />
                    <YAxis
                      stroke={AXIS_COLOR}
                      fontSize={12}
                      tickLine={false}
                      axisLine={false}
                      width={48}
                      tickFormatter={(value: number) => `${value}%`}
                    />
                    <ReferenceLine y={0} stroke={REFERENCE_COLOR} />
                    <Tooltip
                      content={<MonthTooltip field="r_month_pct" />}
                      cursor={{ fill: GRID_COLOR }}
                    />
                    <Bar dataKey="r_month_pct" fill={BAR_COLOR} radius={[4, 4, 4, 4]} />
                  </BarChart>
                </ResponsiveContainer>
              </div>

              <div>
                <p className="mb-1 text-sm text-muted-foreground">Cumulative return</p>
                <ResponsiveContainer width="100%" height={200}>
                  <LineChart data={chartData} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
                    <CartesianGrid stroke={GRID_COLOR} vertical={false} />
                    <XAxis dataKey="label" stroke={AXIS_COLOR} fontSize={12} tickLine={false} />
                    <YAxis
                      stroke={AXIS_COLOR}
                      fontSize={12}
                      tickLine={false}
                      axisLine={false}
                      width={48}
                      tickFormatter={(value: number) => `${value}%`}
                    />
                    <ReferenceLine y={0} stroke={REFERENCE_COLOR} />
                    <Tooltip
                      content={<MonthTooltip field="r_cumulative_pct" />}
                      cursor={{ stroke: GRID_COLOR }}
                    />
                    <Line
                      type="monotone"
                      dataKey="r_cumulative_pct"
                      stroke={LINE_COLOR}
                      strokeWidth={2}
                      dot={false}
                    />
                  </LineChart>
                </ResponsiveContainer>
              </div>
            </>
          )}

          {!comparisonQuery.isError && chartData.length === 0 && (
            <p className="text-muted-foreground">
              No Stock (B3), FII or ETF transactions yet — log a purchase in the "Transactions &
              Positions" tab to start tracking profitability.
            </p>
          )}

          {chartData.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Month</TableHead>
                  <TableHead>BMV</TableHead>
                  <TableHead>Cash flow</TableHead>
                  <TableHead>EMV</TableHead>
                  <TableHead>R month</TableHead>
                  <TableHead>R cumulative</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {monthlyReturns.map((row) => (
                  <TableRow key={row.year_month}>
                    <TableCell>{monthLabel(row.year_month)}</TableCell>
                    <TableCell>{formatMoney(row.bmv, baseCurrency)}</TableCell>
                    <TableCell>{formatMoney(row.cf_total, baseCurrency)}</TableCell>
                    <TableCell>{formatMoney(row.emv, baseCurrency)}</TableCell>
                    <TableCell>{formatPct(row.r_month_pct)}</TableCell>
                    <TableCell>{formatPct(row.r_cumulative_pct)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {yearRows.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Return</CardTitle>
          </CardHeader>
          <CardContent className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Year</TableHead>
                  {MONTH_LABELS.map((label) => (
                    <TableHead key={label}>{label}</TableHead>
                  ))}
                  <TableHead>Annual return</TableHead>
                  <TableHead>Cumulative</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {yearRows.map((row) => (
                  <TableRow key={row.year}>
                    <TableCell className="font-medium">{row.year}</TableCell>
                    {row.months.map((value, index) => (
                      <TableCell key={index} className={pctColorClass(value)}>
                        {value != null ? formatPct(value) : "—"}
                      </TableCell>
                    ))}
                    <TableCell className={pctColorClass(row.annualReturnPct)}>
                      {row.annualReturnPct != null ? formatPct(row.annualReturnPct) : "—"}
                    </TableCell>
                    <TableCell className={pctColorClass(row.cumulativePct)}>
                      {row.cumulativePct != null ? formatPct(row.cumulativePct) : "—"}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

export default ProfitabilitySection;
