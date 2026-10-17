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
import OptionsSection from "./OptionsSection";
import {
  AddToAssetsButton,
  CompanyLogo,
  StatTile,
  computeSafetyMargin,
  computeVerdict,
  formatCurrency,
  formatFractionAsPercent,
  formatPercent,
  formatRatio,
  type StockNote,
} from "./shared";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Dialog, DialogContent } from "@/components/ui/dialog";

type CollectorSummary = { success: boolean; output: string };

type LookupData = {
  quote: StockQuote | null;
  fundamentals: StockFundamentals | null;
  dividendsAvg: StockDividendsAvg | null;
  dcfFundamentals: StockDcfFundamentals | null;
  technicals: StockTechnicals | null;
  note: StockNote | null;
  // Diferente dos outros campos (última linha só), aqui é histórico
  // completo — o gráfico precisa de todos os pagamentos, não só o mais
  // recente.
  dividendPayments: StockDividendPayment[];
};

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

/// Fase 9 (extraído da antiga `StockLookupPanel.tsx` monolítica na Sessão
/// 43, sem mudança de comportamento) — análise de Ação BR: fundamentos,
/// DCF, técnicos, proventos, valuation salva. Irmã de `FiiLookupSection.tsx`
/// (indicadores completamente diferentes — vacância/inadimplência/imóveis
/// não fazem sentido aqui, LPA/VPA/ROE não fazem sentido lá), escolhidas
/// pelo seletor de classe em `StockLookupPanel.tsx`. Sessão 56 — a antiga
/// aba própria "Valuation" (seletor de modelo + 8 forms, `App.tsx`) migrou
/// pra cá: bloco "Saved valuation" ganhou "New Valuation" (abre
/// `NewValuationDialog.tsx`, ticker pré-preenchido) e "All Saved
/// Valuations" (abre `SavedValuationsPanel.tsx`, todos os tickers, dentro
/// de um `Dialog`) — pedido explícito do dono do projeto: os modelos
/// servem pra "empresas", não só Ação BR, então a tela migra pra dentro da
/// análise em vez de ficar solta no nav principal.
function StockAnalysisSection({
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

  const lookupQuery = useQuery<LookupData, AppError>({
    queryKey: ["stock-lookup", ticker],
    queryFn: async () => {
      const [quotes, fundamentals, dividendsAvg, dcfFundamentals, technicals, notes, dividendPayments] =
        await Promise.all([
          invoke<StockQuote[]>("list_stock_quotes"),
          invoke<StockFundamentals[]>("list_stock_fundamentals"),
          invoke<StockDividendsAvg[]>("list_stock_dividends_avg"),
          invoke<StockDcfFundamentals[]>("list_stock_dcf_fundamentals"),
          invoke<StockTechnicals[]>("list_stock_technicals"),
          invoke<StockNote[]>("list_stock_notes"),
          invoke<StockDividendPayment[]>("list_stock_dividend_payments"),
        ]);

      return {
        quote: latestForTicker(quotes, ticker),
        fundamentals: latestForTicker(fundamentals, ticker),
        dividendsAvg: latestForTicker(dividendsAvg, ticker),
        dcfFundamentals: latestForTicker(dcfFundamentals, ticker),
        technicals: latestForTicker(technicals, ticker),
        note: notes.find((n) => n.ticker === ticker) ?? null,
        dividendPayments: dividendPayments.filter((p) => p.ticker === ticker),
      };
    },
  });

  const valuationsQuery = useQuery<ValuationModel[], AppError>({
    queryKey: ["valuations"],
    queryFn: () => invoke("list_valuations"),
  });

  const priceHistoryQuery = useQuery<StockPriceHistory[], AppError>({
    queryKey: ["stock-lookup-price-history", ticker],
    queryFn: () => invoke("list_stock_price_history", { ticker }),
  });

  const collectorMutation = useMutation<CollectorSummary, AppError, string>({
    mutationFn: (t) =>
      invoke<CollectorSummary>("run_stock_collector", { ticker: t, assetClass: null }),
    // `onSettled`: o coletor grava a cotação antes dos fundamentos, que falham sem a chave
    // da bolsai. Mesmo com erro há dado novo no banco, então a tela relê de qualquer jeito.
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["stock-lookup", ticker] });
      queryClient.invalidateQueries({ queryKey: ["stock-lookup-price-history", ticker] });
    },
  });

  // Cache-aware fetch: a ticker with nothing in the DB yet gets exactly one
  // automatic collector run per search session (guarded by the ref, so a
  // genuinely-invalid ticker doesn't retrigger forever after invalidation).
  // Repeat searches for an already-collected ticker just read the DB —
  // "Refresh data" below is the only way to force a new fetch after that.
  useEffect(() => {
    if (
      lookupQuery.isSuccess &&
      lookupQuery.data.quote === null &&
      autoFetchedTickerRef.current !== ticker
    ) {
      autoFetchedTickerRef.current = ticker;
      collectorMutation.mutate(ticker);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ticker, lookupQuery.isSuccess, lookupQuery.data]);

  useEffect(() => {
    setNoteDraft(lookupQuery.data?.note?.note ?? "");
  }, [lookupQuery.data?.note]);

  const saveNoteMutation = useMutation<StockNote, AppError, void>({
    mutationFn: () =>
      invoke<StockNote>("save_stock_note", {
        request: { ticker, note: noteDraft },
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["stock-lookup", ticker] });
    },
  });

  const data = lookupQuery.data;
  const price = data?.quote?.price ?? null;
  const lpa = data?.fundamentals?.lpa ?? null;
  const vpa = data?.fundamentals?.vpa ?? null;
  const roe = data?.fundamentals?.roe ?? null;
  const pl = price != null && lpa ? price / lpa : null;
  const pvp = price != null && vpa ? price / vpa : null;

  // Os 3 indicadores abaixo (Fase 9.2) não têm comando próprio — são conta
  // em cima de campos que o DCF/fundamentals já trazem, mesmo espírito do
  // P/L e P/VP acima. `ebit`/`total_debt`/`cash`/`shares_outstanding`/
  // `revenue` já vêm em R$ milhões da CVM, então dá pra somar/dividir direto
  // sem converter unidade.
  const ebit = data?.dcfFundamentals?.ebit ?? null;
  const da = data?.dcfFundamentals?.depreciation_amortization ?? null;
  const totalDebt = data?.dcfFundamentals?.total_debt ?? null;
  const cash = data?.dcfFundamentals?.cash ?? null;
  const sharesOutstanding = data?.dcfFundamentals?.shares_outstanding ?? null;
  const revenue = data?.dcfFundamentals?.revenue ?? null;

  const ebitda = ebit != null && da != null ? ebit + da : null;
  const netDebt = totalDebt != null && cash != null ? totalDebt - cash : null;
  const netDebtToEbitda = netDebt != null && ebitda ? netDebt / ebitda : null;

  const marketCap = price != null && sharesOutstanding != null ? price * sharesOutstanding : null;
  const enterpriseValue =
    marketCap != null && totalDebt != null && cash != null
      ? marketCap + totalDebt - cash
      : null;
  const evToEbit = enterpriseValue != null && ebit ? enterpriseValue / ebit : null;

  // Lucro líquido não é buscado direto — deriva do ROE (lucro/patrimônio)
  // vezes o patrimônio (VPA × ações em circulação), evitando uma nova
  // extração na CVM só pra chegar num número que já dá pra calcular com o
  // que já temos guardado.
  const netIncome =
    roe != null && vpa != null && sharesOutstanding != null
      ? (roe / 100) * vpa * sharesOutstanding
      : null;
  const netMargin = netIncome != null && revenue ? (netIncome / revenue) * 100 : null;

  const savedValuations = (valuationsQuery.data ?? []).filter((v) => v.ticker === ticker);
  const latestValuation = savedValuations[0] ?? null;

  return (
    <div className="flex flex-col gap-6">
      {lookupQuery.isError && <p className="text-red-600">{lookupQuery.error.message}</p>}
      {collectorMutation.isError && (
        <p className="text-red-600">{collectorMutation.error.message}</p>
      )}

      {lookupQuery.isLoading && <p className="text-muted-foreground">Loading {ticker}...</p>}

      {data?.quote === null && collectorMutation.isPending && (
        <p className="text-muted-foreground">Fetching {ticker} for the first time...</p>
      )}

      {data?.quote === null && !collectorMutation.isPending && (
        <p className="text-muted-foreground">No data found for {ticker}.</p>
      )}

      {data && data.quote !== null && (
        <>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <CompanyLogo ticker={ticker} />
              <h2 className="text-xl font-semibold">{ticker}</h2>
            </div>
            <div className="flex items-center gap-3">
              <AddToAssetsButton
                ticker={ticker}
                assetClass="acao_br"
                name={data.quote?.name ?? ticker}
                currency={data.quote?.currency ?? "BRL"}
                exchange={data.quote?.exchange ?? null}
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
            <StatTile label="SMA 50" value={formatCurrency(data.technicals?.sma_50)} />
            <StatTile label="SMA 100" value={formatCurrency(data.technicals?.sma_100)} />
            <StatTile label="SMA 200" value={formatCurrency(data.technicals?.sma_200)} />
            <StatTile label="CAGR 5y" value={formatPercent(data.technicals?.cagr_5y)} />
            <StatTile label="CAGR 10y" value={formatPercent(data.technicals?.cagr_10y)} />
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
              currencyPrefix={data.quote?.currency === "USD" ? "US$" : "R$"}
            />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Fundamentals</h3>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <StatTile label="LPA" value={formatRatio(data.fundamentals?.lpa ?? null)} />
              <StatTile label="VPA" value={formatRatio(data.fundamentals?.vpa ?? null)} />
              <StatTile label="ROE" value={formatPercent(data.fundamentals?.roe)} />
              <StatTile label="Payout" value={formatPercent(data.fundamentals?.payout)} />
              <StatTile
                label="Avg dividend (5y)"
                value={formatCurrency(data.dividendsAvg?.avg_dividend_5y)}
              />
            </div>
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">DCF fundamentals</h3>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <StatTile label="EBIT" value={formatRatio(data.dcfFundamentals?.ebit ?? null)} />
              <StatTile label="Tax rate" value={formatPercent(data.dcfFundamentals?.tax_rate)} />
              <StatTile
                label="Total debt"
                value={formatRatio(data.dcfFundamentals?.total_debt ?? null)}
              />
              <StatTile label="Cash" value={formatRatio(data.dcfFundamentals?.cash ?? null)} />
            </div>
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Dividend history</h3>
            <DividendHistoryChart payments={data.dividendPayments} />
          </div>

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
                  {formatCurrency(latestValuation.fair_price)}
                </p>
                {(() => {
                  // Compare against the live quote price, not the price
                  // frozen in the valuation at calculation time — otherwise
                  // this card keeps showing yesterday's verdict after the
                  // stock moves and "Refresh data" only updates the quote
                  // above, never this card.
                  const livePrice = price;
                  const fairPrice = latestValuation.fair_price;
                  const liveMargin =
                    livePrice != null && fairPrice != null
                      ? computeSafetyMargin(fairPrice, livePrice)
                      : null;
                  const liveVerdict =
                    livePrice != null && fairPrice != null
                      ? computeVerdict(fairPrice, livePrice)
                      : latestValuation.verdict;
                  return (
                    <>
                      <p className="mt-1 text-sm text-muted-foreground">
                        Safety margin: {formatFractionAsPercent(liveMargin)}
                      </p>
                      <div className="mt-2">
                        <VerdictBadge verdict={liveVerdict} />
                      </div>
                    </>
                  );
                })()}
              </div>
            ) : (
              <p className="text-muted-foreground">No saved valuation for {ticker} yet.</p>
            )}
          </div>

          <OptionsSection key={ticker} ticker={ticker} price={price} workspaceId={workspaceId} />

          <AssetThesesSidebar
            workspaceId={workspaceId}
            ticker={ticker}
            assetClass="acao_br"
            name={data.quote?.name ?? ticker}
            currency={data.quote?.currency ?? "BRL"}
            exchange={data.quote?.exchange ?? null}
            cnpj={null}
          />

          <NewValuationDialog
            open={newValuationOpen}
            onOpenChange={setNewValuationOpen}
            ticker={ticker}
            assetClass="acao_br"
          />

          {/* Sem DialogHeader/DialogTitle aqui de propósito —
              `SavedValuationsPanel` já traz o próprio `Card`/`CardTitle`
              ("Saved valuations"), um título duplicado ficaria redundante. */}
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

export default StockAnalysisSection;
