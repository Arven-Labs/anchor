import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { latestForTicker } from "../collector/latestForTicker";
import type { StockPriceHistory, StockQuote } from "../collector/types";
import { AddToAssetsButton, StatTile, type StockNote } from "./shared";
import PriceHistoryChart from "./PriceHistoryChart";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

type CollectorSummary = { success: boolean; output: string };

// Onça troy → grama, só pra exibição (tile "US$/g" abaixo) — nunca usado
// pra gravar/ler `stock_quotes`/`stock_price_history`, que ficam 100% em
// onça desde a Sessão 57 (decisão explícita do dono do projeto).
const TROY_OUNCE_GRAMS = 31.1034768;

// Exposição especial por metal (`AddToAssetsButton` abaixo) — cada ticker
// tem a própria categoria, não só "gold_metal" (bug da Sessão 55, quando só
// ouro existia; corrigido na Sessão 57 junto com prata/platina/paládio).
const EXPOSURE_VALUE_BY_TICKER: Record<string, string> = {
  XAU: "gold_metal",
  XAG: "silver_metal",
  XPT: "platinum_metal",
  XPD: "palladium_metal",
};

function formatUsdPerOz(value: number | null | undefined): string {
  return value == null ? "—" : `US$ ${value.toFixed(2)}/oz`;
}

function formatUsdPerGram(value: number | null | undefined): string {
  return value == null ? "—" : `US$ ${value.toFixed(2)}/g`;
}

function formatPercentChange(value: number | null): string {
  if (value == null) return "—";
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

// % change from N calendar days ago to the most recent point in the series
// (sorted ascending by date) — closest available point wins, same helper
// already used identically in CryptoLookupSection.tsx.
function changeOverDays(history: StockPriceHistory[], days: number): number | null {
  if (history.length === 0) return null;
  const latest = history[history.length - 1];
  const targetTime = new Date(latest.price_date).getTime() - days * 86_400_000;
  let closest: StockPriceHistory | null = null;
  let closestDiff = Infinity;
  for (const point of history) {
    const diff = Math.abs(new Date(point.price_date).getTime() - targetTime);
    if (diff < closestDiff) {
      closestDiff = diff;
      closest = point;
    }
  }
  if (!closest || closest.close_price === 0) return null;
  return ((latest.close_price - closest.close_price) / closest.close_price) * 100;
}

// Simple moving average of the last `window` daily closes — same helper
// already used identically in CryptoLookupSection.tsx.
function sma(history: StockPriceHistory[], window: number): number | null {
  if (history.length < window) return null;
  const slice = history.slice(-window);
  const sum = slice.reduce((total, point) => total + point.close_price, 0);
  return sum / window;
}

/// Fase 10, item 8, Sessão 55 — análise de Metal, irmã de
/// `CryptoLookupSection.tsx`/`BdrLookupSection.tsx` na tela de Research.
/// Ouro (`XAU`), Prata (`XAG`), Platina (`XPT`) e Paládio (`XPD`) desde a
/// Sessão 57 — mesmo contrato futuro COMEX/NYMEX via Yahoo (sem `.SA`,
/// metal não é listado na B3). Preço/histórico gravados **sempre em USD
/// por onça troy** (unidade real do contrato, sem conversão nenhuma na
/// fonte — pedido explícito do dono do projeto, revertendo a conversão pra
/// grama que a Sessão 55 fazia) — o tile "US$/g" abaixo é só um indicador
/// simples de referência, calculado aqui na exibição, nunca persistido.
/// Sem fundamentos/DCF/dividendos (metal não paga provento) nem o painel de
/// indicadores de ciclo que Cripto tem (não se aplica) — só cotação,
/// variação percentual (mesmo padrão 7/30/90/365d de Cripto), médias
/// móveis (computadas client-side em cima do próprio histórico, mesmo
/// `sma` de Cripto) e notas.
function MetalLookupSection({ ticker }: { ticker: string }) {
  const [noteDraft, setNoteDraft] = useState("");
  const autoFetchedTickerRef = useRef<string | null>(null);

  const queryClient = useQueryClient();

  const quoteQuery = useQuery<StockQuote | null, AppError>({
    queryKey: ["metal-lookup-quote", ticker],
    queryFn: async () => {
      const quotes = await invoke<StockQuote[]>("list_stock_quotes");
      return latestForTicker(quotes, ticker);
    },
  });

  const historyQuery = useQuery<StockPriceHistory[], AppError>({
    queryKey: ["metal-lookup-history", ticker],
    queryFn: () => invoke("list_stock_price_history", { ticker }),
  });

  const collectorMutation = useMutation<CollectorSummary, AppError, string>({
    mutationFn: (t) =>
      invoke<CollectorSummary>("run_stock_collector", { ticker: t, assetClass: "metal" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["metal-lookup-quote", ticker] });
      queryClient.invalidateQueries({ queryKey: ["metal-lookup-history", ticker] });
    },
  });

  // Mesmo padrão cache-aware do resto do app: cotação ausente dispara o
  // coletor no máximo uma vez por ticker buscado.
  useEffect(() => {
    if (
      quoteQuery.isSuccess &&
      quoteQuery.data === null &&
      autoFetchedTickerRef.current !== ticker
    ) {
      autoFetchedTickerRef.current = ticker;
      collectorMutation.mutate(ticker);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ticker, quoteQuery.isSuccess, quoteQuery.data]);

  const notesQuery = useQuery<StockNote[], AppError>({
    queryKey: ["metal-lookup-notes", ticker],
    queryFn: () => invoke("list_stock_notes"),
  });
  const note = (notesQuery.data ?? []).find((n) => n.ticker === ticker) ?? null;

  useEffect(() => {
    setNoteDraft(note?.note ?? "");
  }, [note]);

  const saveNoteMutation = useMutation<StockNote, AppError, void>({
    mutationFn: () => invoke<StockNote>("save_stock_note", { request: { ticker, note: noteDraft } }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["metal-lookup-notes", ticker] });
    },
  });

  const price = quoteQuery.data?.price ?? null;
  const history = historyQuery.data ?? [];

  return (
    <div className="flex flex-col gap-6">
      {quoteQuery.isError && <p className="text-red-600">{quoteQuery.error.message}</p>}
      {collectorMutation.isError && (
        <p className="text-red-600">{collectorMutation.error.message}</p>
      )}

      {quoteQuery.isLoading && <p className="text-muted-foreground">Loading {ticker}...</p>}

      {quoteQuery.data === null && collectorMutation.isPending && (
        <p className="text-muted-foreground">Fetching {ticker} for the first time...</p>
      )}

      {quoteQuery.data === null && !collectorMutation.isPending && !quoteQuery.isLoading && (
        <p className="text-muted-foreground">No data found for {ticker}.</p>
      )}

      {quoteQuery.data !== null && quoteQuery.data !== undefined && (
        <>
          <div className="flex items-center justify-between">
            <h2 className="text-xl font-semibold">
              {ticker} {quoteQuery.data.name && `— ${quoteQuery.data.name}`}
            </h2>
            <div className="flex items-center gap-3">
              <AddToAssetsButton
                ticker={ticker}
                assetClass="metal"
                name={quoteQuery.data?.name ?? ticker}
                currency={quoteQuery.data?.currency ?? "USD"}
                exchange={quoteQuery.data?.exchange ?? null}
                cnpj={null}
                exposureType="categoria_especial"
                exposureValue={EXPOSURE_VALUE_BY_TICKER[ticker] ?? "metal"}
              />
              <Button
                type="button"
                variant="outline"
                onClick={() => collectorMutation.mutate(ticker)}
                disabled={collectorMutation.isPending}
              >
                {collectorMutation.isPending ? "Refreshing..." : "Refresh data"}
              </Button>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-6">
            <StatTile label="Price (oz)" value={formatUsdPerOz(price)} />
            <StatTile
              label="Price (g)"
              value={formatUsdPerGram(price == null ? null : price / TROY_OUNCE_GRAMS)}
            />
            <StatTile label="7d" value={formatPercentChange(changeOverDays(history, 7))} />
            <StatTile label="30d" value={formatPercentChange(changeOverDays(history, 30))} />
            <StatTile label="90d" value={formatPercentChange(changeOverDays(history, 90))} />
            <StatTile label="365d" value={formatPercentChange(changeOverDays(history, 365))} />
          </div>

          <div className="grid grid-cols-3 gap-3">
            <StatTile label="SMA 50 (oz)" value={formatUsdPerOz(sma(history, 50))} />
            <StatTile label="SMA 100 (oz)" value={formatUsdPerOz(sma(history, 100))} />
            <StatTile label="SMA 200 (oz)" value={formatUsdPerOz(sma(history, 200))} />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Price history</h3>
            <PriceHistoryChart history={history} currencyPrefix="US$" />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Notes</h3>
            <Textarea
              value={noteDraft}
              onChange={(e) => setNoteDraft(e.currentTarget.value)}
              rows={4}
            />
            <Button
              type="button"
              className="mt-2"
              onClick={() => saveNoteMutation.mutate()}
              disabled={saveNoteMutation.isPending}
            >
              {saveNoteMutation.isPending ? "Saving..." : "Save notes"}
            </Button>
            {saveNoteMutation.isError && (
              <p className="mt-2 text-red-600">{saveNoteMutation.error.message}</p>
            )}
          </div>
        </>
      )}
    </div>
  );
}

export default MetalLookupSection;
