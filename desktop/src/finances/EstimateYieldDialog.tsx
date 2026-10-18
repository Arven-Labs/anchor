import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { BankAccountView, EstimateResult } from "./types";
import Field from "../components/Field";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

// Fase 35, entrega 10 — estima o rendimento de cada resgate do porquinho a partir do que o extrato mostra
// (aplicações e o que o banco pagou) e da Selic meta, e põe uma proposta por resgate na fila de Pending.
// Backend: `commands/porquinho_estimate.rs`.

function EstimateYieldDialog({
  account,
  workspaceId,
  onClose,
}: {
  account: BankAccountView;
  workspaceId: number;
  onClose: () => void;
}) {
  const [pct, setPct] = useState(
    account.auto_yield_pct_of_cdi && account.auto_yield_pct_of_cdi > 0 ? String(Math.round(account.auto_yield_pct_of_cdi * 100)) : "100",
  );
  // Estimativa automática depois da revisão do extrato: `null` = ainda não decidido, `0` = desligada, > 0 = ligada.
  const [autoPct, setAutoPct] = useState<number | null>(account.auto_yield_pct_of_cdi);
  const queryClient = useQueryClient();

  const parsed = Number(pct);
  const valid = pct.trim() !== "" && Number.isFinite(parsed) && parsed > 0 && parsed <= 200;

  const estimateMutation = useMutation<EstimateResult, AppError, void>({
    mutationFn: () =>
      invoke("stage_porquinho_yield_estimate", {
        request: { bank_account_id: account.id, pct_of_cdi: parsed / 100 },
      }),
    onSuccess: () => {
      for (const key of ["statement_pending", "pending-counts"]) {
        queryClient.invalidateQueries({ queryKey: [key] });
      }
      queryClient.invalidateQueries({ queryKey: ["bank-accounts", workspaceId] });
    },
  });
  const result = estimateMutation.data;

  const autoMutation = useMutation<BankAccountView, AppError, { turnOn: boolean }>({
    mutationFn: ({ turnOn }) =>
      invoke("update_bank_account", {
        request: {
          bank_account_id: account.id,
          nome: account.nome,
          titular: account.titular,
          currency: account.currency,
          ...(turnOn ? { auto_yield_pct_of_cdi: parsed / 100 } : { clear_auto_yield: true }),
        },
      }),
    onSuccess: (updated) => {
      setAutoPct(updated.auto_yield_pct_of_cdi);
      queryClient.invalidateQueries({ queryKey: ["bank-accounts", workspaceId] });
    },
  });
  const autoOn = (autoPct ?? 0) > 0;

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md" data-testid="estimate-dialog">
        <DialogHeader>
          <DialogTitle>Estimate yield — {account.nome}</DialogTitle>
        </DialogHeader>

        <div className="flex flex-col gap-4">
          <p className="text-sm text-muted-foreground">
            Estimates the yield of each redemption from the applications and redemptions in this account and the Selic
            target rate (CDI ≈ target − 0.10 p.p.), with IOF and IR. Each estimate goes to Pending for you to review, and
            days that already have a yield are skipped. It is an estimate: the real figure is what the bank paid.
          </p>
          <Field label="Yield as % of the CDI">
            <Input
              type="number"
              step="1"
              min="1"
              max="200"
              data-testid="estimate-pct"
              value={pct}
              onChange={(e) => setPct(e.currentTarget.value)}
            />
          </Field>

          <div className="flex flex-col gap-2 rounded-md border border-border p-3 text-sm" data-testid="estimate-auto">
            <p data-testid="estimate-auto-state">
              {autoOn
                ? `Automatic: on, at ${Math.round((autoPct ?? 0) * 100)}% of the CDI. Right after you finish reviewing a statement, the yield of the redemptions is estimated and sent to Pending.`
                : autoPct === 0
                  ? "Automatic: off. Estimates only run when you press Estimate."
                  : "Automatic: not set yet. It turns on by itself the first time you approve a pocket transfer from a statement."}
            </p>
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="w-fit"
              data-testid="estimate-auto-toggle"
              disabled={autoMutation.isPending || (!autoOn && !valid)}
              onClick={() => autoMutation.mutate({ turnOn: !autoOn })}
            >
              {autoOn ? "Turn off" : "Turn on"}
            </Button>
            {autoMutation.isError && <p className="text-red-600">{autoMutation.error.message}</p>}
          </div>

          {estimateMutation.isError && (
            <p className="text-sm text-red-600" data-testid="estimate-error">
              {estimateMutation.error.message}
            </p>
          )}
          {result && (
            <div className="text-sm" data-testid="estimate-result">
              <p>
                {result.staged} estimate(s) sent to Pending out of {result.redemptions} redemption(s).
              </p>
              {result.skipped_covered_day > 0 && (
                <p className="text-muted-foreground">
                  {result.skipped_covered_day} skipped: the day already has a yield booked or waiting for review.
                </p>
              )}
              {result.skipped_no_yield > 0 && (
                <p className="text-muted-foreground">{result.skipped_no_yield} skipped: no yield to estimate.</p>
              )}
              {result.duplicates > 0 && (
                <p className="text-muted-foreground">{result.duplicates} skipped: same date and amount already known.</p>
              )}
            </div>
          )}

          <div className="flex gap-2">
            <Button
              type="button"
              data-testid="estimate-run"
              disabled={!valid || estimateMutation.isPending}
              onClick={() => estimateMutation.mutate()}
            >
              {estimateMutation.isPending ? "Estimating..." : "Estimate"}
            </Button>
            <Button type="button" variant="outline" onClick={onClose}>
              {result ? "Close" : "Cancel"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

export default EstimateYieldDialog;
