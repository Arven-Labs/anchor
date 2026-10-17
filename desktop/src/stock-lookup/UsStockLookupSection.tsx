import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError, ValuationModel } from "../types";
import { latestForTicker } from "../collector/latestForTicker";
import type {
  StockDcfFundamentals,
  StockDividendPayment,
  StockDividendsAvg,
  StockFundamentals,
  StockPriceHistory,
  StockQuote,
  StockTechnicals,
} from "../collector/types";
import VerdictBadge from "../components/VerdictBadge";
import DividendHistoryChart from "./DividendHistoryChart";
import PriceHistoryChart from "./PriceHistoryChart";
import NewValuationDialog from "../models/NewValuationDialog";
import SavedValuationsPanel from "../valuations/SavedValuationsPanel";
import AssetThesesSidebar from "./AssetThesesSidebar";
import {
  AddToAssetsButton,
  CompanyLogo,
  StatTile,
  formatPercent,
  formatRatio,
  type StockNote,
} from "./shared";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Dialog, DialogContent } from "@/components/ui/dialog";

type CollectorSummary = { success: boolean; output: string };

// Mesma constante de `StockAnalysisSection.tsx` — duplicada aqui em vez de
// compartilhada, seguindo o precedente do projeto de não abstrair UI entre
// seções de classe até uma 3ª instância idêntica aparecer.
const MODEL_LABELS: Record<string, string> = {
  bazin: "Bazin",
  graham: "Graham",
  gordon: "Gordon / DDM",
  dcf: "DCF / FCFF",
  banks: "Banks (P/B)",
  rim: "RIM (Bancos)",
  rnav: "RNAV",
  projected_ceiling: "Projected Ceiling",
};

// `shared.tsx::formatCurrency` é fixo em R$ (formato de toda classe B3) —
// ação americana usa seu próprio formatador local em USD, mesmo padrão que
// `MetalLookupSection.tsx::formatUsdPerOz` já usa.
function formatUsd(value: number | null | undefined): string {
  return value == null ? "—" : `US$ ${value.toFixed(2)}`;
}

/// Fase 10, item 8 — análise de ação americana (NYSE/NASDAQ, ex.: AAPL).
/// Fatia 1 deu cotação/técnicos/dividendos/histórico de preço, mesmo
/// endpoint Yahoo do resto do app sem o sufixo `.SA` (`acoes_yahoo.py`
/// ganhou um parâmetro `suffix` pra isso) — precisa passar `asset_class:
/// "acao_internacional"` explicitamente pro coletor rotear pro
/// `--us-ticker` (`commands/collector.rs::run_stock_collector`), diferente
/// de BDR/ETF que caem no `--ticker` genérico com `asset_class: null`.
/// Fatia 2 acrescenta fundamentos + DCF via SEC EDGAR (`sec_edgar.py`) e o
/// fluxo de "New Valuation"/"All Saved Valuations", irmã de
/// `StockAnalysisSection.tsx` — mesmos blocos, mantendo as convenções
/// próprias deste arquivo (`formatUsd`, sem "R$") em vez de virar
/// componente compartilhado (precedente do projeto: não abstrair UI entre
/// seções de classe até uma 3ª instância idêntica aparecer). Exposição
/// padrão no cadastro é "US", mesmo padrão do BDR.
function UsStockLookupSection({
  ticker,
  workspaceId,
}: {
  ticker: string;
  workspaceId: number;
}) {
  const [noteDraft, setNoteDraft] = useState("");
  const autoFetchedTickerRef = useRef<string | null>(null);
  const [newValuationOpen, setNewValuationOpen] = useState(false);
  const [savedValuationsOpen, setSavedValuationsOpen] = useState(false);

  const queryClient = useQueryClient();

  const quoteQuery = useQuery<StockQuote | null, AppError>({
    queryKey: ["us-stock-lookup-quote", ticker],
    queryFn: async () => {
      const quotes = await invoke<StockQuote[]>("list_stock_quotes");
      return latestForTicker(quotes, ticker);
    },
  });

  const collectorMutation = useMutation<CollectorSummary, AppError, string>({
    mutationFn: (t) =>
      invoke<CollectorSummary>("run_stock_collector", {
        ticker: t,
        assetClass: "acao_internacional",
      }),
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["us-stock-lookup-quote", ticker] });
      queryClient.invalidateQueries({ queryKey: ["us-stock-lookup-technicals", ticker] });
      queryClient.invalidateQueries({ queryKey: ["us-stock-lookup-dividends", ticker] });
      queryClient.invalidateQueries({ queryKey: ["us-stock-lookup-dividends-avg", ticker] });
      queryClient.invalidateQueries({ queryKey: ["us-stock-lookup-price-history", ticker] });
      queryClient.invalidateQueries({ queryKey: ["us-stock-lookup-fundamentals", ticker] });
      queryClient.invalidateQueries({ queryKey: ["us-stock-lookup-dcf-fundamentals", ticker] });
    },
  });

  const priceHistoryQuery = useQuery<StockPriceHistory[], AppError>({
    queryKey: ["us-stock-lookup-price-history", ticker],
    queryFn: () => invoke("list_stock_price_history", { ticker }),
  });

  const fundamentalsQuery = useQuery<StockFundamentals | null, AppError>({
    queryKey: ["us-stock-lookup-fundamentals", ticker],
    queryFn: async () => {
      const rows = await invoke<StockFundamentals[]>("list_stock_fundamentals");
      return latestForTicker(rows, ticker);
    },
  });

  const dcfFundamentalsQuery = useQuery<StockDcfFundamentals | null, AppError>({
    queryKey: ["us-stock-lookup-dcf-fundamentals", ticker],
    queryFn: async () => {
      const rows = await invoke<StockDcfFundamentals[]>("list_stock_dcf_fundamentals");
      return latestForTicker(rows, ticker);
    },
  });

  const valuationsQuery = useQuery<ValuationModel[], AppError>({
    queryKey: ["valuations"],
    queryFn: () => invoke("list_valuations"),
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
    queryKey: ["us-stock-lookup-technicals", ticker],
    queryFn: async () => {
      const rows = await invoke<StockTechnicals[]>("list_stock_technicals");
      return latestForTicker(rows, ticker);
    },
  });

  const dividendsQuery = useQuery<StockDividendPayment[], AppError>({
    queryKey: ["us-stock-lookup-dividends", ticker],
    queryFn: async () => {
      const payments = await invoke<StockDividendPayment[]>("list_stock_dividend_payments");
      return payments.filter((p) => p.ticker === ticker);
    },
  });

  const dividendsAvgQuery = useQuery<StockDividendsAvg | null, AppError>({
    queryKey: ["us-stock-lookup-dividends-avg", ticker],
    queryFn: async () => {
      const rows = await invoke<StockDividendsAvg[]>("list_stock_dividends_avg");
      return latestForTicker(rows, ticker);
    },
  });

  const notesQuery = useQuery<StockNote[], AppError>({
    queryKey: ["us-stock-lookup-notes", ticker],
    queryFn: () => invoke("list_stock_notes"),
  });
  const note = (notesQuery.data ?? []).find((n) => n.ticker === ticker) ?? null;

  useEffect(() => {
    setNoteDraft(note?.note ?? "");
  }, [note]);

  const saveNoteMutation = useMutation<StockNote, AppError, void>({
    mutationFn: () => invoke<StockNote>("save_stock_note", { request: { ticker, note: noteDraft } }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["us-stock-lookup-notes", ticker] });
    },
  });

  const price = quoteQuery.data?.price ?? null;
  const dividends = dividendsQuery.data ?? [];

  const lpa = fundamentalsQuery.data?.lpa ?? null;
  const vpa = fundamentalsQuery.data?.vpa ?? null;
  const roe = fundamentalsQuery.data?.roe ?? null;
  const pl = price != null && lpa ? price / lpa : null;
  const pvp = price != null && vpa ? price / vpa : null;

  // Mesmos indicadores derivados de `StockAnalysisSection.tsx` (Fase 9.2) —
  // conta em cima de campos que a SEC EDGAR já traz, sem comando próprio.
  // `ebit`/`total_debt`/`cash`/`shares_outstanding`/`revenue` já vêm em
  // US$ milhões (`sec_edgar.py::_to_millions`), soma/divide direto.
  const ebit = dcfFundamentalsQuery.data?.ebit ?? null;
  const da = dcfFundamentalsQuery.data?.depreciation_amortization ?? null;
  const totalDebt = dcfFundamentalsQuery.data?.total_debt ?? null;
  const cash = dcfFundamentalsQuery.data?.cash ?? null;
  const sharesOutstanding = dcfFundamentalsQuery.data?.shares_outstanding ?? null;
  const revenue = dcfFundamentalsQuery.data?.revenue ?? null;

  const ebitda = ebit != null && da != null ? ebit + da : null;
  const netDebt = totalDebt != null && cash != null ? totalDebt - cash : null;
  const netDebtToEbitda = netDebt != null && ebitda ? netDebt / ebitda : null;

  const marketCap = price != null && sharesOutstanding != null ? price * sharesOutstanding : null;
  const enterpriseValue =
    marketCap != null && totalDebt != null && cash != null
      ? marketCap + totalDebt - cash
      : null;
  const evToEbit = enterpriseValue != null && ebit ? enterpriseValue / ebit : null;

  const netIncome =
    roe != null && vpa != null && sharesOutstanding != null
      ? (roe / 100) * vpa * sharesOutstanding
      : null;
  const netMargin = netIncome != null && revenue ? (netIncome / revenue) * 100 : null;

  const savedValuations = (valuationsQuery.data ?? []).filter((v) => v.ticker === ticker);
  const latestValuation = savedValuations[0] ?? null;

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
                assetClass="acao_internacional"
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
            <StatTile label="CAGR 5y" value={formatPercent(technicalsQuery.data?.cagr_5y)} />
            <StatTile label="CAGR 10y" value={formatPercent(technicalsQuery.data?.cagr_10y)} />
            <StatTile
              label="Avg dividend/share (5y)"
              value={formatUsd(dividendsAvgQuery.data?.avg_dividend_5y)}
            />
            <StatTile label="P/L" value={formatRatio(pl)} />
            <StatTile label="P/VP" value={formatRatio(pvp)} />
            <StatTile label="Net Debt/EBITDA" value={formatRatio(netDebtToEbitda)} />
            <StatTile label="EV/EBIT" value={formatRatio(evToEbit)} />
            <StatTile label="Net Margin" value={formatPercent(netMargin)} />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Price history</h3>
            <PriceHistoryChart
              history={priceHistoryQuery.data ?? []}
              currencyPrefix={quoteQuery.data?.currency === "USD" ? "US$" : "R$"}
            />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Fundamentals</h3>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <StatTile label="LPA" value={formatUsd(lpa)} />
              <StatTile label="VPA" value={formatUsd(vpa)} />
              <StatTile label="ROE" value={formatPercent(roe)} />
              <StatTile label="Payout" value={formatPercent(fundamentalsQuery.data?.payout)} />
            </div>
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">DCF fundamentals</h3>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <StatTile label="EBIT" value={formatRatio(ebit)} />
              <StatTile
                label="Tax rate"
                value={formatPercent(dcfFundamentalsQuery.data?.tax_rate)}
              />
              <StatTile label="Total debt" value={formatRatio(totalDebt)} />
              <StatTile label="Cash" value={formatRatio(cash)} />
            </div>
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
              No dividend payments found for {ticker} — not every company distributes.
            </p>
          )}

          <div>
            <div className="mb-3 flex items-center justify-between">
              <h3 className="text-sm font-semibold text-muted-foreground">Saved valuation</h3>
              <div className="flex gap-2">
                <Button type="button" size="sm" onClick={() => setNewValuationOpen(true)}>
                  New Valuation
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => setSavedValuationsOpen(true)}
                >
                  All Saved Valuations
                </Button>
              </div>
            </div>
            {latestValuation ? (
              <div className="rounded-lg border border-border bg-card p-4">
                <p className="text-sm text-muted-foreground">
                  {MODEL_LABELS[latestValuation.model] ?? latestValuation.model}
                </p>
                <p className="mt-1 text-2xl font-semibold">
                  {formatUsd(latestValuation.fair_price)}
                </p>
                <div className="mt-2">
                  <VerdictBadge verdict={latestValuation.verdict} />
                </div>
              </div>
            ) : (
              <p className="text-muted-foreground">No saved valuation for {ticker} yet.</p>
            )}
          </div>

          <AssetThesesSidebar
            workspaceId={workspaceId}
            ticker={ticker}
            assetClass="acao_internacional"
            name={quoteQuery.data?.name ?? ticker}
            currency={quoteQuery.data?.currency ?? "USD"}
            exchange={quoteQuery.data?.exchange ?? null}
            cnpj={null}
            exposureType="pais"
            exposureValue="US"
          />

          <NewValuationDialog
            open={newValuationOpen}
            onOpenChange={setNewValuationOpen}
            ticker={ticker}
            assetClass="acao_internacional"
          />

          {/* Sem DialogHeader/DialogTitle aqui de propósito — mesmo motivo
              de `StockAnalysisSection.tsx`: `SavedValuationsPanel` já traz
              o próprio `Card`/`CardTitle`. */}
          <Dialog open={savedValuationsOpen} onOpenChange={setSavedValuationsOpen}>
            <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-4xl">
              <SavedValuationsPanel />
            </DialogContent>
          </Dialog>

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

export default UsStockLookupSection;
