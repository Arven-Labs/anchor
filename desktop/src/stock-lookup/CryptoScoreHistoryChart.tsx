import { useMemo, useState } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { Button } from "@/components/ui/button";

// Same palette as PriceHistoryChart.tsx / BacktestSection.tsx: the project has
// no shared chart-constants file on purpose.
const LINE_COLOR = "#22c55e"; // --chart-3
const GRID_COLOR = "#1f2630"; // --border
const AXIS_COLOR = "#9fb1c2"; // --muted-foreground
const TOOLTIP_BG = "#111820"; // --card
const BAND_COLOR = "#3a4553";

// Mirrors `domain::crypto_score::{VERDICT_INTACT_MIN_PCT, VERDICT_NEUTRAL_MIN_PCT}`.
const INTACT_MIN_PCT = 70;
const NEUTRAL_MIN_PCT = 40;

/** One day of the score, from `list_crypto_score_history`. */
export type ScoreHistoryPoint = {
  date: string;
  green: number;
  logged: number;
  total: number;
  verdict: "intact" | "neutral" | "reduce_risk";
};

const VERDICT_SHORT: Record<ScoreHistoryPoint["verdict"], string> = {
  intact: "Thesis intact",
  neutral: "Neutral",
  reduce_risk: "Reduce risk",
};

type ChartPoint = ScoreHistoryPoint & { greenPct: number };

type PeriodKey = "30d" | "90d" | "12m" | "all";

const PERIODS: { key: PeriodKey; label: string }[] = [
  { key: "30d", label: "30D" },
  { key: "90d", label: "90D" },
  { key: "12m", label: "12M" },
  { key: "all", label: "All" },
];

const PERIOD_DAYS: Record<Exclude<PeriodKey, "all">, number> = {
  "30d": 30,
  "90d": 90,
  "12m": 365,
};

function daysBetween(from: string, to: string): number {
  return (new Date(to).getTime() - new Date(from).getTime()) / 86_400_000;
}

// Client-side cut of the already-fetched series, counted back from the latest
// reading date (not from today, so an old coin does not come out empty). A
// window with fewer than two dates cannot draw a line, so it shows everything.
function filterByPeriod(history: ScoreHistoryPoint[], period: PeriodKey): ScoreHistoryPoint[] {
  if (period === "all" || history.length === 0) return history;
  const latest = history[history.length - 1].date;
  const cut = history.filter((point) => daysBetween(point.date, latest) <= PERIOD_DAYS[period]);
  return cut.length < 2 ? history : cut;
}

function formatDateLabel(dateStr: string): string {
  const [year, month, day] = dateStr.split("-");
  return `${day}/${month}/${year.slice(2)}`;
}

function HistoryTooltip({ active, payload }: { active?: boolean; payload?: { payload: ChartPoint }[] }) {
  if (!active || !payload?.length) return null;
  const point = payload[0].payload;
  return (
    <div className="rounded-lg border border-border px-3 py-2 text-sm" style={{ background: TOOLTIP_BG }}>
      <p className="font-medium">{formatDateLabel(point.date)}</p>
      <p className="text-muted-foreground">
        {point.green}/{point.total} green, {point.logged} of {point.total} logged
      </p>
      <p className="text-muted-foreground">{VERDICT_SHORT[point.verdict]}</p>
    </div>
  );
}

/// Fase 31 — how the cycle-top score evolved: one point per reading date, the
/// share of green indicators over the coin's total (the same base as the
/// verdict, so the 40% and 70% lines are the verdict bands). The backend
/// recomputes each day from the latest reading of every indicator up to it
/// (`score_history`), using today's thresholds. Receives `history` already
/// fetched, like `PriceHistoryChart` — no fetch of its own.
function CryptoScoreHistoryChart({ history }: { history: ScoreHistoryPoint[] }) {
  const [period, setPeriod] = useState<PeriodKey>("all");
  const visible = useMemo(() => filterByPeriod(history, period), [history, period]);

  if (history.length < 2) {
    return (
      <p className="mb-4 text-sm text-muted-foreground" data-testid="crypto-score-history-empty">
        The score history starts to build with each reading date — it needs at least two.
      </p>
    );
  }

  const data: ChartPoint[] = visible.map((point) => ({
    ...point,
    greenPct: point.total === 0 ? 0 : (point.green / point.total) * 100,
  }));
  const latest = history[history.length - 1];

  return (
    <div className="mb-4 flex flex-col gap-2" data-testid="crypto-score-history">
      <p className="text-sm font-medium" data-testid="crypto-score-history-summary">
        Score history: {visible.length} dates, latest {latest.green}/{latest.total} green on{" "}
        {formatDateLabel(latest.date)}
      </p>
      {/* Só faz sentido quando a série passa do menor recorte. */}
      {daysBetween(history[0].date, latest.date) > PERIOD_DAYS["30d"] && (
        <div className="flex gap-1" data-testid="crypto-score-history-periods">
          {PERIODS.map(({ key, label }) => (
            <Button
              key={key}
              type="button"
              size="sm"
              variant={period === key ? "default" : "outline"}
              onClick={() => setPeriod(key)}
            >
              {label}
            </Button>
          ))}
        </div>
      )}
      <p className="text-xs text-muted-foreground">
        Share of green indicators on each reading date. Past dates use the current thresholds, so editing
        one rewrites the whole line.
      </p>
      <ResponsiveContainer width="100%" height={220}>
        <LineChart data={data} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid stroke={GRID_COLOR} vertical={false} />
          <XAxis
            dataKey="date"
            stroke={AXIS_COLOR}
            fontSize={12}
            tickLine={false}
            tickFormatter={formatDateLabel}
            minTickGap={48}
          />
          <YAxis
            stroke={AXIS_COLOR}
            fontSize={12}
            tickLine={false}
            axisLine={false}
            width={44}
            domain={[0, 100]}
            ticks={[0, 25, 50, 75, 100]}
            tickFormatter={(value: number) => `${value}%`}
          />
          <ReferenceLine
            y={INTACT_MIN_PCT}
            stroke={BAND_COLOR}
            strokeDasharray="4 4"
            label={{ value: "Intact 70%", position: "insideTopRight", fill: AXIS_COLOR, fontSize: 11 }}
          />
          <ReferenceLine
            y={NEUTRAL_MIN_PCT}
            stroke={BAND_COLOR}
            strokeDasharray="4 4"
            label={{ value: "Neutral 40%", position: "insideTopRight", fill: AXIS_COLOR, fontSize: 11 }}
          />
          <Tooltip content={<HistoryTooltip />} cursor={{ stroke: GRID_COLOR }} />
          <Line
            type="monotone"
            dataKey="greenPct"
            stroke={LINE_COLOR}
            strokeWidth={2}
            dot={{ r: 3, fill: LINE_COLOR }}
            // Sem animação: com poucos pontos ela dura mais que a espera do E2E, e a
            // captura saía com a linha pela metade.
            isAnimationActive={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}

export default CryptoScoreHistoryChart;
