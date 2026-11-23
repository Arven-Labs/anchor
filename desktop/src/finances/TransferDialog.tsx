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

// Custos digitados (IOF, tarifa, remessa): debitados da conta de origem, na moeda dela, além do
// valor enviado. Se o extrato já embute a tarifa no valor recebido, não digite de novo.
type CostKind = "iof" | "fee";

// `rate` é a taxa de IOF em % digitada pelo usuário, só para sugerir o valor; não é enviada ao backend.
type CostDraft = { kind: CostKind; valor: string; rate: string };

// Sugestão de IOF: valor enviado × taxa ÷ 100, arredondado ao centavo. O app não afirma alíquota nenhuma.
function suggestIof(amountSent: number, ratePercent: number): string {
  return (Math.round(((amountSent * ratePercent) / 100) * 100) / 100).toFixed(2);
}

type CreateTransferRequest = {
  workspace_id: number;
  from_account_id: number;
  to_account_id: number;
  valor_saida: number;
  valor_entrada: number;
  transaction_date: string;
  notes: string | null;
  costs: { kind: CostKind; valor: number; notes: string | null }[];
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
  const [costs, setCosts] = useState<CostDraft[]>([]);

  const from =accounts.find((a) => String(a.id) === fromId);
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
      setCosts([]);
      onOpenChange(false);
    },
  });

  function updateCost(index: number, patch: Partial<CostDraft>) {
    setCosts((current) => current.map((c, i) => (i === index ? { ...c, ...patch } : c)));
  }

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
      costs: costs.map((c) => ({ kind: c.kind, valor: Number(c.valor), notes: null })),
    });
  }

  const costsValid = costs.every((c) => Number(c.valor) > 0);
  const canSubmit =
    from !== undefined &&
    to !== undefined &&
    fromId !== toId &&
    out > 0 &&
    incoming > 0 &&
    costsValid;

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

          <div className="flex flex-col gap-2" data-testid="transfer-costs">
            <p className="text-sm text-muted-foreground">
              Costs{from ? ` (${from.currency}, charged to the origin account)` : ""}: IOF, fees or
              remittance. Skip what your statement already shows inside the received amount.
            </p>
            {costs.map((cost, index) => (
              <div key={index} className="flex items-center gap-2">
                <Select
                  value={cost.kind}
                  onValueChange={(value) => updateCost(index, { kind: value as CostKind })}
                >
                  <SelectTrigger className="w-40" data-testid={`transfer-cost-kind-${index}`}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="iof">IOF</SelectItem>
                    <SelectItem value="fee">Fee / remittance</SelectItem>
                  </SelectContent>
                </Select>
                {cost.kind === "iof" && (
                  <>
                    <Input
                      type="number"
                      step="any"
                      min="0"
                      className="w-28"
                      placeholder="IOF rate (%)"
                      data-testid={`transfer-cost-rate-${index}`}
                      value={cost.rate}
                      onChange={(e) => updateCost(index, { rate: e.currentTarget.value })}
                    />
                    <Button
                      type="button"
                      variant="outline"
                      data-testid={`transfer-cost-calc-${index}`}
                      disabled={!(out > 0) || !(Number(cost.rate) > 0)}
                      onClick={() =>
                        updateCost(index, { valor: suggestIof(out, Number(cost.rate)) })
                      }
                    >
                      Calculate
                    </Button>
                  </>
                )}
                <Input
                  required
                  type="number"
                  step="any"
                  min="0"
                  placeholder="Amount"
                  data-testid={`transfer-cost-valor-${index}`}
                  value={cost.valor}
                  onChange={(e) => updateCost(index, { valor: e.currentTarget.value })}
                />
                <Button
                  type="button"
                  variant="outline"
                  data-testid={`transfer-cost-remove-${index}`}
                  onClick={() => setCosts((current) => current.filter((_, i) => i !== index))}
                >
                  Remove
                </Button>
              </div>
            ))}
            {from !== undefined && from.currency !== "BRL" && costs.some((c) => c.kind === "iof") && (
              <p className="text-xs text-muted-foreground" data-testid="transfer-iof-base-hint">
                The suggestion uses the amount sent ({from.currency}). IOF applies to the value of
                the exchange in BRL, so adjust the amount if your statement differs.
              </p>
            )}
            <Button
              type="button"
              variant="outline"
              className="w-fit"
              data-testid="transfer-cost-add"
              onClick={() =>
                setCosts((current) => [...current, { kind: "iof", valor: "", rate: "" }])
              }
            >
              Add cost
            </Button>
          </div>

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
