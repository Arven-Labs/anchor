import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { GeneralTransactionCategory } from "./types";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

// Fase 35, entrega 3 — revisão item a item do extrato bancário importado. Mesma
// mecânica do popup de carteira (`portfolio/ImportReviewDialog.tsx`): "Correct"
// grava com o que está na tela, "Discard" recusa de vez, "Skip" deixa para
// depois, e fechar o popup deixa o resto na fila de pendentes.

export type ProposedBankTransaction = {
  line: number;
  transaction_date: string;
  valor: number;
  label: string;
  description: string;
  reference: string | null;
  porquinho: boolean;
};

export type StatementPendingItem = {
  id: number;
  bank_account_id: number;
  bank_account_name: string;
  counterpart_account_id: number | null;
  counterpart_account_name: string | null;
  source: string;
  created_at: string;
  proposed: ProposedBankTransaction;
  suggested_type: "receita" | "despesa" | "transferencia";
};

export const STATEMENT_PENDING_KEY = "statement_pending";

const NO_CATEGORY = "none";
const TYPE_LABELS: Record<string, string> = { receita: "Income", despesa: "Expense" };

function formatMoney(value: number): string {
  return value.toFixed(2);
}

function ReviewForm({
  workspaceId,
  item,
  total,
  onSkip,
}: {
  workspaceId: number;
  item: StatementPendingItem;
  total: number;
  onSkip: () => void;
}) {
  const queryClient = useQueryClient();
  const p = item.proposed;
  const isTransfer = item.suggested_type === "transferencia";
  const [date, setDate] = useState(p.transaction_date);
  const [type, setType] = useState<string>(isTransfer ? "receita" : item.suggested_type);
  const [valor, setValor] = useState(Math.abs(p.valor).toString());
  const [categoryId, setCategoryId] = useState(NO_CATEGORY);
  const [notes, setNotes] = useState(p.description);
  const [error, setError] = useState<string | null>(null);

  const categoriesQuery = useQuery<GeneralTransactionCategory[], AppError>({
    queryKey: ["general-transaction-categories", workspaceId],
    queryFn: () => invoke("list_general_transaction_categories", { workspaceId }),
  });
  const categories = categoriesQuery.data ?? [];

  function refresh() {
    queryClient.invalidateQueries({ queryKey: [STATEMENT_PENDING_KEY, workspaceId] });
    for (const key of ["general-transactions", "bank-accounts", "net-worth", "workspace-summary"]) {
      queryClient.invalidateQueries({ queryKey: [key] });
    }
  }

  const confirmMutation = useMutation<void, AppError, void>({
    mutationFn: () =>
      invoke("confirm_statement_pending", {
        workspaceId,
        id: item.id,
        edited: {
          transaction_date: date,
          transaction_type: type,
          valor: Number(valor),
          notes: notes.trim() === "" ? null : notes.trim(),
          category_id: categoryId === NO_CATEGORY ? null : Number(categoryId),
        },
      }),
    onSuccess: () => {
      setError(null);
      refresh();
    },
    onError: (err) => setError(err.message),
  });

  const discardMutation = useMutation<void, AppError, void>({
    mutationFn: () => invoke("discard_statement_pending", { workspaceId, id: item.id }),
    onSuccess: () => {
      setError(null);
      refresh();
    },
    onError: (err) => setError(err.message),
  });

  const busy = confirmMutation.isPending || discardMutation.isPending;
  const valorInvalid = valor.trim() === "" || Number.isNaN(Number(valor)) || Number(valor) <= 0;
  const [from, to] =
    p.valor < 0
      ? [item.bank_account_name, item.counterpart_account_name]
      : [item.counterpart_account_name, item.bank_account_name];

  return (
    <div className="flex flex-col gap-4" data-testid="statement-review-form">
      <p className="text-sm text-muted-foreground" data-testid="statement-review-counter">
        {total} left to review · line {p.line} · {item.source}
      </p>
      <p className="text-sm" data-testid="statement-review-original">
        <span className="font-medium">{p.label}</span> · {p.description || "—"} · {formatMoney(p.valor)}
      </p>

      <div className="grid grid-cols-2 gap-3">
        <Field label="Date">
          <Input value={date} onChange={(e) => setDate(e.target.value)} placeholder="YYYY-MM-DD" data-testid="statement-date" />
        </Field>
        <Field label="Amount">
          <Input inputMode="decimal" value={valor} onChange={(e) => setValor(e.target.value)} data-testid="statement-valor" />
        </Field>

        {isTransfer ? (
          <Field label="Transfer" className="col-span-2">
            <p className="text-sm" data-testid="statement-transfer">
              {from} → {to}
            </p>
          </Field>
        ) : (
          <>
            <Field label="Type">
              <Select value={type} onValueChange={setType}>
                <SelectTrigger className="w-full" data-testid="statement-type">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {Object.entries(TYPE_LABELS).map(([value, label]) => (
                    <SelectItem key={value} value={value}>
                      {label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <Field label="Category">
              <Select value={categoryId} onValueChange={setCategoryId}>
                <SelectTrigger className="w-full" data-testid="statement-category">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={NO_CATEGORY}>No category</SelectItem>
                  {categories.map((c) => (
                    <SelectItem key={c.id} value={String(c.id)}>
                      {c.nome}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
          </>
        )}

        <Field label="Notes" className="col-span-2">
          <Input value={notes} onChange={(e) => setNotes(e.target.value)} data-testid="statement-notes" />
        </Field>
      </div>

      {error && (
        <p className="text-sm text-destructive" data-testid="statement-error">
          {error}
        </p>
      )}

      <div className="flex flex-wrap gap-2">
        <Button type="button" data-testid="statement-correct" disabled={busy || valorInvalid} onClick={() => confirmMutation.mutate()}>
          Correct
        </Button>
        <Button type="button" variant="outline" data-testid="statement-discard" disabled={busy} onClick={() => discardMutation.mutate()}>
          Discard
        </Button>
        <Button type="button" variant="ghost" data-testid="statement-skip" disabled={busy} onClick={onSkip}>
          Skip
        </Button>
      </div>
    </div>
  );
}

function StatementReviewDialog({
  workspaceId,
  open,
  onClose,
}: {
  workspaceId: number;
  open: boolean;
  onClose: () => void;
}) {
  const [skipped, setSkipped] = useState<number[]>([]);

  const pendingQuery = useQuery<StatementPendingItem[], AppError>({
    queryKey: [STATEMENT_PENDING_KEY, workspaceId],
    queryFn: () => invoke("list_statement_pending", { workspaceId }),
    enabled: open,
  });

  // Os pulados vão para o fim da volta, sem sair da fila.
  const ordered = useMemo(() => {
    const items = pendingQuery.data ?? [];
    return [...items.filter((i) => !skipped.includes(i.id)), ...items.filter((i) => skipped.includes(i.id))];
  }, [pendingQuery.data, skipped]);
  const current = ordered[0];

  function skipCurrent() {
    if (!current) return;
    setSkipped((prev) => [...prev.filter((id) => id !== current.id), current.id]);
  }

  function handleOpenChange(next: boolean) {
    if (!next) {
      setSkipped([]);
      onClose();
    }
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-xl" data-testid="statement-review-dialog">
        <DialogHeader>
          <DialogTitle>Review imported statement</DialogTitle>
        </DialogHeader>
        {pendingQuery.isLoading && <p className="text-sm">Loading…</p>}
        {pendingQuery.isError && <p className="text-sm text-destructive">{pendingQuery.error.message}</p>}
        {!pendingQuery.isLoading && !current && (
          <div className="flex flex-col gap-3" data-testid="statement-review-empty">
            <p className="text-sm text-muted-foreground">Nothing left to review.</p>
            <Button type="button" onClick={() => handleOpenChange(false)}>
              Close
            </Button>
          </div>
        )}
        {current && (
          <ReviewForm key={current.id} workspaceId={workspaceId} item={current} total={ordered.length} onSkip={skipCurrent} />
        )}
      </DialogContent>
    </Dialog>
  );
}

export default StatementReviewDialog;
