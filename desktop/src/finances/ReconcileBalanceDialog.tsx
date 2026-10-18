import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { BankAccountView, ReconcilePreview } from "./types";
import Field from "../components/Field";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

// Fase 35, entrega 9 — o saldo do Porquinho sai só dos fluxos do extrato e não tem o rendimento. Aqui o
// usuário digita o saldo que o app do banco mostra numa data; a diferença contra o saldo do ledger
// vira uma receita (backend: `commands/balance_reconciliation.rs`).

function today(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

function ReconcileBalanceDialog({
  account,
  workspaceId,
  onClose,
}: {
  account: BankAccountView;
  workspaceId: number;
  onClose: () => void;
}) {
  const [balance, setBalance] = useState("");
  const [date, setDate] = useState(today());
  // Ajuste de saldo (conta Corretora): a diferença pode ir para cima ou para baixo e não é rendimento.
  const [adjustment, setAdjustment] = useState(false);
  const queryClient = useQueryClient();

  const parsedBalance = balance.trim() === "" ? null : Number(balance);
  const ready = parsedBalance !== null && Number.isFinite(parsedBalance) && date !== "";

  const previewQuery = useQuery<ReconcilePreview, AppError>({
    queryKey: ["reconcile-preview", account.id, parsedBalance, date, adjustment],
    queryFn: () =>
      invoke("preview_balance_reconciliation", {
        request: { bank_account_id: account.id, bank_balance: parsedBalance, date, adjustment },
      }),
    enabled: ready,
    retry: false,
  });
  const preview = ready ? previewQuery.data : undefined;

  const confirmMutation = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("confirm_balance_reconciliation", {
        request: { bank_account_id: account.id, bank_balance: parsedBalance, date, adjustment },
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["bank-accounts", workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["general-transactions", workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["net-worth", workspaceId] });
      onClose();
    },
  });

  const canBook = preview !== undefined && (adjustment ? preview.difference !== 0 : preview.difference > 0);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md" data-testid="reconcile-dialog">
        <DialogHeader>
          <DialogTitle>Reconcile balance — {account.nome}</DialogTitle>
        </DialogHeader>

        <div className="flex flex-col gap-4">
          <p className="text-sm text-muted-foreground">
            {adjustment
              ? "Enter the balance your broker or bank shows on a date. The difference against the balance built from the transactions is booked as income or an expense: use it for an opening balance or cash the spreadsheets do not carry."
              : "Enter the balance your bank shows for this account on a date. The difference against the balance built from the transactions is booked as yield, including yield not redeemed yet."}
          </p>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              data-testid="reconcile-adjustment"
              checked={adjustment}
              onChange={(e) => setAdjustment(e.currentTarget.checked)}
            />
            Balance adjustment (not yield, can go down)
          </label>
          <Field label={`Bank balance (${account.currency})`}>
            <Input
              type="number"
              step="0.01"
              min="0"
              data-testid="reconcile-balance"
              value={balance}
              onChange={(e) => setBalance(e.currentTarget.value)}
            />
          </Field>
          <Field label="Balance date">
            <Input
              type="date"
              data-testid="reconcile-date"
              value={date}
              max={today()}
              onChange={(e) => setDate(e.currentTarget.value)}
            />
          </Field>

          {ready && previewQuery.isError && (
            <p className="text-sm text-red-600" data-testid="reconcile-error">
              {previewQuery.error.message}
            </p>
          )}
          {preview && (
            <dl className="grid grid-cols-2 gap-1 text-sm">
              <dt className="text-muted-foreground">Ledger balance on that date</dt>
              <dd data-testid="reconcile-ledger">{preview.ledger_balance.toFixed(2)}</dd>
              <dt className="text-muted-foreground">{adjustment ? "Adjustment to book" : "Yield to book"}</dt>
              <dd data-testid="reconcile-difference">{preview.difference.toFixed(2)}</dd>
            </dl>
          )}
          {preview && preview.difference < 0 && !adjustment && (
            <p className="text-sm text-red-600" data-testid="reconcile-negative">
              The bank balance is lower than the ledger balance. A withdrawal may be missing from the statement,
              or a yield was approved for too much.
            </p>
          )}
          {preview && preview.difference === 0 && (
            <p className="text-sm text-muted-foreground">The ledger already matches the bank balance.</p>
          )}
          {confirmMutation.isError && <p className="text-sm text-red-600">{confirmMutation.error.message}</p>}

          <div className="flex gap-2">
            <Button
              type="button"
              data-testid="reconcile-confirm"
              disabled={!canBook || confirmMutation.isPending}
              onClick={() => confirmMutation.mutate()}
            >
              {confirmMutation.isPending ? "Booking..." : adjustment ? "Book adjustment" : "Book yield"}
            </Button>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

export default ReconcileBalanceDialog;
