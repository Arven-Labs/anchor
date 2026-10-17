import { useMemo, useState } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { Button } from "@/components/ui/button";
import type { StockPriceHistory } from "../collector/types";

// Mesma paleta de índice/cor já usada em ProfitabilitySection.tsx/
// DividendHistoryChart.tsx (rampa de verde do tema, --chart-*) — casa
// visualmente com o resto do app sem precisar de constantes compartilhadas
// novas (nenhum outro gráfico do projeto importa de um arquivo comum).
const LINE_COLOR = "#22c55e"; // --chart-3
const GRID_COLOR = "#1f2630"; // --border
const AXIS_COLOR = "#9fb1c2"; // --muted-foreground
const TOOLTIP_BG = "#111820"; // --card

type PeriodKey = "30d" | "12m" | "ytd" | "5y" | "10y" | "all";

const PERIODS: { key: PeriodKey; label: string }[] = [
  { key: "30d", label: "30D" },
  { key: "12m", label: "12M" },
  { key: "ytd", label: "YTD" },
  { key: "5y", label: "5Y" },
  { key: "10y", label: "10Y" },
  { key: "all", label: "All" },
];

const PERIOD_DAYS: Record<Exclude<PeriodKey, "ytd" | "all">, number> = {
  "30d": 30,
  "12m": 365,
  "5y": 365 * 5,
  "10y": 365 * 10,
};

// Cuts the (already-fetched, already-sorted-ascending) series down to the
// selected window — pure client-side slicing, no refetch. A period longer
// than the available history (e.g. "10Y" on a coin with only 365 days,
// CoinGecko's cap) just returns everything there is, same as "All".
function filterByPeriod(history: StockPriceHistory[], period: PeriodKey): StockPriceHistory[] {
  if (period === "all" || history.length === 0) return history;

  const latest = new Date(history[history.length - 1].price_date);
  const cutoff =
    period === "ytd"
      ? new Date(new Date().getFullYear(), 0, 1)
      : new Date(latest.getTime() - PERIOD_DAYS[period] * 86_400_000);

  return history.filter((point) => new Date(point.price_date) >= cutoff);
}

function formatDateLabel(dateStr: string): string {
  const [year, month, day] = dateStr.split("-");
  return `${day}/${month}/${year.slice(2)}`;
}

function PriceTooltip({
  active,
  payload,
  currencyPrefix,
  decimals,
}: {
  active?: boolean;
  payload?: { payload: StockPriceHistory }[];
  currencyPrefix: string;
  decimals: number;
}) {
  if (!active || !payload?.length) return null;
  const point = payload[0].payload;
  return (
    <div
      className="rounded-lg border border-border px-3 py-2 text-sm"
      style={{ background: TOOLTIP_BG }}
    >
      <p className="font-medium">{formatDateLabel(point.price_date)}</p>
      <p className="text-muted-foreground">
        {currencyPrefix} {point.close_price.toFixed(decimals)}
      </p>
    </div>
  );
}

/// Fase 10, item 8, Sessão 52 — pedido explícito do dono do projeto: "um
/// gráfico completo e simples" de preço, o mesmo em toda classe de ativo
/// (Ação/FII/ETF/Cripto hoje, qualquer classe futura com auto-quote). De
/// propósito **sem** os recursos de um gráfico tipo TradingView (zoom livre,
/// candlestick, indicadores sobrepostos, volume) — uma linha só. O único
/// controle é o seletor de período abaixo (mesmo pedido, mesma sessão,
/// lista exata: 30D/12M/YTD/5Y/10Y/All) — recorte client-side em cima do
/// `history` já buscado, sem requisição nova por período. Recebe `history`
/// já buscado (mesmo comando genérico `list_stock_price_history` que toda
/// classe auto-quote grava, ver PHASE.md item 8) — não faz fetch próprio,
/// cada seção decide quando buscar. `decimals` só existe pro câmbio, onde uma
/// cotação como 1,0832 não cabe nas 0 casas do eixo nem nas 2 do tooltip.
function PriceHistoryChart({
  history,
  currencyPrefix,
  decimals,
}: {
  history: StockPriceHistory[];
  currencyPrefix: string;
  decimals?: number;
}) {
  const [period, setPeriod] = useState<PeriodKey>("12m");
  const filtered = useMemo(() => filterByPeriod(history, period), [history, period]);

  if (history.length === 0) {
    return <p className="text-muted-foreground">No price history yet.</p>;
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex gap-1">
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

      <ResponsiveContainer width="100%" height={240}>
        <LineChart data={filtered} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid stroke={GRID_COLOR} vertical={false} />
          <XAxis
            dataKey="price_date"
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
            width={56}
            domain={["auto", "auto"]}
            tickFormatter={(value: number) => value.toFixed(decimals ?? 0)}
          />
          <Tooltip
            content={<PriceTooltip currencyPrefix={currencyPrefix} decimals={decimals ?? 2} />}
            cursor={{ stroke: GRID_COLOR }}
          />
          <Line
            type="monotone"
            dataKey="close_price"
            stroke={LINE_COLOR}
            strokeWidth={2}
            dot={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}

export default PriceHistoryChart;
