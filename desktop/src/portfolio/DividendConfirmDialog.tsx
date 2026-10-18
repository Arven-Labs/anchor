import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { DividendSuggestionView, PaymentType } from "./types";
import { PAYMENT_TYPES, PAYMENT_TYPE_LABELS } from "./types";
import Field from "../components/Field";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

// Confirmar uma sugestão de provento (quantidade, Data Com e tipo). Usado pela tela de
// proventos e pela aba Pending; o resto dos formulários da sugestão fica em
// `DividendSuggestionsSection`.

export function formatDividendDate(date: string): string {
  const [y, m, d] = date.split("-");
  return `${d}/${m}/${y}`;
}

export function formatDividendValue(value: number): string {
  return value.toFixed(3).replace(/\.?0+$/, "");
}

// Chaves a recarregar quando uma sugestão é confirmada ou descartada.
export function invalidateDividendQueries(
  queryClient: ReturnType<typeof useQueryClient>,
  portfolioId: number,
) {
  queryClient.invalidateQueries({ queryKey: ["dividend_suggestions", portfolioId] });
  queryClient.invalidateQueries({ queryKey: ["transactions", portfolioId] });
  queryClient.invalidateQueries({ queryKey: ["positions", portfolioId] });
  queryClient.invalidateQueries({ queryKey: ["pending-counts"] });
}

function DividendConfirmForm({
  portfolioId,
  suggestion,
  onClose,
}: {
  portfolioId: number;
  suggestion: DividendSuggestionView;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const [quantity, setQuantity] = useState(formatDividendValue(suggestion.quantity));
  const [comDate, setComDate] = useState("");
  const [paymentType, setPaymentType] = useState<PaymentType>(
    (suggestion.payment_type as PaymentType) ?? "dividendo",
  );

  const confirmMutation = useMutation<DividendSuggestionView, AppError, void>({
    mutationFn: () =>
      invoke("confirm_dividend_suggestion", {
        request: {
          suggestion_id: suggestion.id,
          quantity: Number(quantity),
          com_date: comDate === "" ? null : comDate,
          payment_type: paymentType,
        },
      }),
    onSuccess: () => {
      invalidateDividendQueries(queryClient, portfolioId);
      onClose();
    },
  });

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    confirmMutation.mutate();
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4" data-testid="dividend-confirm-form">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <Field label="Ticker">
          <div className="pt-2 text-sm">{suggestion.ticker}</div>
        </Field>
        <Field label="Payment date">
          <div className="pt-2 text-sm">{formatDividendDate(suggestion.payment_date)}</div>
        </Field>
        <Field label="Amount / share">
          <div className="pt-2 text-sm">{formatDividendValue(suggestion.amount)}</div>
        </Field>
        <Field label="Quantity">
          <Input
            type="number"
            step="any"
            min="0"
            value={quantity}
            onChange={(e) => setQuantity(e.target.value)}
            data-testid="dividend-quantity"
            required
          />
        </Field>
        <Field label="Data Com (com record date) — optional">
          <Input type="date" value={comDate} onChange={(e) => setComDate(e.target.value)} />
        </Field>
        <Field label="Payment type">
          <Select value={paymentType} onValueChange={(v) => setPaymentType(v as PaymentType)}>
            <SelectTrigger className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PAYMENT_TYPES.map((key) => (
                <SelectItem key={key} value={key}>
                  {PAYMENT_TYPE_LABELS[key]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <Field label="Total">
          <div className="pt-2 text-sm">{(Number(quantity || 0) * suggestion.amount).toFixed(2)}</div>
        </Field>
      </div>
      {confirmMutation.isError && <p className="text-sm text-red-600">{confirmMutation.error.message}</p>}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="outline" onClick={onClose}>
          Cancel
        </Button>
        <Button
          type="submit"
          data-testid="dividend-confirm-submit"
          disabled={confirmMutation.isPending || Number(quantity) <= 0}
        >
          {confirmMutation.isPending ? "Confirming…" : "Confirm"}
        </Button>
      </div>
    </form>
  );
}

function DividendConfirmDialog({
  portfolioId,
  suggestion,
  onClose,
}: {
  portfolioId: number;
  suggestion: DividendSuggestionView | null;
  onClose: () => void;
}) {
  return (
    <Dialog open={suggestion !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Confirm dividend</DialogTitle>
        </DialogHeader>
        {/* `key` zera o formulário a cada sugestão aberta. */}
        {suggestion && (
          <DividendConfirmForm key={suggestion.id} portfolioId={portfolioId} suggestion={suggestion} onClose={onClose} />
        )}
      </DialogContent>
    </Dialog>
  );
}

export default DividendConfirmDialog;
