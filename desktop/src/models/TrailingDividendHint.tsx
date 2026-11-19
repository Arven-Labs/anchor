import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import type { AppError } from "../types";

// Dividendo pago nos últimos 12 meses ao lado do D0 (Gordon e Teto Projetado): o Fetch
// traz a média de 5 anos, que subestima o D0 quando o dividendo cresce. Nunca preenche
// sozinho: "Use" copia o valor. Sem pagamento coletado não aparece nada; se o ativo
// parou de pagar, só avisa (Use poria D0 = 0). O wrapper só existe depois da consulta.
export default function TrailingDividendHint({
  ticker,
  onUse,
}: {
  ticker: string;
  onUse: (dividend: number) => void;
}) {
  const symbol = ticker.trim();
  const { data, isPending } = useQuery<number | null, AppError>({
    queryKey: ["trailing-dividend", symbol.toUpperCase()],
    queryFn: () => invoke("get_trailing_dividend", { ticker: symbol }),
    enabled: symbol !== "",
  });
  if (symbol === "" || isPending || data === null || data === undefined) return null;

  // toPrecision corta o ruído de ponto flutuante da soma (0.1 + 0.2) sem apagar casas reais.
  const suggested = Number(data.toPrecision(12));

  return (
    <p data-testid="trailing-dividend" className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
      {suggested > 0 ? (
        <>
          <span data-testid="trailing-dividend-hint">
            Paid R$ {suggested.toFixed(2)} per share in the last 12 months (extraordinary payments count).
          </span>
          <Button
            type="button"
            variant="outline"
            size="sm"
            data-testid="trailing-dividend-use"
            onClick={() => onUse(suggested)}
          >
            Use
          </Button>
        </>
      ) : (
        <span data-testid="trailing-dividend-none">No dividends paid in the last 12 months.</span>
      )}
    </p>
  );
}
