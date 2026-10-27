import { useEffect, useRef, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { latestForTicker } from "../collector/latestForTicker";
import type { StockPriceHistory, StockQuote } from "../collector/types";
import {
  INDICATORS,
  INDICATOR_KEYS,
  hasOwnScore,
  indicatorKeysFor,
  type IndicatorKey,
} from "./cryptoIndicators";
import { AddToAssetsButton, CryptoFearGreedGauge, StatTile, type StockNote } from "./shared";
import PriceHistoryChart from "./PriceHistoryChart";
import CryptoScoreHistoryChart, { type ScoreHistoryPoint } from "./CryptoScoreHistoryChart";
import ThresholdEditorDialog from "./ThresholdEditorDialog";
import Field from "../components/Field";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Textarea } from "@/components/ui/textarea";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

type CollectorSummary = { success: boolean; output: string };

type RecordCryptoIndicatorRequest = {
  coin: string;
  indicator: string;
  reading_date: string;
  raw_value: number;
  source: string;
};

type CryptoIndicatorReading = {
  id: number;
  coin: string;
  indicator: string;
  reading_date: string;
  raw_value: number;
  signal: "GREEN" | "NEUTRAL" | "RED";
  source: string;
  created_at: string;
};

const SIGNAL_STYLE: Record<CryptoIndicatorReading["signal"], string> = {
  GREEN: "bg-green-100 text-green-800 dark:bg-green-950 dark:text-green-300",
  NEUTRAL:
    "bg-yellow-100 text-yellow-800 dark:bg-yellow-950 dark:text-yellow-300",
  RED: "bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-300",
};

type DraftRow = { rawValue: string; source: string };
type Drafts = Record<IndicatorKey, DraftRow>;

function emptyDrafts(): Drafts {
  return Object.fromEntries(
    INDICATOR_KEYS.map((key) => [key, { rawValue: "", source: "" }]),
  ) as Drafts;
}

function today(): string {
  return new Date().toISOString().slice(0, 10);
}

function formatUsd(value: number | null | undefined): string {
  return value == null ? "—" : `US$ ${value.toFixed(2)}`;
}

function formatPercentChange(value: number | null): string {
  if (value == null) return "—";
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

// Fase 31 — the score (latest reading per indicator, grouped into subscores)
// is computed by the backend (`get_crypto_score`), not here.
type ScoreReading = {
  indicator: string;
  raw_value: number;
  signal: CryptoIndicatorReading["signal"];
  reading_date: string;
  source: string;
};

type GroupScore = {
  group: string;
  label: string;
  green: number;
  logged: number;
  total: number;
  indicators: { indicator: IndicatorKey; reading: ScoreReading | null; expired: boolean }[];
};

type CryptoScore = {
  groups: GroupScore[];
  green: number;
  logged: number;
  total: number;
  verdict: "intact" | "neutral" | "reduce_risk";
};

const VERDICT_LABEL: Record<CryptoScore["verdict"], string> = {
  intact: "Thesis intact — hold / keep adding",
  neutral: "Neutral — watch closely",
  reduce_risk: "Consider reducing risk",
};

// % change from N calendar days ago to the most recent point in the series
// (sorted ascending by date) — closest available point wins, same tolerance
// spirit as the TWR price lookup in profitability.rs, just simpler (no
// month-end alignment needed here, this is just a display stat).
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

// Simple moving average of the last `window` daily closes — `history` must
// be sorted ascending by date (list_stock_price_history already orders it
// that way). `null` when there isn't enough history yet (e.g. a coin
// searched for the first time won't have 200 days on day one).
function sma(history: StockPriceHistory[], window: number): number | null {
  if (history.length < window) return null;
  const slice = history.slice(-window);
  const sum = slice.reduce((total, point) => total + point.close_price, 0);
  return sum / window;
}

function IndicatorTile({
  indicatorKey,
  reading,
  expired,
}: {
  indicatorKey: IndicatorKey;
  reading?: ScoreReading | null;
  expired?: boolean;
}) {
  return (
    <div
      className="rounded-lg border border-border bg-card p-4"
      data-testid={`indicator-tile-${indicatorKey}`}
    >
      <p className="text-sm text-muted-foreground">{INDICATORS[indicatorKey]}</p>
      <p className={`mt-1 text-2xl font-semibold ${expired ? "text-muted-foreground" : ""}`}>
        {reading ? reading.raw_value.toFixed(2) : "—"}
      </p>
      <div className="mt-2 flex items-center justify-between gap-2">
        {reading && expired ? (
          // Past its validity window: still shown, but out of the score until logged again.
          <Badge variant="outline" data-testid={`indicator-expired-${indicatorKey}`}>
            EXPIRED
          </Badge>
        ) : reading ? (
          <Badge
            className={SIGNAL_STYLE[reading.signal]}
            data-testid={`indicator-signal-${indicatorKey}`}
          >
            {reading.signal}
          </Badge>
        ) : (
          <span className="text-sm text-muted-foreground">not logged</span>
        )}
        {reading && (
          <span className="text-xs text-muted-foreground">{reading.reading_date}</span>
        )}
      </div>
    </div>
  );
}

/// Fase 10, item 8, Sessão 51 — Cripto virou classe de ativo (AssetClass),
/// e esta seção une duas coisas que antes eram telas separadas: (1) a
/// cotação/histórico de preço genérica que Research já mostra pra
/// Ação/FII/ETF (fonte CoinGecko em vez de Yahoo, mas mesmo formato de
/// tabela — ver `collect_crypto_ticker` no coletor Python), e (2) o antigo
/// painel solto "Crypto Score" (9 indicadores de topo de ciclo, Fase 3),
/// movido pra cá igual — pedido explícito do dono do projeto: "unificar a
/// tela de análise de cripto na tela de research". Desde a Sessão 112 ETH e
/// BTC têm cada um o seu conjunto de indicadores, coletor
/// (`run_crypto_collector`) e limites em `indicator_thresholds`; o botão do
/// coletor só aparece buscando uma dessas duas. Qualquer outra moeda segue
/// com o registro manual, classificado pelos limites do ETH.
function CryptoLookupSection({ ticker }: { ticker: string }) {
  const [readingDate, setReadingDate] = useState(today());
  const [drafts, setDrafts] = useState<Drafts>(emptyDrafts());
  const [thresholdsOpen, setThresholdsOpen] = useState(false);
  const [noteDraft, setNoteDraft] = useState("");
  const autoFetchedTickerRef = useRef<string | null>(null);

  const queryClient = useQueryClient();
  // ETH and BTC have their own indicator set, collector and thresholds; any other
  // coin is scored with the ETH set.
  const scoreKeys = indicatorKeysFor(ticker);
  const ownScore = hasOwnScore(ticker);

  const quoteQuery = useQuery<StockQuote | null, AppError>({
    queryKey: ["crypto-lookup-quote", ticker],
    queryFn: async () => {
      const quotes = await invoke<StockQuote[]>("list_stock_quotes");
      return latestForTicker(quotes, ticker);
    },
  });

  const historyQuery = useQuery<StockPriceHistory[], AppError>({
    queryKey: ["crypto-lookup-history", ticker],
    queryFn: () => invoke("list_stock_price_history", { ticker }),
  });

  const collectorMutation = useMutation<CollectorSummary, AppError, string>({
    mutationFn: (t) =>
      invoke<CollectorSummary>("run_stock_collector", { ticker: t, assetClass: "cripto" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["crypto-lookup-quote", ticker] });
      queryClient.invalidateQueries({ queryKey: ["crypto-lookup-history", ticker] });
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
    queryKey: ["crypto-lookup-notes", ticker],
    queryFn: () => invoke("list_stock_notes"),
  });
  const note = (notesQuery.data ?? []).find((n) => n.ticker === ticker) ?? null;

  useEffect(() => {
    setNoteDraft(note?.note ?? "");
  }, [note]);

  const saveNoteMutation = useMutation<StockNote, AppError, void>({
    mutationFn: () => invoke<StockNote>("save_stock_note", { request: { ticker, note: noteDraft } }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["crypto-lookup-notes", ticker] });
    },
  });

  // --- Score de ciclo (antigo CryptoScorePanel), agora keyed pelo ticker
  // buscado em vez de um campo "Coin" próprio.
  const scoreQuery = useQuery<CryptoScore, AppError>({
    queryKey: ["crypto-score", ticker],
    queryFn: () => invoke("get_crypto_score", { coin: ticker }),
  });

  // Evolution of the same score, one point per reading date. Anything that
  // changes readings or thresholds must invalidate it next to ["crypto-score"]
  // (it does not match by prefix: "crypto-score-history" is another key).
  const scoreHistoryQuery = useQuery<ScoreHistoryPoint[], AppError>({
    queryKey: ["crypto-score-history", ticker],
    queryFn: () => invoke("list_crypto_score_history", { coin: ticker }),
  });

  const runCryptoCollectorMutation = useMutation<CollectorSummary, AppError, void>({
    mutationFn: () => invoke("run_crypto_collector", { coin: ticker }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["crypto-score", ticker] });
      queryClient.invalidateQueries({ queryKey: ["crypto-score-history", ticker] });
    },
  });

  const updateAllMutation = useMutation<void, AppError, void>({
    mutationFn: async () => {
      const entries = scoreKeys.filter((key) => drafts[key].rawValue.trim() !== "");
      await Promise.all(
        entries.map((indicator) => {
          const request: RecordCryptoIndicatorRequest = {
            coin: ticker,
            indicator,
            reading_date: readingDate,
            raw_value: Number(drafts[indicator].rawValue),
            source: drafts[indicator].source,
          };
          return invoke("record_crypto_indicator", { request });
        }),
      );
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["crypto-score", ticker] });
      queryClient.invalidateQueries({ queryKey: ["crypto-score-history", ticker] });
      setDrafts(emptyDrafts());
    },
  });

  function handleIndicatorsSubmit(event: FormEvent) {
    event.preventDefault();
    updateAllMutation.mutate();
  }

  function updateDraft(key: IndicatorKey, field: keyof DraftRow, value: string) {
    setDrafts((current) => ({ ...current, [key]: { ...current[key], [field]: value } }));
  }

  const price = quoteQuery.data?.price ?? null;
  const history = historyQuery.data ?? [];
  const score = scoreQuery.data;
  // Collector output: one line per reading, failures start with "ERROR ".
  const collectorErrors =
    runCryptoCollectorMutation.data?.output
      .split("\n")
      .filter((line) => line.startsWith("ERROR ")) ?? [];

  return (
    <div className="flex flex-col gap-6">
      <CryptoFearGreedGauge />

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
                assetClass="cripto"
                name={quoteQuery.data?.name ?? ticker}
                currency={quoteQuery.data?.currency ?? "USD"}
                exchange={quoteQuery.data?.exchange ?? null}
                cnpj={null}
                exposureType="categoria_especial"
                exposureValue="crypto"
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

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-5">
            <StatTile label="Price" value={formatUsd(price)} />
            <StatTile label="7d" value={formatPercentChange(changeOverDays(history, 7))} />
            <StatTile label="30d" value={formatPercentChange(changeOverDays(history, 30))} />
            <StatTile label="90d" value={formatPercentChange(changeOverDays(history, 90))} />
            <StatTile label="365d" value={formatPercentChange(changeOverDays(history, 365))} />
          </div>

          <div className="grid grid-cols-3 gap-3">
            <StatTile label="SMA 50" value={formatUsd(sma(history, 50))} />
            <StatTile label="SMA 100" value={formatUsd(sma(history, 100))} />
            <StatTile label="SMA 200" value={formatUsd(sma(history, 200))} />
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

          <div className="border-t border-border pt-6">
            <h3
              className="mb-1 text-sm font-semibold text-muted-foreground"
              data-testid="crypto-score-header"
            >
              Cycle-top score ({score?.green ?? 0}/{score?.total ?? scoreKeys.length} green,{" "}
              {score?.logged ?? 0} of {score?.total ?? scoreKeys.length} logged)
            </h3>
            {score && (
              <p className="mb-1 text-sm font-medium">{VERDICT_LABEL[score.verdict]}</p>
            )}
            <p className="mb-4 text-sm text-muted-foreground">
              {ownScore
                ? `These indicators are split into groups, each with its own subscore. ${ticker} has its own green/red thresholds, calibrated for it.`
                : `These indicators are split into groups, each with its own subscore, and were calibrated for Ethereum — logging them under ${ticker} is fine, but the green/red thresholds are ETH's, not ${ticker}'s.`}
            </p>
            {scoreHistoryQuery.data && <CryptoScoreHistoryChart history={scoreHistoryQuery.data} />}
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="mb-4 w-fit"
              data-testid="edit-thresholds"
              onClick={() => setThresholdsOpen(true)}
            >
              Edit thresholds
            </Button>
            <ThresholdEditorDialog coin={ticker} open={thresholdsOpen} onOpenChange={setThresholdsOpen} />

            {ownScore && (
              <div className="mb-4 flex flex-col gap-2">
                <Button
                  type="button"
                  variant="outline"
                  className="w-fit"
                  onClick={() => runCryptoCollectorMutation.mutate()}
                  disabled={runCryptoCollectorMutation.isPending}
                >
                  {runCryptoCollectorMutation.isPending
                    ? "Running..."
                    : `Run crypto collector (all ${scoreKeys.length} indicators)`}
                </Button>
                {collectorErrors.length > 0 && (
                  <p className="whitespace-pre-wrap text-red-600">
                    Some sources failed — the rest were saved:
                    {"\n"}
                    {collectorErrors.join("\n")}
                  </p>
                )}
                {runCryptoCollectorMutation.isError && (
                  <p className="text-red-600">{runCryptoCollectorMutation.error.message}</p>
                )}
              </div>
            )}

            {scoreQuery.isError && (
              <p className="mb-3 text-red-600">{scoreQuery.error.message}</p>
            )}

            {(score?.groups ?? []).map((group) => (
              <div key={group.group} className="mb-5">
                <h4 className="mb-2 flex items-center gap-2 text-sm font-semibold">
                  {group.label}
                  <Badge variant="secondary">
                    {group.green}/{group.total} green
                  </Badge>
                  <span className="text-xs font-normal text-muted-foreground">
                    {group.logged} of {group.total} logged
                  </span>
                </h4>
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
                  {group.indicators.map((item) => (
                    <IndicatorTile
                      key={item.indicator}
                      indicatorKey={item.indicator}
                      reading={item.reading}
                      expired={item.expired}
                    />
                  ))}
                </div>
              </div>
            ))}

            <h4 className="mt-6 mb-3 text-sm font-semibold text-muted-foreground">
              Update readings manually
            </h4>

            <form onSubmit={handleIndicatorsSubmit} className="flex flex-col gap-4">
              <Field label="Reading date" className="max-w-xs">
                <Input
                  required
                  type="date"
                  value={readingDate}
                  onChange={(e) => setReadingDate(e.currentTarget.value)}
                />
              </Field>

              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Indicator</TableHead>
                    <TableHead>Value</TableHead>
                    <TableHead>Source</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {scoreKeys.map((key) => (
                    <TableRow key={key}>
                      <TableCell className="whitespace-normal">{INDICATORS[key]}</TableCell>
                      <TableCell>
                        <Input
                          data-testid={`manual-${key}`}
                          type="number"
                          step="any"
                          value={drafts[key].rawValue}
                          onChange={(e) => updateDraft(key, "rawValue", e.currentTarget.value)}
                        />
                      </TableCell>
                      <TableCell>
                        <Input
                          value={drafts[key].source}
                          onChange={(e) => updateDraft(key, "source", e.currentTarget.value)}
                          placeholder="ultrasound.money"
                        />
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>

              {updateAllMutation.isError && (
                <p className="text-red-600">{updateAllMutation.error.message}</p>
              )}

              <Button
                type="submit"
                data-testid="manual-submit"
                disabled={updateAllMutation.isPending}
                className="w-fit"
              >
                {updateAllMutation.isPending ? "Updating..." : "Update all"}
              </Button>
            </form>
          </div>
        </>
      )}
    </div>
  );
}

export default CryptoLookupSection;
