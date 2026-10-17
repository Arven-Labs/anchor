import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../types";
import { formatCurrency } from "./shared";
import type { OptionChain, OptionLeg, StrikeRow } from "./optionTypes";
import OptionStrategySimulator, { legFromSeries, type LegDraft } from "./OptionStrategySimulator";
import { Button } from "@/components/ui/button";
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

// Fase 18, 1ª fatia (Sessão 114) — consulta de opções da B3 na análise da
// Ação BR: cadeia por vencimento (call e put lado a lado) e as gregas de uma
// série. Só leitura. A fonte só tem o último preço negociado (sem bid/ask nem
// open interest) e a primeira carga do dia baixa ~80 MB da B3, por isso nada
// carrega sozinho.

type OptionGreeks = {
  series_ticker: string;
  underlying_ticker: string;
  option_type: "call" | "put";
  strike: number;
  expiration: string;
  spot: number;
  days_to_expiry: number;
  risk_free_rate_pct: number;
  di_curve_date: string;
  last_trade_date: string;
  last_price: number;
  implied_volatility_pct: number;
  delta: number;
  gamma: number;
  theta: number;
  vega: number;
};

function formatDate(iso: string): string {
  return new Date(`${iso}T00:00:00`).toLocaleDateString("pt-BR");
}

/** Strike mais próximo do preço da ação (a linha "at the money"). */
function atmStrike(rows: StrikeRow[], spot: number | null): number | null {
  if (spot == null || rows.length === 0) return null;
  return rows.reduce((best, row) =>
    Math.abs(row.strike - spot) < Math.abs(best.strike - spot) ? row : best,
  ).strike;
}

function LegCell({
  leg,
  selected,
  onSelect,
}: {
  leg: OptionLeg | null;
  selected: boolean;
  onSelect: (seriesTicker: string) => void;
}) {
  if (!leg) return <TableCell className="text-muted-foreground">—</TableCell>;
  return (
    <TableCell>
      <Button
        type="button"
        variant={selected ? "secondary" : "ghost"}
        size="sm"
        data-testid={`option-series-${leg.series_ticker}`}
        onClick={() => onSelect(leg.series_ticker)}
      >
        {leg.series_ticker}
      </Button>
    </TableCell>
  );
}

function GreeksPanel({ seriesTicker, underlying }: { seriesTicker: string; underlying: string }) {
  const greeksQuery = useQuery<OptionGreeks, AppError>({
    queryKey: ["option-greeks", seriesTicker, underlying],
    queryFn: () => invoke("get_option_greeks", { request: { series_ticker: seriesTicker, underlying } }),
    retry: false,
    staleTime: 5 * 60 * 1000,
  });

  if (greeksQuery.isLoading) {
    return <p className="text-sm text-muted-foreground">Calculating {seriesTicker}...</p>;
  }
  if (greeksQuery.isError) {
    return (
      <p className="text-sm text-red-600">
        {seriesTicker}: {greeksQuery.error.message}
      </p>
    );
  }
  const g = greeksQuery.data;
  if (!g) return null;

  const rows: [string, string][] = [
    ["Implied volatility", `${g.implied_volatility_pct.toFixed(1)}%`],
    ["Delta", g.delta.toFixed(3)],
    ["Gamma", g.gamma.toFixed(4)],
    ["Theta (per day)", g.theta.toFixed(4)],
    ["Vega (per vol point)", g.vega.toFixed(4)],
    ["Risk-free rate (DI)", `${g.risk_free_rate_pct.toFixed(2)}%`],
    ["Days to expiry", String(g.days_to_expiry)],
    ["Spot", formatCurrency(g.spot)],
    ["Last price", `${formatCurrency(g.last_price)} on ${formatDate(g.last_trade_date)}`],
  ];

  return (
    <div className="rounded-lg border border-border p-4" data-testid="option-greeks">
      <p className="mb-3 text-sm font-medium">
        {g.series_ticker} — {g.option_type}, strike {formatCurrency(g.strike)}, expires{" "}
        {formatDate(g.expiration)}
      </p>
      <dl className="grid grid-cols-2 gap-x-6 gap-y-2 text-sm sm:grid-cols-3">
        {rows.map(([label, value]) => (
          <div key={label}>
            <dt className="text-muted-foreground">{label}</dt>
            <dd className="font-medium">{value}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

function OptionsSection({
  ticker,
  price,
  workspaceId,
}: {
  ticker: string;
  price: number | null;
  workspaceId: number;
}) {
  const [requested, setRequested] = useState(false);
  const [expiration, setExpiration] = useState<string | null>(null);
  const [onlyTraded, setOnlyTraded] = useState(true);
  const [selected, setSelected] = useState<string | null>(null);
  // As pernas do simulador moram aqui porque a cadeia também alimenta o simulador
  // ("Add buy/sell leg" na série escolhida e os atalhos prontos).
  const [legs, setLegs] = useState<LegDraft[]>([]);

  const chainQuery = useQuery<OptionChain, AppError>({
    queryKey: ["option-chain", ticker],
    queryFn: () => invoke("list_option_chain", { request: { underlying: ticker } }),
    enabled: requested,
    // Uma carga é um download de ~80 MB: nunca repetir sozinha.
    retry: false,
    staleTime: Infinity,
  });

  const chain = chainQuery.data;
  const current =
    chain?.expirations.find((e) => e.expiration === expiration) ?? chain?.expirations[0] ?? null;
  const visibleRows = (current?.rows ?? []).filter(
    (row) => !onlyTraded || row.call?.last_price != null || row.put?.last_price != null,
  );
  const atm = atmStrike(visibleRows, price);

  // A série escolhida na tabela, com o tipo e o strike que o simulador precisa.
  const selectedSeries = (() => {
    for (const row of current?.rows ?? []) {
      if (row.call?.series_ticker === selected) {
        return { kind: "call" as const, strike: row.strike, lastPrice: row.call.last_price };
      }
      if (row.put?.series_ticker === selected) {
        return { kind: "put" as const, strike: row.strike, lastPrice: row.put.last_price };
      }
    }
    return null;
  })();

  return (
    <div className="flex flex-col gap-6">
    <Card data-testid="options-section">
      <CardHeader>
        <CardTitle>Options</CardTitle>
        <CardDescription>
          B3 options on {ticker}: series by expiry, last traded price and, for a chosen series,
          implied volatility and Greeks. No bid/ask or open interest is available, and Greeks use
          the European approximation.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {!requested && (
          <div className="flex flex-col items-start gap-2">
            <p className="text-sm text-muted-foreground">
              The first load of the day downloads the B3 files (about 80 MB) and can take around 2
              minutes. After that it is cached for 24 hours.
            </p>
            <Button type="button" data-testid="load-options" onClick={() => setRequested(true)}>
              Load options
            </Button>
          </div>
        )}

        {requested && chainQuery.isLoading && (
          <p className="text-sm text-muted-foreground" data-testid="options-loading">
            Downloading the B3 files... the first load can take around 2 minutes.
          </p>
        )}

        {chainQuery.isError && (
          <div className="flex flex-col items-start gap-2">
            <p className="text-sm text-red-600">{chainQuery.error.message}</p>
            <Button type="button" variant="outline" size="sm" onClick={() => chainQuery.refetch()}>
              Try again
            </Button>
          </div>
        )}

        {chain && chain.expirations.length === 0 && (
          <p className="text-sm text-muted-foreground">No options registered for {ticker}.</p>
        )}

        {chain && chain.expirations.length > 0 && current && (
          <>
            <div className="flex flex-wrap items-center gap-3">
              <Select value={current.expiration} onValueChange={setExpiration}>
                <SelectTrigger size="sm" className="w-56" data-testid="option-expiration">
                  <SelectValue>
                    {formatDate(current.expiration)} ({current.days_to_expiry}d)
                  </SelectValue>
                </SelectTrigger>
                <SelectContent align="start">
                  {chain.expirations.map((e) => (
                    <SelectItem key={e.expiration} value={e.expiration}>
                      {formatDate(e.expiration)} ({e.days_to_expiry}d)
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Button
                type="button"
                variant={onlyTraded ? "secondary" : "outline"}
                size="sm"
                data-testid="option-only-traded"
                onClick={() => setOnlyTraded((v) => !v)}
              >
                Only traded: {onlyTraded ? "on" : "off"}
              </Button>
              {chain.stale && (
                <span className="text-xs text-muted-foreground">
                  The B3 is unreachable; showing older data.
                </span>
              )}
            </div>

            <Table data-testid="option-chain">
              <TableHeader>
                <TableRow>
                  <TableHead>Call</TableHead>
                  <TableHead>Last</TableHead>
                  <TableHead className="text-center">Strike</TableHead>
                  <TableHead>Last</TableHead>
                  <TableHead>Put</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {visibleRows.map((row) => (
                  <TableRow
                    key={row.strike}
                    className={row.strike === atm ? "bg-muted/50" : undefined}
                  >
                    <LegCell
                      leg={row.call}
                      selected={selected === row.call?.series_ticker}
                      onSelect={setSelected}
                    />
                    <TableCell>{formatCurrency(row.call?.last_price)}</TableCell>
                    <TableCell className="text-center font-medium">
                      {row.strike.toFixed(2)}
                      {row.strike === atm && (
                        <span className="ml-2 text-xs text-muted-foreground">ATM</span>
                      )}
                    </TableCell>
                    <TableCell>{formatCurrency(row.put?.last_price)}</TableCell>
                    <LegCell
                      leg={row.put}
                      selected={selected === row.put?.series_ticker}
                      onSelect={setSelected}
                    />
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            {visibleRows.length === 0 && (
              <p className="text-sm text-muted-foreground">
                No traded series for this expiry. Turn "Only traded" off to see all of them.
              </p>
            )}

            {selected && <GreeksPanel seriesTicker={selected} underlying={ticker} />}

            {selectedSeries && (
              <div className="flex flex-wrap items-center gap-2">
                {(["long", "short"] as const).map((side) => (
                  <Button
                    key={side}
                    type="button"
                    variant="outline"
                    size="sm"
                    data-testid={`add-series-${side}`}
                    onClick={() =>
                      setLegs((existing) => [
                        ...existing,
                        legFromSeries(
                          selectedSeries.kind,
                          side,
                          selectedSeries.strike,
                          selectedSeries.lastPrice,
                          selected ?? "",
                          current?.expiration ?? null,
                        ),
                      ])
                    }
                  >
                    {side === "long" ? "Add as buy leg" : "Add as sell leg"}
                  </Button>
                ))}
                <span className="text-xs text-muted-foreground">
                  to the strategy simulator below, at the last traded price
                </span>
              </div>
            )}
          </>
        )}
      </CardContent>
    </Card>

    <OptionStrategySimulator
      spot={price}
      rows={current?.rows ?? []}
      expiration={current?.expiration ?? null}
      workspaceId={workspaceId}
      underlying={ticker}
      legs={legs}
      setLegs={setLegs}
    />
    </div>
  );
}

export default OptionsSection;
