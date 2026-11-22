import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation } from "@tanstack/react-query";
import type { AppError } from "../types";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

// Previdência privada: limite de dedução das contribuições ao PGBL na declaração (Lei 9.532/1997, art. 11, redação da Lei
// 10.887/2004). Informativo: nada é gravado e nada entra na apuração de IR, no patrimônio nem na Rentabilidade. O percentual
// vem de `tax_rules` (editável em Settings → Taxes). A base e os aportes ao PGBL são digitados: o app não grava o tipo do
// plano (não separa PGBL de VGBL) e não afirma o que compõe a base da declaração.
type DeductionLimitEstimate = {
  rate: number;
  limit: number;
  deductible: number;
  remaining: number;
  excess: number;
};

function formatMoney(value: number, currency: string): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency });
}

function formatPct(value: number): string {
  return `${(value * 100).toFixed(2)}%`;
}

export default function PensionDeductionLimit({ currency }: { currency: string }) {
  const [base, setBase] = useState("");
  const [contributions, setContributions] = useState("");

  const estimateMutation = useMutation<DeductionLimitEstimate, AppError, void>({
    mutationFn: () =>
      invoke("estimate_pension_deduction_limit", {
        request: { base: Number(base), contributions: Number(contributions) },
      }),
  });

  const result = estimateMutation.data;
  const canSubmit = Number(base) > 0 && contributions !== "" && Number(contributions) >= 0;

  // Mexer num campo apaga o resultado anterior: ele já não corresponde ao que está digitado.
  function edit(setter: (value: string) => void) {
    return (event: FormEvent<HTMLInputElement>) => {
      setter(event.currentTarget.value);
      estimateMutation.reset();
    };
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    estimateMutation.mutate();
  }

  return (
    <div data-testid="pension-deduction-limit">
      <h3 className="mb-2 text-sm font-semibold">PGBL deduction limit</h3>
      <p className="mb-3 text-sm text-muted-foreground">
        Estimate only: nothing here is saved or counted in your tax assessment. Contributions to a
        PGBL are deductible up to a share of the total income in the tax base of the annual return
        (Lei 9.532/1997, art. 11; you can edit the percentage in Settings → Taxes). It only applies
        to the complete model, to PGBL (not VGBL), and if you also contribute to the general or
        your own social security regime. Enter the base as the return computes it; the app does not
        work it out.
      </p>
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <Field label="Income in the tax base (year)">
            <Input
              required
              data-testid="pension-limit-base"
              type="number"
              step="any"
              min="0"
              value={base}
              onChange={edit(setBase)}
            />
          </Field>
          <Field label="PGBL contributions (year)">
            <Input
              required
              data-testid="pension-limit-contributions"
              type="number"
              step="any"
              min="0"
              value={contributions}
              onChange={edit(setContributions)}
            />
          </Field>
        </div>
        <Button
          type="submit"
          className="w-fit"
          data-testid="pension-limit-submit"
          disabled={!canSubmit || estimateMutation.isPending}
        >
          {estimateMutation.isPending ? "Calculating..." : "Calculate limit"}
        </Button>
      </form>
      {estimateMutation.isError && (
        <p className="mt-3 text-sm text-destructive">{estimateMutation.error.message}</p>
      )}
      {result && (
        <div className="mt-3 flex flex-col gap-1 text-sm" data-testid="pension-limit-result">
          <p>
            Limit: <strong>{formatMoney(result.limit, currency)}</strong> (
            {formatPct(result.rate)} of {formatMoney(Number(base), currency)})
          </p>
          <p>
            Deductible: <strong>{formatMoney(result.deductible, currency)}</strong> — room left:{" "}
            {formatMoney(result.remaining, currency)}
          </p>
          {result.excess > 0 && (
            <p className="text-destructive" data-testid="pension-limit-excess">
              Above the limit: {formatMoney(result.excess, currency)} is not deductible.
            </p>
          )}
        </div>
      )}
    </div>
  );
}
