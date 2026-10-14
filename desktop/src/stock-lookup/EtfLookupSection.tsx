import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { latestForTicker } from "../collector/latestForTicker";
import type {
  StockDividendPayment,
  StockDividendsAvg,
  StockPriceHistory,
  StockQuote,
  StockTechnicals,
} from "../collector/types";
import DividendHistoryChart from "./DividendHistoryChart";
import PriceHistoryChart from "./PriceHistoryChart";
import {
  AddToAssetsButton,
  CompanyLogo,
  StatTile,
  formatCurrency,
  formatPercent,
  type StockNote,
} from "./shared";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

type CollectorSummary = { success: boolean; output: string };

/// Fase 10, item 8, Sessão 50 — análise de ETF, irmã de
/// `StockAnalysisSection.tsx`/`FiiLookupSection.tsx` na tela de Research
/// unificada. Sem fundamentos (LPA/VPA/ROE), sem DCF, sem "preço teto" —
/// nenhum desses conceitos existe pra um fundo de índice (pedido explícito
/// do dono do projeto: "tira a sessão de preço teto, não tem como fazer").
/// Só o que já é real e coletado de graça pelo Yahoo: cotação, médias de
/// período (SMA/CAGR, mesmo dado que a ação já mostra) e proventos (alguns
/// ETFs distribuem, outros não — a seção só aparece se houver pagamento).
/// Composição da carteira/índice replicado/site do fundo ficaram de fora —
/// pesquisado e confirmado que nem Yahoo nem bolsai têm esse dado de graça
/// pra ETF brasileiro (ver PHASE.md item 8, achado da Sessão 49/50).
function EtfLookupSection({ ticker }: { ticker: string }) {
  const [noteDraft, setNoteDraft] = useState("");
  const autoFetchedTickerRef = useRef<string | null>(null);

  const queryClient = useQueryClient();

  const quoteQuery = useQuery<StockQuote | null, AppError>({
    queryKey: ["etf-lookup-quote", ticker],
    queryFn: async () => {
      const quotes = await invoke<StockQuote[]>("list_stock_quotes");
      return latestForTicker(quotes, ticker);
    },
  });

  const collectorMutation = useMutation<CollectorSummary, AppError, string>({
    mutationFn: (t) =>
      invoke<CollectorSummary>("run_stock_collector", { ticker: t, assetClass: null }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["etf-lookup-quote", ticker] });
      queryClient.invalidateQueries({ queryKey: ["etf-lookup-technicals", ticker] });
      queryClient.invalidateQueries({ queryKey: ["etf-lookup-dividends", ticker] });
      queryClient.invalidateQueries({ queryKey: ["etf-lookup-dividends-avg", ticker] });
      queryClient.invalidateQueries({ queryKey: ["etf-lookup-price-history", ticker] });
    },
  });

  const priceHistoryQuery = useQuery<StockPriceHistory[], AppError>({
    queryKey: ["etf-lookup-price-history", ticker],
    queryFn: () => invoke("list_stock_price_history", { ticker }),
  });

  // Mesmo padrão cache-aware do resto do app: cotação ausente dispara o
  // coletor Yahoo no máximo uma vez por ticker buscado.
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

  const technicalsQuery = useQuery<StockTechnicals | null, AppError>({
    queryKey: ["etf-lookup-technicals", ticker],
    queryFn: async () => {
      const rows = await invoke<StockTechnicals[]>("list_stock_technicals");
      return latestForTicker(rows, ticker);
    },
  });

  const dividendsQuery = useQuery<StockDividendPayment[], AppError>({
    queryKey: ["etf-lookup-dividends", ticker],
    queryFn: async () => {
      const payments = await invoke<StockDividendPayment[]>("list_stock_dividend_payments");
      return payments.filter((p) => p.ticker === ticker);
    },
  });

  const dividendsAvgQuery = useQuery<StockDividendsAvg | null, AppError>({
    queryKey: ["etf-lookup-dividends-avg", ticker],
    queryFn: async () => {
      const rows = await invoke<StockDividendsAvg[]>("list_stock_dividends_avg");
      return latestForTicker(rows, ticker);
    },
  });

  const notesQuery = useQuery<StockNote[], AppError>({
    queryKey: ["etf-lookup-notes", ticker],
    queryFn: () => invoke("list_stock_notes"),
  });
  const note = (notesQuery.data ?? []).find((n) => n.ticker === ticker) ?? null;

  useEffect(() => {
    setNoteDraft(note?.note ?? "");
  }, [note]);

  const saveNoteMutation = useMutation<StockNote, AppError, void>({
    mutationFn: () => invoke<StockNote>("save_stock_note", { request: { ticker, note: noteDraft } }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["etf-lookup-notes", ticker] });
    },
  });

  const price = quoteQuery.data?.price ?? null;
  const dividends = dividendsQuery.data ?? [];

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
            <div className="flex items-center gap-3">
              <CompanyLogo ticker={ticker} />
              <h2 className="text-xl font-semibold">{ticker}</h2>
            </div>
            <div className="flex items-center gap-3">
              <AddToAssetsButton
                ticker={ticker}
                assetClass="etf_br"
                name={quoteQuery.data?.name ?? ticker}
                currency={quoteQuery.data?.currency ?? "BRL"}
                exchange={quoteQuery.data?.exchange ?? null}
                cnpj={null}
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

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <StatTile label="Price" value={formatCurrency(price)} />
            <StatTile label="SMA 50" value={formatCurrency(technicalsQuery.data?.sma_50)} />
            <StatTile label="SMA 100" value={formatCurrency(technicalsQuery.data?.sma_100)} />
            <StatTile label="SMA 200" value={formatCurrency(technicalsQuery.data?.sma_200)} />
            <StatTile label="CAGR 5y" value={formatPercent(technicalsQuery.data?.cagr_5y)} />
            <StatTile label="CAGR 10y" value={formatPercent(technicalsQuery.data?.cagr_10y)} />
            <StatTile
              label="Avg dividend/share (5y)"
              value={formatCurrency(dividendsAvgQuery.data?.avg_dividend_5y)}
            />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Price history</h3>
            <PriceHistoryChart
              history={priceHistoryQuery.data ?? []}
              currencyPrefix={quoteQuery.data?.currency === "USD" ? "US$" : "R$"}
            />
          </div>

          {dividends.length > 0 && (
            <div>
              <h3 className="mb-3 text-sm font-semibold text-muted-foreground">
                Dividend history
              </h3>
              <DividendHistoryChart payments={dividends} />
            </div>
          )}
          {dividendsQuery.isSuccess && dividends.length === 0 && (
            <p className="text-sm text-muted-foreground">
              No dividend payments found for {ticker} — not every ETF distributes.
            </p>
          )}

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

export default EtfLookupSection;
