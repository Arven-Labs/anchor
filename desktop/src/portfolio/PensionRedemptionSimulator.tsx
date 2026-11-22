import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation } from "@tanstack/react-query";
import type { AppError } from "../types";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// Previdência privada: estimativa informativa do IR no resgate, regime com tabela por prazo (Lei 11.053/2004, art. 1º).
// Nada é gravado e nada entra na apuração de IR, no patrimônio nem na Rentabilidade. As alíquotas vêm de `tax_rules`
// (editáveis em Settings → Taxes); o rendimento do VGBL e o prazo são informados pelo usuário (o informe da seguradora
// traz os dois), porque a fonte não diz como separar principal de rendimento num resgate parcial.
type PensionKind = "pgbl" | "vgbl";

type PensionRedemptionEstimate = {
  rule_key: string;
  rate: number;
  taxable_base: number;
  estimated_tax: number;
  net_amount: number;
  advance_rate: number;
};

function formatMoney(value: number, currency: string): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency });
}

function formatPct(value: number): string {
  return `${(value * 100).toFixed(2)}%`;
}

export default function PensionRedemptionSimulator({ currency }: { currency: string }) {
  const [kind, setKind] = useState<PensionKind>("pgbl");
  const [redeemed, setRedeemed] = useState("");
  const [income, setIncome] = useState("");
  const [years, setYears] = useState("");

  const estimateMutation = useMutation<PensionRedemptionEstimate, AppError, void>({
    mutationFn: () =>
      invoke("estimate_pension_redemption", {
        request: {
          kind,
          redeemed: Number(redeemed),
          income: kind === "vgbl" ? Number(income) : null,
          years_since_contribution: Number(years),
        },
      }),
  });

  const result = estimateMutation.data;
  // O backend também recusa rendimento maior que o resgate, mas com a mensagem crua do erro; barramos aqui para o aviso ser claro.
  const incomeTooHigh =
    kind === "vgbl" && redeemed !== "" && income !== "" && Number(income) > Number(redeemed);
  const canSubmit =
    Number(redeemed) > 0 &&
    years !== "" &&
    Number(years) >= 0 &&
    (kind === "pgbl" || income !== "") &&
    !incomeTooHigh;

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    estimateMutation.mutate();
  }

  return (
    <div data-testid="pension-redemption-simulator">
      <h3 className="mb-2 text-sm font-semibold">Simulate redemption</h3>
      <p className="mb-3 text-sm text-muted-foreground">
        Estimate only: the insurer or pension entity withholds the tax on the redemption, and
        nothing here is saved or counted in your tax assessment. It uses the table regime (Lei
        11.053/2004, art. 1º), with rates by how long the contribution was held; you can edit
        them in Settings → Taxes. For a partial redemption, enter the age of the contribution and,
        for VGBL, the income part from the statement.
      </p>
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-4">
          <Field label="Plan type">
            <Select value={kind} onValueChange={(value) => setKind(value as PensionKind)}>
              <SelectTrigger className="w-full" data-testid="pension-kind">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="pgbl">PGBL</SelectItem>
                <SelectItem value="vgbl">VGBL</SelectItem>
              </SelectContent>
            </Select>
          </Field>
          <Field label="Amount redeemed">
            <Input
              required
              data-testid="pension-redeemed"
              type="number"
              step="any"
              min="0"
              value={redeemed}
              onChange={(e) => setRedeemed(e.currentTarget.value)}
            />
          </Field>
          {kind === "vgbl" && (
            <Field label="Of which income">
              <Input
                required
                data-testid="pension-income"
                type="number"
                step="any"
                min="0"
                value={income}
                onChange={(e) => setIncome(e.currentTarget.value)}
              />
            </Field>
          )}
          <Field label="Years since the contribution">
            <Input
              required
              data-testid="pension-years"
              type="number"
              step="any"
              min="0"
              value={years}
              onChange={(e) => setYears(e.currentTarget.value)}
            />
          </Field>
        </div>
        <Button
          type="submit"
          className="w-fit"
          data-testid="pension-submit"
          disabled={!canSubmit || estimateMutation.isPending}
        >
          {estimateMutation.isPending ? "Estimating..." : "Estimate"}
        </Button>
      </form>
      {incomeTooHigh && (
        <p className="mt-3 text-sm text-destructive" data-testid="pension-income-error">
          The income part can't be greater than the amount redeemed.
        </p>
      )}
      {estimateMutation.isError && (
        <p className="mt-3 text-sm text-destructive">{estimateMutation.error.message}</p>
      )}
      {result && (
        <div className="mt-3 flex flex-col gap-1 text-sm" data-testid="pension-result">
          <p>
            Rate: <strong>{formatPct(result.rate)}</strong> on{" "}
            {formatMoney(result.taxable_base, currency)}
            {kind === "vgbl" ? " (the income only)" : " (the whole redemption)"}
          </p>
          <p>
            Estimated tax: <strong>{formatMoney(result.estimated_tax, currency)}</strong> — you would
            receive about {formatMoney(result.net_amount, currency)}
          </p>
          <p className="text-muted-foreground">
            If you did not opt for the table regime, {formatPct(result.advance_rate)} is withheld as
            an advance and the final tax is settled in the annual return; that is not calculated
            here.
          </p>
        </div>
      )}
    </div>
  );
}
