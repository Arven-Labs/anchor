import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatFractionAsPercent, formatPercent, formatRatio } from "./shared";

// Screener de ações (ROADMAP, Sessão 340, 1ª fatia): filtra os tickers que o
// Anchor já coletou (preço + fundamentos na base). Não é a B3 inteira: as
// coletas são sob demanda, por ticker. A tela diz isso e mostra a idade do dado.

type ScreenerRow = {
  ticker: string;
  name: string | null;
  currency: string | null;
  price: number;
  pe: number | null;
  pb: number | null;
  roe: number;
  payout: number | null;
  dividend_yield_12m: number | null;
  dividend_yield_5y_avg: number | null;
  fundamentals_fetched_at: string;
};

type ScreenerResult = { universe_size: number; rows: ScreenerRow[] };

type Draft = { pe_max: string; pb_max: string; roe_min: string; dy_min: string };
type Filters = { pe_max: number | null; pb_max: number | null; roe_min: number | null; dy_min: number | null };

const EMPTY_DRAFT: Draft = { pe_max: "", pb_max: "", roe_min: "", dy_min: "" };

const FIELDS: { key: keyof Draft; label: string; hint: string }[] = [
  { key: "pe_max", label: "P/E max", hint: "e.g. 10" },
  { key: "pb_max", label: "P/B max", hint: "e.g. 1.5" },
  { key: "roe_min", label: "ROE min (%)", hint: "e.g. 15" },
  { key: "dy_min", label: "Dividend yield 12m min (%)", hint: "e.g. 6" },
];

/** Campo vazio vira `null` (filtro desligado); texto que não é número vira `undefined` (inválido). */
function parseField(text: string): number | null | undefined {
  const trimmed = text.trim().replace(",", ".");
  if (trimmed === "") return null;
  const value = Number(trimmed);
  return Number.isFinite(value) ? value : undefined;
}

function parseDraft(draft: Draft): Filters | null {
  const pe_max = parseField(draft.pe_max);
  const pb_max = parseField(draft.pb_max);
  const roe_min = parseField(draft.roe_min);
  const dy_min = parseField(draft.dy_min);
  if (pe_max === undefined || pb_max === undefined || roe_min === undefined || dy_min === undefined) {
    return null;
  }
  return { pe_max, pb_max, roe_min, dy_min };
}

function formatPrice(row: ScreenerRow): string {
  const symbol = row.currency === "USD" ? "US$" : "R$";
  return `${symbol} ${row.price.toFixed(2)}`;
}

function ScreenerSection() {
  const [draft, setDraft] = useState<Draft>(EMPTY_DRAFT);
  const [applied, setApplied] = useState<Filters>({
    pe_max: null,
    pb_max: null,
    roe_min: null,
    dy_min: null,
  });

  const query = useQuery<ScreenerResult, AppError>({
    queryKey: ["screen-stocks", applied],
    queryFn: () => invoke("screen_stocks", { filters: applied }),
  });

  const parsed = parseDraft(draft);

  return (
    <div className="flex flex-col gap-4" data-testid="screener">
      <p className="text-sm text-muted-foreground">
        Filters only the tickers Anchor has already collected (searched, held or on a watchlist), not the whole B3.
        A ticker with no value for a filtered indicator is left out.
      </p>

      <form
        className="grid grid-cols-2 items-end gap-3 md:grid-cols-5"
        onSubmit={(event) => {
          event.preventDefault();
          if (parsed) setApplied(parsed);
        }}
      >
        {FIELDS.map((field) => (
          <div key={field.key} className="flex flex-col gap-1.5">
            <Label htmlFor={`screener-${field.key}`}>{field.label}</Label>
            <Input
              id={`screener-${field.key}`}
              data-testid={`screener-${field.key}`}
              inputMode="decimal"
              placeholder={field.hint}
              value={draft[field.key]}
              onChange={(event) => setDraft({ ...draft, [field.key]: event.target.value })}
            />
          </div>
        ))}
        <div className="flex gap-2">
          <Button type="submit" disabled={!parsed} data-testid="screener-apply">
            Apply
          </Button>
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              setDraft(EMPTY_DRAFT);
              setApplied({ pe_max: null, pb_max: null, roe_min: null, dy_min: null });
            }}
          >
            Clear
          </Button>
        </div>
      </form>
      {!parsed && <p className="text-sm text-destructive">Use plain numbers in the filters (e.g. 10 or 1.5).</p>}

      {query.isPending && <p className="text-sm text-muted-foreground">Loading…</p>}
      {query.isError && <p className="text-sm text-destructive">{query.error.message}</p>}

      {query.data && (
        <>
          <p className="text-sm text-muted-foreground" data-testid="screener-summary">
            {query.data.rows.length} of {query.data.universe_size} collected tickers match.
          </p>
          {query.data.universe_size === 0 ? (
            <p className="text-sm text-muted-foreground">
              Nothing collected yet. Search a stock in the top bar to collect its price and fundamentals.
            </p>
          ) : (
            <Table data-testid="screener-table">
              <TableHeader>
                <TableRow>
                  <TableHead>Ticker</TableHead>
                  <TableHead>Name</TableHead>
                  <TableHead className="text-right">Price</TableHead>
                  <TableHead className="text-right">P/E</TableHead>
                  <TableHead className="text-right">P/B</TableHead>
                  <TableHead className="text-right">ROE</TableHead>
                  <TableHead className="text-right">Div. yield 12m</TableHead>
                  <TableHead className="text-right">Div. yield 5y avg</TableHead>
                  <TableHead className="text-right">Payout</TableHead>
                  <TableHead>Fundamentals as of</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {query.data.rows.map((row) => (
                  <TableRow key={row.ticker} data-testid={`screener-row-${row.ticker}`}>
                    <TableCell className="font-medium">{row.ticker}</TableCell>
                    <TableCell>{row.name ?? "—"}</TableCell>
                    <TableCell className="text-right">{formatPrice(row)}</TableCell>
                    <TableCell className="text-right">{formatRatio(row.pe)}</TableCell>
                    <TableCell className="text-right">{formatRatio(row.pb)}</TableCell>
                    <TableCell className="text-right">{formatPercent(row.roe)}</TableCell>
                    <TableCell className="text-right">{formatPercent(row.dividend_yield_12m)}</TableCell>
                    <TableCell className="text-right">{formatPercent(row.dividend_yield_5y_avg)}</TableCell>
                    <TableCell className="text-right">{formatFractionAsPercent(row.payout)}</TableCell>
                    <TableCell>{row.fundamentals_fetched_at.slice(0, 10)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </>
      )}
    </div>
  );
}

export default ScreenerSection;
