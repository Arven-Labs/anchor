import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Legend,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { AppError } from "../types";
import type { ProventoRow, ProventosMonthlyBucket, ProventosSummary } from "./types";
import { ASSET_CLASS_LABELS, PAYMENT_TYPE_LABELS, type AssetClass, type PaymentType } from "./types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import DividendSuggestionsSection from "./DividendSuggestionsSection";

// Fase 13.4 — tela dedicada "Dividends" (Proventos): resumo (KPIs + donut por
// ativo), evolução mensal/anual empilhando pago (sólido) vs. a receber
// (translúcido), histórico Ano×Mês e a tabela linha-a-linha "My dividends"
// (`commands/proventos.rs::get_proventos_summary`). "Criar meta" do pedido
// original fica de fora (decisão do dono do projeto, sem integração
// desenhada com a aba Metas) e yield on cost também (ambíguo no pedido
// original).

// Mesmos 8 tons já validados em SummarySection.tsx (skill dataviz,
// `validate_palette.js --mode dark --surface "#111820"`), só que aqui
// reaproveitados por RANKING (top-7 ativos por proventos recebidos em 12M),
// não por classe — é um "top N + outros" de verdade, não um filtro que muda
// de composição, então repintar por posição é o padrão esperado aqui.
const RANK_COLORS = [
  "#3987e5",
  "#d95926",
  "#199e70",
  "#c98500",
  "#d55181",
  "#008300",
  "#9085e9",
  "#e66767",
];
const OTHER_COLOR = "#9fb1c2"; // --muted-foreground

// Par recebido/a-receber: mesmo azul de "Buys" (LedgerSection.tsx, já
// validado); "a receber" usa o mesmo hue a 35% de opacidade + traço de borda
// a 100% (skill dataviz: opacidade separa por luminância, não por matiz, então
// sobrevive a daltonismo — a legenda abaixo garante que a identidade nunca
// depende só da cor).
const RECEIVED_COLOR = "#3987e5";
const GRID_COLOR = "#1f2630"; // --border
const AXIS_COLOR = "#9fb1c2"; // --muted-foreground
const TOOLTIP_BG = "#111820"; // --card

const MONTH_LABELS = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun",
  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

function monthLabel(yearMonth: string): string {
  const [year, month] = yearMonth.split("-");
  return `${MONTH_LABELS[Number(month) - 1]}/${year.slice(2)}`;
}

function formatCurrency(value: number): string {
  return value.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function formatDate(date: string | null): string {
  if (!date) return "—";
  const [y, m, d] = date.split("-");
  return `${d}/${m}/${y}`;
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

type Granularity = "monthly" | "annual";

type ChartBucket = { key: string; label: string; received: number; expected: number };

function toChartBuckets(monthly: ProventosMonthlyBucket[], granularity: Granularity): ChartBucket[] {
  if (granularity === "monthly") {
    return monthly.map((b) => ({
      key: b.year_month,
      label: monthLabel(b.year_month),
      received: b.received,
      expected: b.expected,
    }));
  }
  const byYear = new Map<string, { received: number; expected: number }>();
  for (const b of monthly) {
    const year = b.year_month.slice(0, 4);
    const entry = byYear.get(year) ?? { received: 0, expected: 0 };
    entry.received += b.received;
    entry.expected += b.expected;
    byYear.set(year, entry);
  }
  return [...byYear.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([year, v]) => ({ key: year, label: year, ...v }));
}

function DonutTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { payload: { ticker: string; total: number } }[];
}) {
  if (!active || !payload?.length) return null;
  const row = payload[0].payload;
  return (
    <div className="rounded-lg border border-border px-3 py-2 text-sm" style={{ background: TOOLTIP_BG }}>
      <p className="font-medium">{row.ticker}</p>
      <p className="text-muted-foreground">{formatCurrency(row.total)}</p>
    </div>
  );
}

function EvolutionTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { payload: ChartBucket }[];
}) {
  if (!active || !payload?.length) return null;
  const bucket = payload[0].payload;
  return (
    <div className="rounded-lg border border-border px-3 py-2 text-sm" style={{ background: TOOLTIP_BG }}>
      <p className="font-medium">{bucket.label}</p>
      <p style={{ color: RECEIVED_COLOR }}>Received: {formatCurrency(bucket.received)}</p>
      <p className="text-muted-foreground">Receivable: {formatCurrency(bucket.expected)}</p>
    </div>
  );
}

type YearRow = { year: string; months: (number | null)[]; average: number; total: number };

// Pivota a série mensal (recebido + a receber, já que a tabela mostra "o que
// existiu naquele mês", pago ou projetado) em Ano × Jan-Dez. Média é sobre os
// meses com dado, não /12 fixo — um ano em andamento não deve ter a média
// diluída pelos meses futuros ainda sem nada.
function pivotByYear(monthly: ProventosMonthlyBucket[]): YearRow[] {
  const byYear = new Map<string, (number | null)[]>();
  for (const bucket of monthly) {
    const [year, month] = bucket.year_month.split("-");
    const months = byYear.get(year) ?? new Array(12).fill(null);
    const idx = Number(month) - 1;
    months[idx] = (months[idx] ?? 0) + bucket.received + bucket.expected;
    byYear.set(year, months);
  }
  return [...byYear.entries()]
    .sort(([a], [b]) => b.localeCompare(a))
    .map(([year, months]) => {
      const values = months.filter((v): v is number => v != null);
      const total = values.reduce((sum, v) => sum + v, 0);
      return { year, months, average: values.length > 0 ? total / values.length : 0, total };
    });
}

function DividendsSummarySection({ portfolioId }: { portfolioId: number }) {
  const [granularity, setGranularity] = useState<Granularity>("monthly");
  const [suggestionsOpen, setSuggestionsOpen] = useState(false);

  const summaryQuery = useQuery<ProventosSummary, AppError>({
    queryKey: ["proventos-summary", portfolioId],
    queryFn: () => invoke("get_proventos_summary", { portfolioId }),
  });

  const summary = summaryQuery.data;

  const donutData = useMemo(() => {
    if (!summary) return [];
    const top = summary.by_asset_12m.slice(0, 7);
    const rest = summary.by_asset_12m.slice(7);
    const restTotal = rest.reduce((sum, a) => sum + a.total, 0);
    const entries = top.map((a) => ({ ticker: a.ticker, total: a.total }));
    if (restTotal > 0) entries.push({ ticker: "Others", total: restTotal });
    return entries;
  }, [summary]);

  const chartData = useMemo(
    () => (summary ? toChartBuckets(summary.monthly, granularity) : []),
    [summary, granularity],
  );

  const yearRows = useMemo(() => (summary ? pivotByYear(summary.monthly) : []), [summary]);

  if (summaryQuery.isError) {
    return <p className="text-red-600">{summaryQuery.error.message}</p>;
  }
  if (summaryQuery.isLoading || !summary) {
    return <p className="text-muted-foreground">Loading...</p>;
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex justify-end">
        <Button type="button" variant="outline" onClick={() => setSuggestionsOpen(true)}>
          Manage suggestions
        </Button>
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[320px_1fr]">
        <div className="flex flex-col gap-4">
          <KpiCard
            title={`Avg. monthly (12M) · ${summary.base_currency}`}
            value={formatCurrency(summary.avg_monthly_12m)}
          />
          <KpiCard title={`Total (12M) · ${summary.base_currency}`} value={formatCurrency(summary.total_12m)} />
          <KpiCard
            title={`Total (all time) · ${summary.base_currency}`}
            value={formatCurrency(summary.total_all_time)}
          />
          <Card>
            <CardHeader>
              <CardTitle>Dividend distribution (12M)</CardTitle>
            </CardHeader>
            <CardContent>
              {donutData.length === 0 ? (
                <p className="text-sm text-muted-foreground">No dividends received in the last 12 months.</p>
              ) : (
                <ResponsiveContainer width="100%" height={220}>
                  <PieChart>
                    <Pie
                      data={donutData}
                      dataKey="total"
                      nameKey="ticker"
                      innerRadius={48}
                      outerRadius={80}
                      paddingAngle={2}
                    >
                      {donutData.map((entry, index) => (
                        <Cell
                          key={entry.ticker}
                          fill={index < RANK_COLORS.length ? RANK_COLORS[index] : OTHER_COLOR}
                          stroke="#111820"
                          strokeWidth={2}
                        />
                      ))}
                    </Pie>
                    <Tooltip content={<DonutTooltip />} />
                    <Legend />
                  </PieChart>
                </ResponsiveContainer>
              )}
            </CardContent>
          </Card>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Dividend evolution</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="flex gap-2">
              <Button
                type="button"
                size="sm"
                variant={granularity === "monthly" ? "default" : "outline"}
                onClick={() => setGranularity("monthly")}
              >
                Monthly
              </Button>
              <Button
                type="button"
                size="sm"
                variant={granularity === "annual" ? "default" : "outline"}
                onClick={() => setGranularity("annual")}
              >
                Annual
              </Button>
            </div>

            {chartData.length === 0 ? (
              <p className="text-muted-foreground">No dividend history yet.</p>
            ) : (
              <ResponsiveContainer width="100%" height={280}>
                <BarChart data={chartData} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
                  <CartesianGrid stroke={GRID_COLOR} vertical={false} />
                  <XAxis dataKey="label" stroke={AXIS_COLOR} fontSize={12} tickLine={false} />
                  <YAxis
                    stroke={AXIS_COLOR}
                    fontSize={12}
                    tickLine={false}
                    axisLine={false}
                    width={72}
                    tickFormatter={(value: number) => formatCurrency(value)}
                  />
                  <Tooltip content={<EvolutionTooltip />} cursor={{ fill: GRID_COLOR }} />
                  <Legend formatter={(value) => (value === "received" ? "Received" : "Receivable")} />
                  <Bar
                    dataKey="received"
                    name="received"
                    stackId="proventos"
                    fill={RECEIVED_COLOR}
                    radius={[0, 0, 0, 0]}
                  />
                  <Bar
                    dataKey="expected"
                    name="expected"
                    stackId="proventos"
                    fill={RECEIVED_COLOR}
                    fillOpacity={0.35}
                    stroke={RECEIVED_COLOR}
                    strokeWidth={1}
                    radius={[4, 4, 0, 0]}
                  />
                </BarChart>
              </ResponsiveContainer>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Monthly history</CardTitle>
        </CardHeader>
        <CardContent className="overflow-x-auto">
          {yearRows.length === 0 ? (
            <p className="text-muted-foreground">No dividend history yet.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Year</TableHead>
                  {MONTH_LABELS.map((label) => (
                    <TableHead key={label}>{label}</TableHead>
                  ))}
                  <TableHead>Average</TableHead>
                  <TableHead>Total</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {yearRows.map((row) => (
                  <TableRow key={row.year}>
                    <TableCell className="font-medium">{row.year}</TableCell>
                    {row.months.map((value, index) => (
                      <TableCell key={index}>{value != null ? formatCurrency(value) : "—"}</TableCell>
                    ))}
                    <TableCell>{formatCurrency(row.average)}</TableCell>
                    <TableCell className="font-medium">{formatCurrency(row.total)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>My dividends</CardTitle>
        </CardHeader>
        <CardContent className="overflow-x-auto">
          {summary.rows.length === 0 ? (
            <p className="text-muted-foreground">
              No dividends logged yet — log a "provento" transaction or confirm a suggestion in
              "Dividend suggestions".
            </p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Asset</TableHead>
                  <TableHead>Asset type</TableHead>
                  <TableHead>Payment status</TableHead>
                  <TableHead>Payment type</TableHead>
                  <TableHead>Data Com</TableHead>
                  <TableHead>Payment date</TableHead>
                  <TableHead>Quantity</TableHead>
                  <TableHead>Amount/share</TableHead>
                  <TableHead>Total value</TableHead>
                  <TableHead>Net total</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {summary.rows.map((row: ProventoRow, index) => (
                  <TableRow key={`${row.asset_id}-${row.payment_date}-${index}`}>
                    <TableCell>
                      <p className="font-medium">{row.ticker}</p>
                      <p className="text-xs text-muted-foreground">{row.name}</p>
                    </TableCell>
                    <TableCell>{ASSET_CLASS_LABELS[row.asset_class as AssetClass] ?? row.asset_class}</TableCell>
                    <TableCell>
                      <Badge variant={row.status === "paid" ? "default" : "outline"}>
                        {row.status === "paid" ? "Paid" : "Receivable"}
                      </Badge>
                    </TableCell>
                    <TableCell>{PAYMENT_TYPE_LABELS[row.payment_type as PaymentType] ?? row.payment_type}</TableCell>
                    <TableCell>{formatDate(row.com_date)}</TableCell>
                    <TableCell>{formatDate(row.payment_date)}</TableCell>
                    <TableCell>{row.quantity}</TableCell>
                    <TableCell>
                      {row.amount_per_share.toFixed(3)}
                      {row.native_currency !== summary.base_currency && (
                        <span className="ml-1 text-xs text-muted-foreground">{row.native_currency}</span>
                      )}
                    </TableCell>
                    <TableCell>
                      {row.fx_missing ? "—" : formatCurrency(row.total_value)}
                      {row.native_currency !== summary.base_currency && (
                        <span className="block text-xs text-muted-foreground" data-testid="dividend-native">
                          {row.fx_missing
                            ? `no ${row.native_currency}/${summary.base_currency} rate`
                            : `${formatCurrency(row.native_total_value)} ${row.native_currency}`}
                        </span>
                      )}
                    </TableCell>
                    <TableCell className="font-medium">
                      {row.fx_missing ? "—" : formatCurrency(row.net_total)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <Dialog open={suggestionsOpen} onOpenChange={setSuggestionsOpen}>
        <DialogContent className="max-h-[90vh] max-w-4xl overflow-y-auto">
          <DialogHeader>
            <DialogTitle>Dividend suggestions</DialogTitle>
          </DialogHeader>
          <DividendSuggestionsSection portfolioId={portfolioId} />
        </DialogContent>
      </Dialog>
    </div>
  );
}

export default DividendsSummarySection;
