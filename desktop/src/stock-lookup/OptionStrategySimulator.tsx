import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ReferenceDot,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { AppError } from "../types";
import { StatTile } from "./shared";
import { formatCurrency } from "./shared";
import type { StrikeRow } from "./optionTypes";
import type { Portfolio } from "../portfolio/types";
import Field from "../components/Field";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

// Fase 18, 2ª fatia (Sessão 115) — simulador do resultado de uma estratégia
// com opções no vencimento. O cálculo é do backend (`simulate_option_strategy`,
// `domain::option_payoff`); aqui ficam o editor de pernas, os atalhos prontos
// e o gráfico.
//
// Cores: par divergente azul (ganho) e vermelho (perda) nos passos escuros da
// paleta de referência da skill `dataviz` (slots 1 e 8), validado contra a
// superfície do card (`#111820`): ΔE normal 29,0, CVD 19,2, contraste >= 3:1.
// A linha do resultado é neutra; ganho e perda aparecem nas áreas e na legenda,
// nunca só pela cor (os tiles dizem "Max gain"/"Max loss" por extenso).
const GAIN_COLOR = "#3987e5";
const LOSS_COLOR = "#e66767";
const LINE_COLOR = "#e6edf3"; // --foreground
const GRID_COLOR = "#1f2630"; // --border
const AXIS_COLOR = "#9fb1c2"; // --muted-foreground
const TOOLTIP_BG = "#111820"; // --card
const ZERO_COLOR = "#3a4553";
const SURFACE = "#0b1016"; // anel em volta dos marcadores

const DEFAULT_QUANTITY = "100"; // 1 lote padrão da B3

export type LegKind = "stock" | "call" | "put";
export type LegSide = "long" | "short";

/** Perna como o formulário a guarda: números ainda em texto. */
export type LegDraft = {
  id: number;
  kind: LegKind;
  side: LegSide;
  strike: string;
  price: string;
  quantity: string;
  /** Série da B3 e vencimento, quando a perna veio da cadeia (pra salvar como posição). */
  seriesTicker?: string;
  expiration?: string;
};

type PayoffPoint = { price: number; pnl: number };

type StrategyResult = {
  points: PayoffPoint[];
  breakevens: number[];
  max_gain: number | null;
  max_loss: number | null;
  unlimited_gain: boolean;
  unlimited_loss: boolean;
  net_debit: number;
};

let nextLegId = 1;

export function newLeg(partial: Partial<Omit<LegDraft, "id">> = {}): LegDraft {
  return {
    id: nextLegId++,
    kind: "call",
    side: "long",
    strike: "",
    price: "",
    quantity: DEFAULT_QUANTITY,
    ...partial,
  };
}

/** Texto de um preço vindo da cadeia: série sem negócio fica vazia, pra ser preenchida. */
function priceText(price: number | null): string {
  return price == null ? "" : price.toFixed(2);
}

const KIND_LABEL: Record<LegKind, string> = { stock: "Stock", call: "Call", put: "Put" };
const SIDE_LABEL: Record<LegSide, string> = { long: "Buy", short: "Sell" };

// ---------------------------------------------------------------------------
// Atalhos prontos
// ---------------------------------------------------------------------------

export type PresetKey =
  | "covered_call"
  | "protective_put"
  | "bull_call_spread"
  | "bear_put_spread"
  | "long_straddle"
  | "cash_secured_put";

const PRESETS: { key: PresetKey; label: string }[] = [
  { key: "covered_call", label: "Covered call" },
  { key: "protective_put", label: "Protective put" },
  { key: "bull_call_spread", label: "Bull call spread" },
  { key: "bear_put_spread", label: "Bear put spread" },
  { key: "long_straddle", label: "Long straddle" },
  { key: "cash_secured_put", label: "Cash-secured put" },
];

type PresetResult = { legs: LegDraft[]; unpriced: boolean } | { error: string };

function optionLeg(
  kind: "call" | "put",
  side: LegSide,
  row: StrikeRow,
  expiration: string | null,
): { draft: LegDraft; unpriced: boolean } {
  const series = kind === "call" ? row.call : row.put;
  const price = series?.last_price ?? null;
  return {
    draft: newLeg({
      kind,
      side,
      strike: row.strike.toFixed(2),
      price: priceText(price),
      seriesTicker: series?.series_ticker,
      expiration: expiration ?? undefined,
    }),
    unpriced: price == null,
  };
}

/**
 * Monta as pernas de um atalho a partir das linhas (strikes) do vencimento e do
 * preço atual da ação. Escolhe os strikes vizinhos ao preço; devolve `unpriced`
 * quando alguma perna não tem preço de negócio (o prêmio fica vazio).
 */
export function buildPreset(
  key: PresetKey,
  spot: number | null,
  rows: StrikeRow[],
  expiration: string | null,
): PresetResult {
  if (spot == null) return { error: "The stock price is not available yet." };
  const withCall = rows.filter((r) => r.call);
  const withPut = rows.filter((r) => r.put);
  const nearest = (list: StrikeRow[]) =>
    list.reduce<StrikeRow | null>(
      (best, r) => (best == null || Math.abs(r.strike - spot) < Math.abs(best.strike - spot) ? r : best),
      null,
    );
  const sorted = (list: StrikeRow[]) => [...list].sort((a, b) => a.strike - b.strike);
  const missing = { error: "This expiry has no suitable strikes for that strategy." };
  const stock = (): LegDraft => newLeg({ kind: "stock", side: "long", price: spot.toFixed(2) });

  switch (key) {
    case "covered_call": {
      const row = sorted(withCall).find((r) => r.strike >= spot);
      if (!row) return missing;
      const short = optionLeg("call", "short", row, expiration);
      return { legs: [stock(), short.draft], unpriced: short.unpriced };
    }
    case "protective_put": {
      const row = sorted(withPut).reverse().find((r) => r.strike <= spot);
      if (!row) return missing;
      const long = optionLeg("put", "long", row, expiration);
      return { legs: [stock(), long.draft], unpriced: long.unpriced };
    }
    case "bull_call_spread": {
      const atm = nearest(withCall);
      const upper = atm && sorted(withCall).find((r) => r.strike > atm.strike);
      if (!atm || !upper) return missing;
      const [buy, sell] = [
        optionLeg("call", "long", atm, expiration),
        optionLeg("call", "short", upper, expiration),
      ];
      return { legs: [buy.draft, sell.draft], unpriced: buy.unpriced || sell.unpriced };
    }
    case "bear_put_spread": {
      const atm = nearest(withPut);
      const lower = atm && sorted(withPut).reverse().find((r) => r.strike < atm.strike);
      if (!atm || !lower) return missing;
      const [buy, sell] = [
        optionLeg("put", "long", atm, expiration),
        optionLeg("put", "short", lower, expiration),
      ];
      return { legs: [buy.draft, sell.draft], unpriced: buy.unpriced || sell.unpriced };
    }
    case "long_straddle": {
      const row = nearest(rows.filter((r) => r.call && r.put));
      if (!row) return missing;
      const [call, put] = [
        optionLeg("call", "long", row, expiration),
        optionLeg("put", "long", row, expiration),
      ];
      return { legs: [call.draft, put.draft], unpriced: call.unpriced || put.unpriced };
    }
    case "cash_secured_put": {
      const row = sorted(withPut).reverse().find((r) => r.strike <= spot);
      if (!row) return missing;
      const short = optionLeg("put", "short", row, expiration);
      return { legs: [short.draft], unpriced: short.unpriced };
    }
  }
}

/** Perna de uma série escolhida na cadeia, comprada ou vendida, com o último preço. */
export function legFromSeries(
  kind: "call" | "put",
  side: LegSide,
  strike: number,
  lastPrice: number | null,
  seriesTicker: string,
  expiration: string | null,
): LegDraft {
  return newLeg({
    kind,
    side,
    strike: strike.toFixed(2),
    price: priceText(lastPrice),
    seriesTicker,
    expiration: expiration ?? undefined,
  });
}

// ---------------------------------------------------------------------------
// Conversão e formatação
// ---------------------------------------------------------------------------

type RequestLeg = {
  kind: LegKind;
  side: LegSide;
  strike: number | null;
  price: number;
  quantity: number;
};

function toNumber(text: string): number | null {
  if (text.trim() === "") return null;
  const value = Number(text);
  return Number.isFinite(value) ? value : null;
}

/** `null` se alguma perna ainda está incompleta ou fora do domínio. */
function toRequestLegs(legs: LegDraft[]): RequestLeg[] | null {
  const out: RequestLeg[] = [];
  for (const leg of legs) {
    const price = toNumber(leg.price);
    const quantity = toNumber(leg.quantity);
    const strike = leg.kind === "stock" ? null : toNumber(leg.strike);
    if (price == null || price < 0 || quantity == null || quantity <= 0) return null;
    if (leg.kind !== "stock" && (strike == null || strike <= 0)) return null;
    out.push({ kind: leg.kind, side: leg.side, strike, price, quantity });
  }
  return out;
}

function netPremiumText(netDebit: number): string {
  if (Math.abs(netDebit) < 0.005) return "None";
  return netDebit > 0
    ? `${formatCurrency(netDebit)} debit`
    : `${formatCurrency(-netDebit)} credit`;
}

function maxGainText(result: StrategyResult): string {
  if (result.unlimited_gain) return "Unlimited";
  return result.max_gain == null || result.max_gain <= 0 ? "None" : formatCurrency(result.max_gain);
}

function maxLossText(result: StrategyResult): string {
  if (result.unlimited_loss) return "Unlimited";
  return result.max_loss == null || result.max_loss >= 0 ? "None" : formatCurrency(-result.max_loss);
}

function breakevenText(breakevens: number[]): string {
  return breakevens.length === 0 ? "—" : breakevens.map((b) => formatCurrency(b)).join(" · ");
}

// ---------------------------------------------------------------------------
// Gráfico
// ---------------------------------------------------------------------------

function PayoffTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { payload: PayoffPoint }[];
}) {
  if (!active || !payload?.length) return null;
  const { price, pnl } = payload[0].payload;
  return (
    <div className="rounded-lg border border-border px-3 py-2 text-sm" style={{ background: TOOLTIP_BG }}>
      <p className="text-muted-foreground">Stock at expiry {formatCurrency(price)}</p>
      <p className="font-medium">
        {pnl >= 0 ? "Profit" : "Loss"} {formatCurrency(Math.abs(pnl))}
      </p>
    </div>
  );
}

function PayoffChart({ result, spot }: { result: StrategyResult; spot: number | null }) {
  // Duas áreas a partir de zero (ganho e perda); a curva já traz os pontos
  // exatos dos equilíbrios, então elas não vazam pro outro lado.
  const data = useMemo(
    () =>
      result.points.map((p) => ({
        ...p,
        gain: Math.max(p.pnl, 0),
        loss: Math.min(p.pnl, 0),
      })),
    [result.points],
  );

  return (
    <div data-testid="payoff-chart">
      <div className="mb-2 flex flex-wrap items-center gap-4 text-xs text-muted-foreground">
        <span className="flex items-center gap-1.5">
          <span className="inline-block size-2.5 rounded-sm" style={{ background: GAIN_COLOR }} />
          Profit
        </span>
        <span className="flex items-center gap-1.5">
          <span className="inline-block size-2.5 rounded-sm" style={{ background: LOSS_COLOR }} />
          Loss
        </span>
        <span className="flex items-center gap-1.5">
          <span
            className="inline-block h-0 w-4 border-t-2 border-dashed"
            style={{ borderColor: AXIS_COLOR }}
          />
          Current price
        </span>
        <span className="flex items-center gap-1.5">
          <span
            className="inline-block size-2.5 rounded-full"
            style={{ background: LINE_COLOR, boxShadow: `0 0 0 2px ${SURFACE}` }}
          />
          Breakeven
        </span>
      </div>
      <ResponsiveContainer width="100%" height={260}>
        <ComposedChart data={data} margin={{ top: 8, right: 16, bottom: 0, left: 8 }}>
          <CartesianGrid stroke={GRID_COLOR} vertical={false} />
          <XAxis
            dataKey="price"
            type="number"
            domain={["dataMin", "dataMax"]}
            tick={{ fill: AXIS_COLOR, fontSize: 12 }}
            tickFormatter={(v: number) => v.toFixed(0)}
            stroke={GRID_COLOR}
          />
          <YAxis
            tick={{ fill: AXIS_COLOR, fontSize: 12 }}
            tickFormatter={(v: number) => v.toFixed(0)}
            stroke={GRID_COLOR}
            width={64}
          />
          <Tooltip content={<PayoffTooltip />} cursor={{ stroke: AXIS_COLOR, strokeDasharray: "3 3" }} />
          <ReferenceLine y={0} stroke={ZERO_COLOR} />
          {spot != null && (
            <ReferenceLine x={spot} stroke={AXIS_COLOR} strokeDasharray="4 4" strokeWidth={1.5} />
          )}
          <Area
            dataKey="gain"
            type="linear"
            baseValue={0}
            stroke="none"
            fill={GAIN_COLOR}
            fillOpacity={0.3}
            isAnimationActive={false}
            activeDot={false}
          />
          <Area
            dataKey="loss"
            type="linear"
            baseValue={0}
            stroke="none"
            fill={LOSS_COLOR}
            fillOpacity={0.3}
            isAnimationActive={false}
            activeDot={false}
          />
          <Line
            dataKey="pnl"
            type="linear"
            stroke={LINE_COLOR}
            strokeWidth={2}
            dot={false}
            isAnimationActive={false}
          />
          {result.breakevens.map((b) => (
            <ReferenceDot
              key={b}
              x={b}
              y={0}
              r={4}
              fill={LINE_COLOR}
              stroke={SURFACE}
              strokeWidth={2}
              ifOverflow="visible"
            />
          ))}
        </ComposedChart>
      </ResponsiveContainer>
    </div>
  );
}

/** Pontos da tabela alternativa ao gráfico: extremos, strikes e equilíbrios. */
function keyPoints(result: StrategyResult, legs: RequestLeg[]): PayoffPoint[] {
  const wanted = new Set<number>(legs.flatMap((l) => (l.strike == null ? [] : [l.strike])));
  result.breakevens.forEach((b) => wanted.add(b));
  const first = result.points[0];
  const last = result.points[result.points.length - 1];
  const near = (a: number, b: number) => Math.abs(a - b) < 1e-6;
  return result.points.filter(
    (p) => p === first || p === last || [...wanted].some((w) => near(w, p.price)),
  );
}

// ---------------------------------------------------------------------------
// Editor de pernas
// ---------------------------------------------------------------------------

function LegRow({
  index,
  leg,
  onChange,
  onRemove,
}: {
  index: number;
  leg: LegDraft;
  onChange: (patch: Partial<LegDraft>) => void;
  onRemove: () => void;
}) {
  const isStock = leg.kind === "stock";
  return (
    <div className="flex flex-wrap items-center gap-2" data-testid={`leg-${index}`}>
      <Select value={leg.side} onValueChange={(v) => onChange({ side: v as LegSide })}>
        <SelectTrigger size="sm" className="w-24" data-testid={`leg-side-${index}`}>
          <SelectValue>{SIDE_LABEL[leg.side]}</SelectValue>
        </SelectTrigger>
        <SelectContent align="start">
          <SelectItem value="long">Buy</SelectItem>
          <SelectItem value="short">Sell</SelectItem>
        </SelectContent>
      </Select>
      <Select
        value={leg.kind}
        onValueChange={(v) => onChange({ kind: v as LegKind, strike: v === "stock" ? "" : leg.strike })}
      >
        <SelectTrigger size="sm" className="w-24" data-testid={`leg-kind-${index}`}>
          <SelectValue>{KIND_LABEL[leg.kind]}</SelectValue>
        </SelectTrigger>
        <SelectContent align="start">
          <SelectItem value="stock">Stock</SelectItem>
          <SelectItem value="call">Call</SelectItem>
          <SelectItem value="put">Put</SelectItem>
        </SelectContent>
      </Select>
      <Input
        className="w-24"
        type="number"
        step="any"
        placeholder={isStock ? "—" : "Strike"}
        aria-label="Strike"
        disabled={isStock}
        value={isStock ? "" : leg.strike}
        data-testid={`leg-strike-${index}`}
        onChange={(e) => onChange({ strike: e.currentTarget.value })}
      />
      <Input
        className="w-24"
        type="number"
        step="any"
        placeholder={isStock ? "Entry price" : "Premium"}
        aria-label={isStock ? "Entry price" : "Premium"}
        value={leg.price}
        data-testid={`leg-price-${index}`}
        onChange={(e) => onChange({ price: e.currentTarget.value })}
      />
      <Input
        className="w-24"
        type="number"
        step="any"
        placeholder="Quantity"
        aria-label="Quantity"
        value={leg.quantity}
        data-testid={`leg-qty-${index}`}
        onChange={(e) => onChange({ quantity: e.currentTarget.value })}
      />
      <Button type="button" variant="ghost" size="sm" onClick={onRemove} aria-label="Remove leg">
        Remove
      </Button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Seção
// ---------------------------------------------------------------------------

/** Espera o usuário parar de digitar antes de recalcular (cada cálculo é uma chamada ao backend). */
function useDebounced<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(timer);
  }, [value, delayMs]);
  return debounced;
}

// ---------------------------------------------------------------------------
// Salvar a estratégia como posições (Sessão 119)
// ---------------------------------------------------------------------------

const DEFAULT_DAYS_TO_EXPIRY = 30;

function dateIso(offsetDays = 0): string {
  const date = new Date();
  date.setDate(date.getDate() + offsetDays);
  return date.toISOString().slice(0, 10);
}

/** Uma perna de opção no diálogo: série e vencimento editáveis, o resto vem do simulador. */
type SaveRow = {
  legId: number;
  label: string;
  strike: number;
  kind: "call" | "put";
  side: LegSide;
  series: string;
  expiration: string;
  premium: string;
  quantity: string;
};

function saveRowsFrom(legs: LegDraft[], defaultExpiration: string | null): SaveRow[] {
  return legs.flatMap((leg) => {
    if (leg.kind === "stock") return [];
    return [
      {
        legId: leg.id,
        label: `${SIDE_LABEL[leg.side]} ${KIND_LABEL[leg.kind].toLowerCase()}, strike ${Number(leg.strike).toFixed(2)}`,
        strike: Number(leg.strike),
        kind: leg.kind,
        side: leg.side,
        series: leg.seriesTicker ?? "",
        expiration: leg.expiration ?? defaultExpiration ?? dateIso(DEFAULT_DAYS_TO_EXPIRY),
        premium: leg.price,
        quantity: leg.quantity,
      },
    ];
  });
}

function SaveStrategyDialog({
  open,
  onOpenChange,
  workspaceId,
  underlying,
  legs,
  defaultExpiration,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  workspaceId: number;
  underlying: string;
  legs: LegDraft[];
  defaultExpiration: string | null;
}) {
  const queryClient = useQueryClient();
  const [rows, setRows] = useState<SaveRow[]>([]);
  const [portfolioId, setPortfolioId] = useState("");
  const [savedCount, setSavedCount] = useState<number | null>(null);

  const portfoliosQuery = useQuery<Portfolio[], AppError>({
    queryKey: ["portfolios", workspaceId],
    queryFn: () => invoke("list_portfolios", { workspaceId }),
    enabled: open,
  });
  const portfolios = portfoliosQuery.data ?? [];

  // Cada abertura parte das pernas atuais do simulador.
  useEffect(() => {
    if (open) {
      setRows(saveRowsFrom(legs, defaultExpiration));
      setSavedCount(null);
    }
    // As pernas só são lidas ao abrir; editar o simulador com o diálogo aberto não o reescreve.
  }, [open]);

  // Uma carteira só: já vem escolhida.
  useEffect(() => {
    if (portfolioId === "" && portfolios.length === 1) setPortfolioId(String(portfolios[0].id));
  }, [portfolios, portfolioId]);

  const stockLegs = legs.filter((leg) => leg.kind === "stock").length;

  const saveMutation = useMutation<unknown[], AppError, void>({
    mutationFn: () =>
      invoke("create_option_positions", {
        request: {
          portfolio_id: Number(portfolioId),
          underlying_ticker: underlying,
          open_date: dateIso(),
          legs: rows.map((row) => ({
            series_ticker: row.series,
            option_type: row.kind,
            strike: row.strike,
            expiration: row.expiration,
            side: row.side,
            quantity: Number(row.quantity),
            premium: Number(row.premium),
          })),
        },
      }),
    onSuccess: (created) => {
      setSavedCount(created.length);
      queryClient.invalidateQueries({ queryKey: ["option-positions"] });
      queryClient.invalidateQueries({ queryKey: ["net-worth"] });
    },
  });

  function patchRow(legId: number, patch: Partial<SaveRow>) {
    setRows((current) => current.map((row) => (row.legId === legId ? { ...row, ...patch } : row)));
  }

  const ready = portfolioId !== "" && rows.length > 0 && rows.every((r) => r.series.trim() !== "");

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-3xl" data-testid="save-strategy-dialog">
        <DialogHeader>
          <DialogTitle>Save as option positions</DialogTitle>
        </DialogHeader>
        {savedCount != null ? (
          <div className="flex flex-col gap-3">
            <p className="text-sm" data-testid="save-done">
              Saved {savedCount} position{savedCount === 1 ? "" : "s"}. They are in Portfolio →
              Options, where you can close them and track the result.
            </p>
            <Button
              type="button"
              className="w-fit"
              data-testid="save-close"
              onClick={() => onOpenChange(false)}
            >
              Done
            </Button>
          </div>
        ) : (
          <form
            className="flex flex-col gap-4"
            onSubmit={(e) => {
              e.preventDefault();
              saveMutation.mutate();
            }}
          >
            <p className="text-sm text-muted-foreground">
              Each option leg becomes its own position on {underlying}, opened today. Check the
              series ticker and expiry of every leg.
              {stockLegs > 0 &&
                ` The ${stockLegs} stock leg${stockLegs === 1 ? " is" : "s are"} not saved: positions only track options.`}
            </p>

            {portfoliosQuery.isSuccess && portfolios.length === 0 ? (
              <p className="text-sm text-red-600" data-testid="save-no-portfolio">
                This workspace has no portfolio yet. Create one in Portfolio first.
              </p>
            ) : (
              <Field label="Portfolio" className="max-w-xs">
                <Select value={portfolioId} onValueChange={setPortfolioId}>
                  <SelectTrigger data-testid="save-portfolio">
                    <SelectValue placeholder="Choose a portfolio">
                      {portfolios.find((p) => String(p.id) === portfolioId)?.name}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {portfolios.map((p) => (
                      <SelectItem key={p.id} value={String(p.id)}>
                        {p.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            )}

            <div className="flex flex-col gap-3">
              {rows.map((row, index) => (
                <div key={row.legId} className="rounded-lg border border-border p-3">
                  <p className="mb-2 text-sm font-medium">{row.label}</p>
                  <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                    <Field label="Series ticker">
                      <Input
                        required
                        placeholder="PETRK600"
                        value={row.series}
                        data-testid={`save-series-${index}`}
                        onChange={(e) => patchRow(row.legId, { series: e.currentTarget.value })}
                      />
                    </Field>
                    <Field label="Expiration">
                      <Input
                        required
                        type="date"
                        value={row.expiration}
                        data-testid={`save-expiration-${index}`}
                        onChange={(e) => patchRow(row.legId, { expiration: e.currentTarget.value })}
                      />
                    </Field>
                    <Field label="Premium per option">
                      <Input
                        required
                        type="number"
                        step="any"
                        value={row.premium}
                        data-testid={`save-premium-${index}`}
                        onChange={(e) => patchRow(row.legId, { premium: e.currentTarget.value })}
                      />
                    </Field>
                    <Field label="Quantity">
                      <Input
                        required
                        type="number"
                        step="any"
                        value={row.quantity}
                        data-testid={`save-quantity-${index}`}
                        onChange={(e) => patchRow(row.legId, { quantity: e.currentTarget.value })}
                      />
                    </Field>
                  </div>
                </div>
              ))}
            </div>

            {saveMutation.isError && <p className="text-sm text-red-600">{saveMutation.error.message}</p>}
            <Button type="submit" className="w-fit" data-testid="save-submit" disabled={!ready || saveMutation.isPending}>
              {saveMutation.isPending ? "Saving..." : `Save ${rows.length} position${rows.length === 1 ? "" : "s"}`}
            </Button>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}

function OptionStrategySimulator({
  spot,
  rows,
  expiration,
  workspaceId,
  underlying,
  legs,
  setLegs,
}: {
  /** Preço atual da ação. */
  spot: number | null;
  /** Strikes do vencimento selecionado; vazio enquanto a cadeia não foi carregada. */
  rows: StrikeRow[];
  /** Vencimento selecionado na cadeia (`YYYY-MM-DD`); nulo sem cadeia. */
  expiration: string | null;
  workspaceId: number;
  /** Ticker da ação analisada: o ativo-objeto das posições salvas. */
  underlying: string;
  legs: LegDraft[];
  setLegs: (update: (current: LegDraft[]) => LegDraft[]) => void;
}) {
  const [presetNote, setPresetNote] = useState<string | null>(null);
  const [showTable, setShowTable] = useState(false);
  const [saveOpen, setSaveOpen] = useState(false);

  const requestLegs = useMemo(() => toRequestLegs(legs), [legs]);
  const debouncedLegs = useDebounced(requestLegs, 350);

  const resultQuery = useQuery<StrategyResult, AppError>({
    queryKey: ["option-strategy", JSON.stringify(debouncedLegs), spot],
    queryFn: () =>
      invoke("simulate_option_strategy", { request: { spot, legs: debouncedLegs } }),
    enabled: debouncedLegs != null && debouncedLegs.length > 0,
    retry: false,
    staleTime: Infinity,
  });

  function addLeg() {
    const strike = spot != null ? String(Math.round(spot)) : "";
    setLegs((current) => [...current, newLeg({ strike })]);
  }

  function applyPreset(key: PresetKey) {
    const preset = buildPreset(key, spot, rows, expiration);
    if ("error" in preset) {
      setPresetNote(preset.error);
      return;
    }
    setLegs(() => preset.legs);
    setPresetNote(
      preset.unpriced
        ? "A leg has no traded price on the B3: edit its premium before trusting the result."
        : null,
    );
  }

  function patchLeg(id: number, patch: Partial<LegDraft>) {
    // Mudar o tipo ou o strike deixa de ser a série que veio da cadeia.
    const detach = "kind" in patch || "strike" in patch ? { seriesTicker: undefined } : {};
    setLegs((current) =>
      current.map((leg) => (leg.id === id ? { ...leg, ...patch, ...detach } : leg)),
    );
  }

  const hasOptionLeg = legs.some((leg) => leg.kind !== "stock");

  const result = resultQuery.data;
  const incomplete = legs.length > 0 && requestLegs == null;

  return (
    <Card data-testid="strategy-simulator">
      <CardHeader>
        <CardTitle>Strategy simulator</CardTitle>
        <CardDescription>
          Result at expiry for a combination of options and the stock. It ignores fees, taxes,
          dividends and early exercise. Quantities are in options (100 is one standard lot).
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center gap-2">
          {PRESETS.map((preset) => (
            <Button
              key={preset.key}
              type="button"
              variant="outline"
              size="sm"
              disabled={rows.length === 0}
              data-testid={`preset-${preset.key}`}
              onClick={() => applyPreset(preset.key)}
            >
              {preset.label}
            </Button>
          ))}
        </div>
        {rows.length === 0 && (
          <p className="text-xs text-muted-foreground">
            Load the options above to use the ready-made strategies, or add legs by hand.
          </p>
        )}
        {presetNote && <p className="text-sm text-yellow-600 dark:text-yellow-400">{presetNote}</p>}

        <div className="flex flex-col gap-2">
          {legs.map((leg, index) => (
            <LegRow
              key={leg.id}
              index={index}
              leg={leg}
              onChange={(patch) => patchLeg(leg.id, patch)}
              onRemove={() => setLegs((current) => current.filter((l) => l.id !== leg.id))}
            />
          ))}
          <div className="flex gap-2">
            <Button type="button" variant="outline" size="sm" data-testid="add-leg" onClick={addLeg}>
              Add leg
            </Button>
            {legs.length > 0 && (
              <Button type="button" variant="ghost" size="sm" onClick={() => setLegs(() => [])}>
                Clear
              </Button>
            )}
            {hasOptionLeg && !incomplete && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                data-testid="save-strategy"
                onClick={() => setSaveOpen(true)}
              >
                Save as positions
              </Button>
            )}
          </div>
        </div>

        <SaveStrategyDialog
          open={saveOpen}
          onOpenChange={setSaveOpen}
          workspaceId={workspaceId}
          underlying={underlying}
          legs={legs}
          defaultExpiration={expiration}
        />

        {legs.length === 0 && (
          <p className="text-sm text-muted-foreground">Add a leg to see the result at expiry.</p>
        )}
        {incomplete && (
          <p className="text-sm text-muted-foreground">
            Fill in the strike, price and quantity of every leg.
          </p>
        )}
        {resultQuery.isError && !incomplete && (
          <p className="text-sm text-red-600">{resultQuery.error.message}</p>
        )}

        {result && !incomplete && debouncedLegs && (
          <>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4" data-testid="strategy-result">
              <StatTile label="Net premium" value={netPremiumText(result.net_debit)} />
              <StatTile label="Max gain" value={maxGainText(result)} />
              <StatTile label="Max loss" value={maxLossText(result)} />
              <StatTile label="Breakeven" value={breakevenText(result.breakevens)} />
            </div>

            <PayoffChart result={result} spot={spot} />

            <div>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => setShowTable((v) => !v)}
              >
                {showTable ? "Hide values" : "Show values"}
              </Button>
              {showTable && (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Stock at expiry</TableHead>
                      <TableHead>Result</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {keyPoints(result, debouncedLegs).map((p) => (
                      <TableRow key={p.price}>
                        <TableCell>{formatCurrency(p.price)}</TableCell>
                        <TableCell>
                          {p.pnl >= 0 ? "Profit" : "Loss"} {formatCurrency(Math.abs(p.pnl))}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}

export default OptionStrategySimulator;
