import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { BankAccountView } from "./types";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import Field from "../components/Field";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// Fase 19 — transferência entre contas. Mesma moeda: um valor só (o que sai é o
// que chega). Moedas diferentes: o usuário digita o que saiu e o que chegou, como
// no extrato, e a taxa efetiva é só informativa (spread e tarifa já embutidos).

type TransferResult = {
  transfer_id: number;
  from_currency: string;
  to_currency: string;
  effective_rate: number | null;
};

type CreateTransferRequest = {
  workspace_id: number;
  from_account_id: number;
  to_account_id: number;
  valor_saida: number;
  valor_entrada: number;
  transaction_date: string;
  notes: string | null;
};

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

function accountOption(account: BankAccountView): string {
  return `${account.nome} (${account.titular}) · ${account.currency}`;
}

function TransferDialog({
  workspaceId,
  accounts,
  open,
  onOpenChange,
  onDone,
}: {
  workspaceId: number;
  accounts: BankAccountView[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDone: () => void;
}) {
  const [fromId, setFromId] = useState("");
  const [toId, setToId] = useState("");
  const [amountOut, setAmountOut] = useState("");
  const [amountIn, setAmountIn] = useState("");
  const [date, setDate] = useState(todayIso());
  const [notes, setNotes] = useState("");

  const from = accounts.find((a) => String(a.id) === fromId);
  const to = accounts.find((a) => String(a.id) === toId);
  const crossCurrency = from !== undefined && to !== undefined && from.currency !== to.currency;
  const out = Number(amountOut);
  const incoming = crossCurrency ? Number(amountIn) : out;
  const rate = crossCurrency && out > 0 && incoming > 0 ? incoming / out : null;

  const mutation = useMutation<TransferResult, AppError, CreateTransferRequest>({
    mutationFn: (request) => invoke("create_transfer", { request }),
    onSuccess: () => {
      onDone();
      setFromId("");
      setToId("");
      setAmountOut("");
      setAmountIn("");
      setNotes("");
      onOpenChange(false);
    },
  });

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    mutation.mutate({
      workspace_id: workspaceId,
      from_account_id: Number(fromId),
      to_account_id: Number(toId),
      valor_saida: out,
      valor_entrada: incoming,
      transaction_date: date,
      notes: notes.trim() === "" ? null : notes,
    });
  }

  const canSubmit =
    from !== undefined && to !== undefined && fromId !== toId && out > 0 && incoming > 0;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>New transfer</DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <Field label="From account">
              <Select value={fromId} onValueChange={setFromId}>
                <SelectTrigger className="w-full" data-testid="transfer-from">
                  <SelectValue placeholder="Select an account" />
                </SelectTrigger>
                <SelectContent>
                  {accounts.map((account) => (
                    <SelectItem key={account.id} value={String(account.id)}>
                      {accountOption(account)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <Field label="To account">
              <Select value={toId} onValueChange={setToId}>
                <SelectTrigger className="w-full" data-testid="transfer-to">
                  <SelectValue placeholder="Select an account" />
                </SelectTrigger>
                <SelectContent>
                  {accounts.map((account) => (
                    <SelectItem key={account.id} value={String(account.id)}>
                      {accountOption(account)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <Field label={crossCurrency && from ? `Amount sent (${from.currency})` : "Amount"}>
              <Input
                required
                type="number"
                step="any"
                min="0"
                data-testid="transfer-amount-out"
                value={amountOut}
                onChange={(e) => setAmountOut(e.currentTarget.value)}
              />
            </Field>
            {crossCurrency && to && (
              <Field label={`Amount received (${to.currency})`}>
                <Input
                  required
                  type="number"
                  step="any"
                  min="0"
                  data-testid="transfer-amount-in"
                  value={amountIn}
                  onChange={(e) => setAmountIn(e.currentTarget.value)}
                />
              </Field>
            )}
          </div>

          {rate !== null && from && to && (
            <p className="text-sm text-muted-foreground" data-testid="transfer-rate">
              Effective rate: 1 {from.currency} = {rate.toFixed(4)} {to.currency}
            </p>
          )}

          <Field label="Date">
            <Input
              required
              type="date"
              value={date}
              onChange={(e) => setDate(e.currentTarget.value)}
            />
          </Field>
          <Field label="Notes (optional)">
            <Input value={notes} onChange={(e) => setNotes(e.currentTarget.value)} />
          </Field>

          {mutation.isError && <p className="text-red-600">{mutation.error.message}</p>}

          <Button
            type="submit"
            data-testid="transfer-submit"
            disabled={!canSubmit || mutation.isPending}
            className="w-fit"
          >
            {mutation.isPending ? "Transferring..." : "Transfer"}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export default TransferDialog;
