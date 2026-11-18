import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import type { AppError } from "../types";

// Crescimento histórico dos dividendos ao lado do campo g (Gordon e Teto Projetado),
// com o período que o sustenta. Nunca preenche sozinho: o botão "Use" copia o valor.
// Sem histórico suficiente (menos de 3 anos de variação, ou parou de pagar) não
// aparece nada. O wrapper `dividend-growth` só existe depois que a consulta termina.
type DividendGrowth = { cagr_pct: number; from_year: number; to_year: number };

export default function DividendGrowthHint({
  ticker,
  ke,
  onUse,
}: {
  ticker: string;
  /** Ke digitado (em %), para avisar se g sugerido ficou igual ou acima dele (só Gordon, que diverge com g ≥ Ke). */
  ke?: string;
  onUse: (growthPct: number) => void;
}) {
  const symbol = ticker.trim();
  const { data, isPending } = useQuery<DividendGrowth | null, AppError>({
    queryKey: ["dividend-growth", symbol.toUpperCase()],
    queryFn: () => invoke("get_dividend_growth", { ticker: symbol }),
    enabled: symbol !== "",
  });
  if (symbol === "" || isPending || !data) return null;

  const suggested = Math.round(data.cagr_pct * 10) / 10;
  const keValue = ke === undefined || ke.trim() === "" ? null : Number(ke);
  const aboveKe = keValue !== null && Number.isFinite(keValue) && suggested >= keValue;

  return (
    <div data-testid="dividend-growth" className="grid gap-1 text-sm text-muted-foreground">
      <p className="flex flex-wrap items-center gap-2">
        <span data-testid="dividend-growth-hint">
          Dividends grew {suggested.toFixed(1)}% a year from {data.from_year} to {data.to_year}{" "}
          (sum of dividends per share per calendar year; extraordinary payments count).
        </span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          data-testid="dividend-growth-use"
          onClick={() => onUse(suggested)}
        >
          Use
        </Button>
      </p>
      {aboveKe && (
        <p data-testid="growth-above-ke" className="text-amber-700 dark:text-amber-400">
          g is at or above Ke: the Gordon formula only works when growth stays below the required return.
        </p>
      )}
    </div>
  );
}
