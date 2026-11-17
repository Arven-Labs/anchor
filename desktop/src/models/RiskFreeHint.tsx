import { Button } from "@/components/ui/button";
import { formatDate, formatRate, useInterestRates } from "../rates/useInterestRates";

// Taxa livre de risco ao lado do campo do valuation, com fonte e data. Nunca
// preenche sozinha: no DCF o botão "Use" copia a taxa para o campo; nos modelos
// de Ke ela aparece como piso (um título público sem risco não pode pagar mais
// do que o retorno exigido da ação) e avisa se o Ke digitado ficou abaixo.
//
// O wrapper `risk-free` só existe depois que a consulta dos juros termina, com
// ou sem taxa — é o que os E2E esperam para afirmar que o hint não apareceu.
const SOURCE_TEXT = {
  di10y: "10-year DI, curve of",
  selic_meta: "Selic target, since",
} as const;

export default function RiskFreeHint({
  onUse,
  ke,
}: {
  /** DCF: recebe a taxa (em %) quando o usuário clica em "Use". */
  onUse?: (ratePct: number) => void;
  /** Modelos de Ke: valor digitado (em %), para avisar se ficou abaixo da taxa livre. */
  ke?: string;
}) {
  const { data, isPending } = useInterestRates();
  if (isPending) return null;

  const riskFree = data?.risk_free ?? null;
  const keValue = ke === undefined || ke.trim() === "" ? null : Number(ke);
  const keBelow = riskFree !== null && keValue !== null && Number.isFinite(keValue) && keValue >= riskFree.rate_pct;

  return (
    <div data-testid="risk-free" className="grid gap-1 text-sm text-muted-foreground">
      {riskFree && (
        <p data-testid="risk-free-hint" className="flex flex-wrap items-center gap-2">
          <span>
            Risk-free rate today: {formatRate(riskFree.rate_pct)} ({SOURCE_TEXT[riskFree.source]}{" "}
            {formatDate(riskFree.as_of)}).
            {onUse === undefined && " Ke should stay above it."}
          </span>
          {onUse && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              data-testid="risk-free-use"
              onClick={() => onUse(riskFree.rate_pct)}
            >
              Use
            </Button>
          )}
        </p>
      )}
      {keBelow && (
        <p data-testid="ke-below-risk-free" className="text-amber-700 dark:text-amber-400">
          Ke is at or below the risk-free rate: a risk-free bond pays at least as much as the
          return you are asking from this stock.
        </p>
      )}
    </div>
  );
}
