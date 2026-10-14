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
import type { ReitFundamentals, ReitManualIndicators } from "../portfolio/types";
import DividendHistoryChart from "./DividendHistoryChart";
import PriceHistoryChart from "./PriceHistoryChart";
import {
  AddToAssetsButton,
  CompanyLogo,
  StatTile,
  formatFractionAsPercent,
  formatPercent,
  type StockNote,
} from "./shared";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import Field from "../components/Field";

type CollectorSummary = { success: boolean; output: string };

// Mesmo padrão de `UsStockLookupSection.tsx::formatUsd` — REIT cota em USD,
// `shared.tsx::formatCurrency` é fixo em R$.
function formatUsd(value: number | null | undefined): string {
  return value == null ? "—" : `US$ ${value.toFixed(2)}`;
}

const ONE_YEAR_MS = 365 * 24 * 60 * 60 * 1000;

/// Fase 10, item 8 — REIT (equivalente americano do FII, ex.: Realty Income
/// `O`). Cotação/técnicos/dividendos/histórico via Yahoo (mesmo pipeline de
/// `UsStockLookupSection.tsx`, sem `.SA`), indicadores imobiliários via SEC
/// EDGAR — mas em tabela própria (`reit_fundamentals`), **sem** os 8
/// modelos de valuation/DCF (decisão explícita: DCF clássico não encaixa
/// bem em imobiliário, mesmo espírito de FII não ter LPA/VPA/ROE).
///
/// **Achado confirmado ao vivo antes de implementar** (Realty Income/Simon
/// Property/Prologis/AvalonBay): FFO/AFFO e taxa de ocupação — os
/// indicadores "de verdade" de REIT — não existem como tag XBRL, são
/// métricas non-GAAP só em texto/tabela do 10-K. Por isso viram campo
/// manual editável (`reit_manual_indicators`), mesmo espírito do landbank
/// do RNAV, em vez de tentar automatizar algo que a fonte não tem.
///
/// Sem `AssetThesesSidebar` — não foi pedido, e FII também não tem (fica pra
/// uma sessão futura se sentir falta).
function ReitLookupSection({ ticker }: { ticker: string }) {
  const [noteDraft, setNoteDraft] = useState("");
  const [ffoDraft, setFfoDraft] = useState("");
  const [affoDraft, setAffoDraft] = useState("");
  const [occupancyDraft, setOccupancyDraft] = useState("");
  const autoFetchedTickerRef = useRef<string | null>(null);
  const manualIndicatorsPrefilledRef = useRef<string | null>(null);

  const queryClient = useQueryClient();

  const quoteQuery = useQuery<StockQuote | null, AppError>({
    queryKey: ["reit-lookup-quote", ticker],
    queryFn: async () => {
      const quotes = await invoke<StockQuote[]>("list_stock_quotes");
      return latestForTicker(quotes, ticker);
    },
  });

  const collectorMutation = useMutation<CollectorSummary, AppError, string>({
    mutationFn: (t) =>
      invoke<CollectorSummary>("run_stock_collector", { ticker: t, assetClass: "reit" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["reit-lookup-quote", ticker] });
      queryClient.invalidateQueries({ queryKey: ["reit-lookup-technicals", ticker] });
      queryClient.invalidateQueries({ queryKey: ["reit-lookup-dividends", ticker] });
      queryClient.invalidateQueries({ queryKey: ["reit-lookup-dividends-avg", ticker] });
      queryClient.invalidateQueries({ queryKey: ["reit-lookup-price-history", ticker] });
      queryClient.invalidateQueries({ queryKey: ["reit-lookup-fundamentals", ticker] });
    },
  });

  // Mesmo padrão cache-aware do resto do app: cotação ausente dispara o
  // coletor no máximo uma vez por ticker buscado — um fetch só cobre
  // cotação+técnicos+dividendos+histórico+indicadores imobiliários
  // (diferente do FII, que tem um segundo botão CVM separado).
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
    queryKey: ["reit-lookup-technicals", ticker],
    queryFn: async () => {
      const rows = await invoke<StockTechnicals[]>("list_stock_technicals");
      return latestForTicker(rows, ticker);
    },
  });

  const priceHistoryQuery = useQuery<StockPriceHistory[], AppError>({
    queryKey: ["reit-lookup-price-history", ticker],
    queryFn: () => invoke("list_stock_price_history", { ticker }),
  });

  const dividendsQuery = useQuery<StockDividendPayment[], AppError>({
    queryKey: ["reit-lookup-dividends", ticker],
    queryFn: async () => {
      const payments = await invoke<StockDividendPayment[]>("list_stock_dividend_payments");
      return payments.filter((p) => p.ticker === ticker);
    },
  });

  const dividendsAvgQuery = useQuery<StockDividendsAvg | null, AppError>({
    queryKey: ["reit-lookup-dividends-avg", ticker],
    queryFn: async () => {
      const rows = await invoke<StockDividendsAvg[]>("list_stock_dividends_avg");
      return latestForTicker(rows, ticker);
    },
  });

  const fundamentalsQuery = useQuery<ReitFundamentals | null, AppError>({
    queryKey: ["reit-lookup-fundamentals", ticker],
    queryFn: async () => {
      const rows = await invoke<ReitFundamentals[]>("list_reit_fundamentals");
      return latestForTicker(rows, ticker);
    },
  });

  const manualIndicatorsQuery = useQuery<ReitManualIndicators | null, AppError>({
    queryKey: ["reit-lookup-manual-indicators", ticker],
    queryFn: () => invoke("get_reit_manual_indicators", { ticker }),
  });

  useEffect(() => {
    if (
      manualIndicatorsQuery.isSuccess &&
      manualIndicatorsPrefilledRef.current !== ticker
    ) {
      manualIndicatorsPrefilledRef.current = ticker;
      const data = manualIndicatorsQuery.data;
      setFfoDraft(data?.ffo_per_share?.toString() ?? "");
      setAffoDraft(data?.affo_per_share?.toString() ?? "");
      setOccupancyDraft(data?.occupancy_pct?.toString() ?? "");
    }
  }, [ticker, manualIndicatorsQuery.isSuccess, manualIndicatorsQuery.data]);

  const saveManualIndicatorsMutation = useMutation<ReitManualIndicators, AppError, void>({
    mutationFn: () =>
      invoke("save_reit_manual_indicators", {
        request: {
          ticker,
          ffo_per_share: ffoDraft.trim() === "" ? null : Number(ffoDraft),
          affo_per_share: affoDraft.trim() === "" ? null : Number(affoDraft),
          occupancy_pct: occupancyDraft.trim() === "" ? null : Number(occupancyDraft),
        },
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["reit-lookup-manual-indicators", ticker] });
    },
  });

  const notesQuery = useQuery<StockNote[], AppError>({
    queryKey: ["reit-lookup-notes", ticker],
    queryFn: () => invoke("list_stock_notes"),
  });
  const note = (notesQuery.data ?? []).find((n) => n.ticker === ticker) ?? null;

  useEffect(() => {
    setNoteDraft(note?.note ?? "");
  }, [note]);

  const saveNoteMutation = useMutation<StockNote, AppError, void>({
    mutationFn: () => invoke<StockNote>("save_stock_note", { request: { ticker, note: noteDraft } }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["reit-lookup-notes", ticker] });
    },
  });

  const price = quoteQuery.data?.price ?? null;
  const dividends = dividendsQuery.data ?? [];

  // DY: soma dos pagamentos dos últimos 12 meses / preço — validado ao vivo
  // contra Realty Income (5,11%) antes de implementar. REIT paga mensal
  // (não trimestral), mas a conta não muda.
  const trailingYearDividends = dividends
    .filter((p) => Date.now() - new Date(p.payment_date).getTime() <= ONE_YEAR_MS)
    .reduce((sum, p) => sum + p.amount, 0);
  const dividendYield = price != null && trailingYearDividends > 0 ? trailingYearDividends / price : null;

  const fundamentals = fundamentalsQuery.data;

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
                assetClass="reit"
                name={quoteQuery.data?.name ?? ticker}
                currency={quoteQuery.data?.currency ?? "USD"}
                exchange={quoteQuery.data?.exchange ?? null}
                cnpj={null}
                exposureType="pais"
                exposureValue="US"
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
            <StatTile label="Price" value={formatUsd(price)} />
            <StatTile label="SMA 50" value={formatUsd(technicalsQuery.data?.sma_50)} />
            <StatTile label="SMA 100" value={formatUsd(technicalsQuery.data?.sma_100)} />
            <StatTile label="SMA 200" value={formatUsd(technicalsQuery.data?.sma_200)} />
            <StatTile label="Dividend yield (ttm)" value={formatFractionAsPercent(dividendYield)} />
            <StatTile
              label="Avg dividend/share (5y)"
              value={formatUsd(dividendsAvgQuery.data?.avg_dividend_5y)}
            />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Price history</h3>
            <PriceHistoryChart
              history={priceHistoryQuery.data ?? []}
              currencyPrefix="US$"
            />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">
              Real estate indicators
              {fundamentals && ` (FY${fundamentals.reference_year})`}
            </h3>
            {!fundamentals && !collectorMutation.isPending && (
              <p className="text-sm text-muted-foreground">
                No SEC EDGAR data yet — click "Refresh data" above.
              </p>
            )}
            {fundamentals && (
              <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
                <StatTile label="Revenue" value={formatUsd(fundamentals.revenue)} />
                <StatTile
                  label="Real estate property (net)"
                  value={formatUsd(fundamentals.real_estate_property_net)}
                />
                <StatTile
                  label="Real estate property (at cost)"
                  value={formatUsd(fundamentals.real_estate_property_at_cost)}
                />
                <StatTile label="Stockholders equity" value={formatUsd(fundamentals.stockholders_equity)} />
                <StatTile label="Net income" value={formatUsd(fundamentals.net_income)} />
                <StatTile label="EPS (diluted)" value={formatUsd(fundamentals.eps_diluted)} />
              </div>
            )}
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Manual indicators</h3>
            <p className="mb-3 text-sm text-muted-foreground">
              FFO, AFFO and occupancy rate aren't available as structured data from the SEC —
              they're non-GAAP metrics only disclosed as text/tables in the 10-K. No automatic
              source for now; fill in by hand if you have the numbers.
            </p>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
              <Field label="FFO/share (US$)">
                <Input
                  type="number"
                  step="0.01"
                  value={ffoDraft}
                  onChange={(e) => setFfoDraft(e.currentTarget.value)}
                />
              </Field>
              <Field label="AFFO/share (US$)">
                <Input
                  type="number"
                  step="0.01"
                  value={affoDraft}
                  onChange={(e) => setAffoDraft(e.currentTarget.value)}
                />
              </Field>
              <Field label="Occupancy (%)">
                <Input
                  type="number"
                  step="0.1"
                  value={occupancyDraft}
                  onChange={(e) => setOccupancyDraft(e.currentTarget.value)}
                />
              </Field>
            </div>
            <Button
              type="button"
              className="mt-3"
              size="sm"
              onClick={() => saveManualIndicatorsMutation.mutate()}
              disabled={saveManualIndicatorsMutation.isPending}
            >
              {saveManualIndicatorsMutation.isPending ? "Saving..." : "Save manual indicators"}
            </Button>
            {saveManualIndicatorsMutation.isError && (
              <p className="mt-2 text-red-600">{saveManualIndicatorsMutation.error.message}</p>
            )}
            {manualIndicatorsQuery.data && !saveManualIndicatorsMutation.isPending && (
              <p className="mt-2 text-xs text-muted-foreground">
                Last updated: {new Date(manualIndicatorsQuery.data.updated_at).toLocaleString()}
                {manualIndicatorsQuery.data.occupancy_pct != null &&
                  ` — Occupancy ${formatPercent(manualIndicatorsQuery.data.occupancy_pct)}`}
              </p>
            )}
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
              No dividend payments found for {ticker} — not every REIT distributes.
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

export default ReitLookupSection;
