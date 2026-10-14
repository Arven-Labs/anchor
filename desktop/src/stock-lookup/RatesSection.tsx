import { useMemo, useState } from "react";
import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ReferenceDot,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { Button } from "@/components/ui/button";
import KeyRatesTable from "../rates/KeyRatesTable";
import { formatDate, formatRate, useInterestRates } from "../rates/useInterestRates";

// Fase 17 — tela "Interest rates" do Research (sem ticker): Meta Selic
// (decisões do Copom, gráfico em degrau) e curva DI×PRE da B3 com os
// prazos-chave. Mesmo laranja do grupo "juros/macro" já validado pro CDI na
// Rentabilidade (Sessão 83, skill dataviz) — nenhum matiz novo. Curva de
// hoje vs. 1 mês atrás = mesma entidade em dois momentos: mesmo matiz,
// diferenciada por traço + legenda (nunca só por cor).
const RATE_COLOR = "#d95926";
const GRID_COLOR = "#1f2630"; // --border
const AXIS_COLOR = "#9fb1c2"; // --muted-foreground
const TOOLTIP_BG = "#111820"; // --card
const SURFACE = "#0b1016"; // anel em volta dos marcadores sobre a linha

type Range = "1y" | "5y" | "all";
const RANGES: { key: Range; label: string; years: number | null }[] = [
  { key: "1y", label: "1Y", years: 1 },
  { key: "5y", label: "5Y", years: 5 },
  { key: "all", label: "All", years: null },
];

const DAY_MS = 24 * 60 * 60 * 1000;

function toTime(date: string): number {
  return new Date(`${date.slice(0, 10)}T00:00:00`).getTime();
}

function formatTimeTick(t: number, spanYears: number): string {
  const d = new Date(t);
  const month = String(d.getMonth() + 1).padStart(2, "0");
  return spanYears <= 2 ? `${month}/${String(d.getFullYear()).slice(2)}` : String(d.getFullYear());
}

function SelicTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { payload: { t: number; rate: number } }[];
}) {
  if (!active || !payload?.length) return null;
  const { t, rate } = payload[0].payload;
  return (
    <div className="rounded-lg border border-border px-3 py-2 text-sm" style={{ background: TOOLTIP_BG }}>
      <p className="text-muted-foreground">{new Date(t).toLocaleDateString("pt-BR")}</p>
      <p className="font-medium">Selic target {formatRate(rate)}</p>
    </div>
  );
}

function CurveTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { name: string; payload: { years: number; rate_pct: number } }[];
}) {
  if (!active || !payload?.length) return null;
  return (
    <div className="rounded-lg border border-border px-3 py-2 text-sm" style={{ background: TOOLTIP_BG }}>
      {payload.map((p) => (
        <p key={p.name}>
          <span className="text-muted-foreground">{p.name}: </span>
          <span className="font-medium">
            {p.payload.years.toFixed(1)}y → {formatRate(p.payload.rate_pct)}
          </span>
        </p>
      ))}
    </div>
  );
}

function SelicChart({
  data,
  range,
  onRangeChange,
  spanYears,
}: {
  data: { t: number; rate: number }[];
  range: Range;
  onRangeChange: (range: Range) => void;
  spanYears: number;
}) {
  return (
    <>
      <div className="flex gap-1">
        {RANGES.map((r) => (
          <Button
            key={r.key}
            size="sm"
            variant={range === r.key ? "secondary" : "ghost"}
            onClick={() => onRangeChange(r.key)}
          >
            {r.label}
          </Button>
        ))}
      </div>
      <ResponsiveContainer width="100%" height={240}>
        <LineChart data={data} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid stroke={GRID_COLOR} vertical={false} />
          <XAxis
            dataKey="t"
            type="number"
            scale="time"
            domain={["dataMin", "dataMax"]}
            tickFormatter={(t) => formatTimeTick(t, spanYears)}
            stroke={AXIS_COLOR}
            fontSize={12}
            tickLine={false}
          />
          <YAxis
            stroke={AXIS_COLOR}
            fontSize={12}
            tickLine={false}
            axisLine={false}
            width={44}
            tickFormatter={(v) => `${v}%`}
            domain={["auto", "auto"]}
          />
          <Tooltip content={<SelicTooltip />} cursor={{ stroke: GRID_COLOR }} />
          <Line
            type="stepAfter"
            dataKey="rate"
            stroke={RATE_COLOR}
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 4, stroke: SURFACE, strokeWidth: 2 }}
            isAnimationActive={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </>
  );
}

export default function RatesSection() {
  const ratesQuery = useInterestRates();
  const [range, setRange] = useState<Range>("5y");

  const selicData = useMemo(() => {
    const selic = ratesQuery.data?.selic;
    if (!selic) return [];
    const points = selic.changes.map((c) => ({ t: toTime(c.date), rate: c.rate_pct }));
    // Estende o último degrau até hoje.
    points.push({ t: Date.now(), rate: selic.current_pct });
    const years = RANGES.find((r) => r.key === range)?.years;
    if (years == null) return points;
    const start = Date.now() - years * 365 * DAY_MS;
    // Mantém o degrau vigente no início da janela.
    const before = points.filter((p) => p.t < start).pop();
    const inside = points.filter((p) => p.t >= start);
    return before ? [{ t: start, rate: before.rate }, ...inside] : inside;
  }, [ratesQuery.data, range]);

  const spanYears = RANGES.find((r) => r.key === range)?.years ?? 30;

  if (ratesQuery.isLoading) return <p className="text-sm">Loading interest rates…</p>;
  if (ratesQuery.isError) {
    return (
      <p className="text-sm text-red-600">
        Couldn't load interest rates from the Finance API ({ratesQuery.error.message}).
      </p>
    );
  }
  const data = ratesQuery.data;
  if (!data) return null;
  const { selic, di } = data;

  return (
    <div className="flex flex-col gap-8">
      <section className="flex flex-col gap-3">
        <div>
          <h3 className="text-lg font-semibold">Selic target rate</h3>
          {selic ? (
            <p className="text-sm text-muted-foreground">
              Set by the Central Bank's Copom. Currently{" "}
              <span className="font-medium text-foreground">{formatRate(selic.current_pct)}</span>{" "}
              per year, since {formatDate(selic.since)}.
            </p>
          ) : (
            <p className="text-sm text-muted-foreground">
              Unavailable right now — the Central Bank's service didn't answer ({data.selic_error}).
              Try again in a few minutes.
            </p>
          )}
        </div>
        {selic && (
          <SelicChart data={selicData} range={range} onRangeChange={setRange} spanYears={spanYears} />
        )}
      </section>

      <section className="flex flex-col gap-3">
        <div>
          <h3 className="text-lg font-semibold">DI futures curve</h3>
          <p className="text-sm text-muted-foreground">
            What the market prices for interest rates at each term (B3, DI × fixed rate). A curve
            rising with the term means the market expects higher rates ahead.
          </p>
        </div>
        {!di ? (
          <p className="text-sm text-muted-foreground">
            {data.di_error
              ? `Unavailable right now (${data.di_error}).`
              : "No DI curve published in the last days."}
          </p>
        ) : (
          <>
            <ResponsiveContainer width="100%" height={280}>
              <LineChart margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
                <CartesianGrid stroke={GRID_COLOR} vertical={false} />
                <XAxis
                  dataKey="years"
                  type="number"
                  domain={[0, "dataMax"]}
                  tickFormatter={(v) => `${v}y`}
                  stroke={AXIS_COLOR}
                  fontSize={12}
                  tickLine={false}
                  allowDuplicatedCategory={false}
                />
                <YAxis
                  stroke={AXIS_COLOR}
                  fontSize={12}
                  tickLine={false}
                  axisLine={false}
                  width={44}
                  tickFormatter={(v) => `${v}%`}
                  domain={["auto", "auto"]}
                />
                <Tooltip content={<CurveTooltip />} cursor={{ stroke: GRID_COLOR }} />
                <Legend />
                <Line
                  data={di.vertices}
                  dataKey="rate_pct"
                  name={formatDate(di.reference_date)}
                  stroke={RATE_COLOR}
                  strokeWidth={2}
                  dot={false}
                  activeDot={{ r: 4, stroke: SURFACE, strokeWidth: 2 }}
                  isAnimationActive={false}
                />
                {di.month_ago_vertices.length > 0 && di.month_ago_reference_date && (
                  <Line
                    data={di.month_ago_vertices}
                    dataKey="rate_pct"
                    name={formatDate(di.month_ago_reference_date)}
                    stroke={RATE_COLOR}
                    strokeOpacity={0.6}
                    strokeWidth={2}
                    strokeDasharray="4 3"
                    dot={false}
                    activeDot={{ r: 4, stroke: SURFACE, strokeWidth: 2 }}
                    isAnimationActive={false}
                  />
                )}
                {di.key_rates
                  .filter((k) => k.rate_pct != null)
                  .map((k) => (
                    <ReferenceDot
                      key={k.years}
                      x={k.years}
                      y={k.rate_pct as number}
                      r={4}
                      fill={RATE_COLOR}
                      stroke={SURFACE}
                      strokeWidth={2}
                    />
                  ))}
              </LineChart>
            </ResponsiveContainer>
            <p className="text-xs text-muted-foreground">
              Dots mark the key terms below. Rates between published maturities are linearly
              interpolated.
            </p>
            <KeyRatesTable di={di} />
          </>
        )}
      </section>
    </div>
  );
}
