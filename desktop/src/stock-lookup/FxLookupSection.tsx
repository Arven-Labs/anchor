import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { latestForTicker } from "../collector/latestForTicker";
import type { StockPriceHistory, StockQuote } from "../collector/types";
import { StatTile } from "./shared";
import PriceHistoryChart from "./PriceHistoryChart";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

type CollectorSummary = { success: boolean; output: string };

// Espelho de `CURRENCIES` em `src-tauri/src/finance_core/fx.rs` (17 moedas
// ISO 4217, curadas pro público brasileiro). O Rust valida o par de novo, então
// uma moeda que fique só aqui vira "No data found", não um dado errado.
const CURRENCIES: [string, string][] = [
  ["USD", "US Dollar"],
  ["EUR", "Euro"],
  ["GBP", "British Pound"],
  ["JPY", "Japanese Yen"],
  ["CHF", "Swiss Franc"],
  ["CAD", "Canadian Dollar"],
  ["AUD", "Australian Dollar"],
  ["NZD", "New Zealand Dollar"],
  ["CNY", "Chinese Yuan"],
  ["BRL", "Brazilian Real"],
  ["ARS", "Argentine Peso"],
  ["UYU", "Uruguayan Peso"],
  ["PYG", "Paraguayan Guarani"],
  ["CLP", "Chilean Peso"],
  ["COP", "Colombian Peso"],
  ["PEN", "Peruvian Sol"],
  ["MXN", "Mexican Peso"],
];

// Cotação com 4 casas (1,0832); o JPY e o guarani passam de 100 e perdem
// pouco com isso, e o gráfico usa a mesma precisão.
const RATE_DECIMALS = 4;

function formatRate(value: number | null | undefined): string {
  return value == null ? "—" : value.toFixed(RATE_DECIMALS);
}

function formatPercentChange(value: number | null): string {
  if (value == null) return "—";
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

// Mesmo helper de `MetalLookupSection.tsx`/`CryptoLookupSection.tsx`: variação
// de N dias atrás até o último ponto (série ascendente), o ponto mais próximo vence.
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

function sma(history: StockPriceHistory[], window: number): number | null {
  if (history.length < window) return null;
  const slice = history.slice(-window);
  const sum = slice.reduce((total, point) => total + point.close_price, 0);
  return sum / window;
}

function CurrencySelect({
  testId,
  value,
  onChange,
}: {
  testId: string;
  value: string;
  onChange: (code: string) => void;
}) {
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger data-testid={testId} size="sm" className="w-28">
        <SelectValue>{value}</SelectValue>
      </SelectTrigger>
      <SelectContent align="start">
        {CURRENCIES.map(([code, name]) => (
          <SelectItem key={code} value={code}>
            {code} — {name}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

/// Fase 32, seção I — câmbio multi-moeda (porta do `currency_service` do Lume).
/// Diferente das outras seções de Research, não recebe ticker: o par se escolhe
/// aqui mesmo (base e cotada), e a moeda não é uma classe de ativo da carteira,
/// por isso a tela é uma `view` própria (como os juros) e não entra em
/// `AssetClass`. A cotação e o histórico de 10 anos são gravados em
/// `stock_quotes`/`stock_price_history` com o código do par (`EURUSD`) e lidos
/// pelos mesmos comandos genéricos. Sem notas nem "adicionar aos ativos".
function FxLookupSection() {
  const [base, setBase] = useState("USD");
  const [quote, setQuote] = useState("BRL");
  const autoFetchedPairRef = useRef<string | null>(null);

  const queryClient = useQueryClient();
  const samePair = base === quote;
  const pair = `${base}${quote}`;

  const quoteQuery = useQuery<StockQuote | null, AppError>({
    queryKey: ["fx-lookup-quote", pair],
    queryFn: async () => {
      const quotes = await invoke<StockQuote[]>("list_stock_quotes");
      return latestForTicker(quotes, pair);
    },
    enabled: !samePair,
  });

  const historyQuery = useQuery<StockPriceHistory[], AppError>({
    queryKey: ["fx-lookup-history", pair],
    queryFn: () => invoke("list_stock_price_history", { ticker: pair }),
    enabled: !samePair,
  });

  const collectorMutation = useMutation<CollectorSummary, AppError, void>({
    mutationFn: () => invoke<CollectorSummary>("run_fx_collector", { base, quote }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["fx-lookup-quote", pair] });
      queryClient.invalidateQueries({ queryKey: ["fx-lookup-history", pair] });
    },
  });

  // Cotação ausente dispara o coletor no máximo uma vez por par escolhido.
  useEffect(() => {
    if (
      !samePair &&
      quoteQuery.isSuccess &&
      quoteQuery.data === null &&
      autoFetchedPairRef.current !== pair
    ) {
      autoFetchedPairRef.current = pair;
      collectorMutation.mutate();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pair, samePair, quoteQuery.isSuccess, quoteQuery.data]);

  const rate = quoteQuery.data?.price ?? null;
  const history = historyQuery.data ?? [];

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-wrap items-center gap-3">
        <CurrencySelect testId="fx-base" value={base} onChange={setBase} />
        <span className="text-muted-foreground">to</span>
        <CurrencySelect testId="fx-quote" value={quote} onChange={setQuote} />
      </div>

      {samePair && <p className="text-muted-foreground">Pick two different currencies.</p>}

      {quoteQuery.isError && <p className="text-red-600">{quoteQuery.error.message}</p>}
      {collectorMutation.isError && (
        <p className="text-red-600">{collectorMutation.error.message}</p>
      )}

      {!samePair && quoteQuery.isLoading && (
        <p className="text-muted-foreground">Loading {base}/{quote}...</p>
      )}

      {!samePair && quoteQuery.data === null && collectorMutation.isPending && (
        <p className="text-muted-foreground">Fetching {base}/{quote} for the first time...</p>
      )}

      {!samePair &&
        quoteQuery.data === null &&
        !collectorMutation.isPending &&
        !quoteQuery.isLoading && (
          <p className="text-muted-foreground">No data found for {base}/{quote}.</p>
        )}

      {!samePair && quoteQuery.data !== null && quoteQuery.data !== undefined && (
        <>
          <div className="flex items-center justify-between">
            <h2 className="text-xl font-semibold">
              {base}/{quote} — 1 {base} in {quote}
            </h2>
            <Button
              type="button"
              variant="outline"
              onClick={() => collectorMutation.mutate()}
              disabled={collectorMutation.isPending}
            >
              {collectorMutation.isPending ? "Refreshing..." : "Refresh data"}
            </Button>
          </div>

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-5">
            <StatTile label={`Rate (${quote})`} value={formatRate(rate)} />
            <StatTile label="7d" value={formatPercentChange(changeOverDays(history, 7))} />
            <StatTile label="30d" value={formatPercentChange(changeOverDays(history, 30))} />
            <StatTile label="90d" value={formatPercentChange(changeOverDays(history, 90))} />
            <StatTile label="365d" value={formatPercentChange(changeOverDays(history, 365))} />
          </div>

          <div className="grid grid-cols-3 gap-3">
            <StatTile label="SMA 50" value={formatRate(sma(history, 50))} />
            <StatTile label="SMA 100" value={formatRate(sma(history, 100))} />
            <StatTile label="SMA 200" value={formatRate(sma(history, 200))} />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Rate history</h3>
            <PriceHistoryChart history={history} currencyPrefix={quote} decimals={RATE_DECIMALS} />
          </div>
        </>
      )}
    </div>
  );
}

export default FxLookupSection;
