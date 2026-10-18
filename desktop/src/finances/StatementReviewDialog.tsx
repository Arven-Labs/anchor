import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
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

// Rendimento de renda fixa derivado da Movimentação da B3 (entrega 4): o bruto vem do
// valor do resgate, o IR da taxa por papel em Settings → Taxes.
export type YieldDetail = {
  instrument: string;
  code: string;
  quantity: number;
  principal: number;
  gross_value: number;
  gross_yield: number;
  iof_amount: number;
  holding_days: number | null;
  // `full`, `partial`, `unknown` ou `not_applicable`: de onde veio o IOF.
  iof_basis: "full" | "partial" | "unknown" | "not_applicable";
  ir_rate: number;
  ir_amount: number;
  net_yield: number;
  base_unit_price: number;
  base_price_source: string;
  day_derived_total: number;
  bank_total_that_day: number | null;
  // Só nas estimativas do porquinho (`base_price_source === "estimate"`): fração do CDI usada.
  estimate_pct_of_cdi: number | null;
};

export type StatementPendingItem = {
  id: number;
  kind: "bank_transaction" | "fixed_income_yield";
  bank_account_id: number;
  bank_account_name: string;
  counterpart_account_id: number | null;
  counterpart_account_name: string | null;
  source: string;
  created_at: string;
  proposed: ProposedBankTransaction;
  suggested_type: "receita" | "despesa" | "transferencia" | "rendimento";
  fixed_income: YieldDetail | null;
};

export const STATEMENT_PENDING_KEY = "statement_pending";

const NO_CATEGORY = "none";
const TYPE_LABELS: Record<string, string> = { receita: "Income", despesa: "Expense" };

function formatMoney(value: number): string {
  return value.toFixed(2);
}

function refreshAfterReview(queryClient: QueryClient, workspaceId: number) {
  queryClient.invalidateQueries({ queryKey: [STATEMENT_PENDING_KEY, workspaceId] });
  for (const key of ["pending-counts", "general-transactions", "bank-accounts", "net-worth", "workspace-summary", "general-transaction-categories"]) {
    queryClient.invalidateQueries({ queryKey: [key] });
  }
}

// Compara o que o extrato já aprovado mostra saindo do porquinho no dia com o que o
// arquivo da B3 explica. A diferença costuma ser IOF ou uma faixa de IR maior que a
// taxa fixa; corrija o IR abaixo para o banco e o app concordarem.
function dayCheck(detail: YieldDetail): { text: string; matches: boolean } | null {
  if (detail.bank_total_that_day === null) return null;
  const diff = detail.bank_total_that_day - detail.day_derived_total;
  if (Math.abs(diff) < 0.005) {
    return { text: `The statement shows R$ ${formatMoney(detail.bank_total_that_day)} leaving the pocket that day, exactly what the B3 file explains.`, matches: true };
  }
  return {
    text:
      `The statement shows R$ ${formatMoney(detail.bank_total_that_day)} leaving the pocket that day, but the B3 file explains ` +
      `R$ ${formatMoney(detail.day_derived_total)} (difference R$ ${formatMoney(diff)}). ` +
      (diff < 0
        ? "The bank paid less: more tax was withheld than computed here (for example IOF on units whose application is not in the file, or a higher IR bracket)."
        : "The bank paid more: less tax was withheld than computed here."),
    matches: false,
  };
}

// De onde veio o IOF proposto, em uma frase.
function iofNote(detail: YieldDetail): string {
  if (detail.base_price_source === "estimate") {
    return detail.iof_basis === "partial"
      ? "Estimated from the applications in the account's statement; part of this redemption has no application there, so that part has no yield or IOF here."
      : `Estimated from the applications in the account's statement (about ${detail.holding_days ?? 0} day(s) held); IOF applies to redemptions within 30 days.`;
  }
  switch (detail.iof_basis) {
    case "full":
      return `Held ${detail.holding_days ?? 0} day(s): IOF applies to redemptions within 30 days.`;
    case "partial":
      return `Only part of the units has its application in the file (about ${detail.holding_days ?? 0} day(s)); the rest has no IOF here. Enter it if the bank paid less.`;
    case "unknown":
      return "This CDB has no application in the file, so the holding time is unknown and the IOF is 0 here. Enter it if the bank paid less.";
    default:
      return "No IOF for this instrument.";
  }
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
    refreshAfterReview(queryClient, workspaceId);
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

function YieldReviewForm({
  workspaceId,
  item,
  detail,
  total,
  onSkip,
}: {
  workspaceId: number;
  item: StatementPendingItem;
  detail: YieldDetail;
  total: number;
  onSkip: () => void;
}) {
  const queryClient = useQueryClient();
  const [date, setDate] = useState(item.proposed.transaction_date);
  const [gross, setGross] = useState(detail.gross_yield.toString());
  const [iof, setIof] = useState(detail.iof_amount.toString());
  const [ir, setIr] = useState(detail.ir_amount.toString());
  const [notes, setNotes] = useState("");
  const [error, setError] = useState<string | null>(null);

  const net = Number(gross) - Number(iof) - Number(ir);
  const invalid =
    [gross, iof, ir].some((v) => v.trim() === "" || Number.isNaN(Number(v))) ||
    Number(gross) <= 0 || Number(iof) < 0 || Number(ir) < 0 || Number(iof) + Number(ir) > Number(gross);
  const check = dayCheck(detail);

  const confirmMutation = useMutation<void, AppError, void>({
    mutationFn: () =>
      invoke("confirm_yield_pending", {
        workspaceId,
        id: item.id,
        edited: {
          transaction_date: date,
          gross_yield: Number(gross),
          iof_amount: Number(iof),
          ir_amount: Number(ir),
          notes: notes.trim() === "" ? null : notes.trim(),
        },
      }),
    onSuccess: () => {
      setError(null);
      refreshAfterReview(queryClient, workspaceId);
    },
    onError: (err) => setError(err.message),
  });

  const discardMutation = useMutation<void, AppError, void>({
    mutationFn: () => invoke("discard_statement_pending", { workspaceId, id: item.id }),
    onSuccess: () => {
      setError(null);
      refreshAfterReview(queryClient, workspaceId);
    },
    onError: (err) => setError(err.message),
  });

  const busy = confirmMutation.isPending || discardMutation.isPending;

  return (
    <div className="flex flex-col gap-4" data-testid="statement-review-form">
      <p className="text-sm text-muted-foreground" data-testid="statement-review-counter">
        {total} left to review · line {item.proposed.line} · {item.source}
      </p>
      {detail.base_price_source === "estimate" ? (
        <p className="text-sm" data-testid="yield-original">
          <span className="font-medium">{detail.instrument.toUpperCase()} {detail.code}</span> · estimated from the CDI curve at{" "}
          {Math.round((detail.estimate_pct_of_cdi ?? 1) * 100)}% of the CDI: a principal of {formatMoney(detail.principal)} redeemed
          for {formatMoney(detail.gross_value)} before tax. This is an estimate; correct it if the bank paid differently.
        </p>
      ) : (
        <p className="text-sm" data-testid="yield-original">
          <span className="font-medium">{detail.instrument.toUpperCase()} {detail.code}</span> · redeemed{" "}
          {formatMoney(detail.gross_value)} for a principal of {formatMoney(detail.principal)} ({detail.quantity} units at{" "}
          {detail.base_unit_price}, {detail.base_price_source === "application" ? "from its application" : "common to all applications in the file"})
        </p>
      )}

      <div className="grid grid-cols-2 gap-3">
        <Field label="Date">
          <Input value={date} onChange={(e) => setDate(e.target.value)} placeholder="YYYY-MM-DD" data-testid="yield-date" />
        </Field>
        <Field label={`Gross yield (income) — ${item.bank_account_name}`}>
          <Input inputMode="decimal" value={gross} onChange={(e) => setGross(e.target.value)} data-testid="yield-gross" />
        </Field>
        <Field label="IOF withheld (expense in Taxes)">
          <Input inputMode="decimal" value={iof} onChange={(e) => setIof(e.target.value)} data-testid="yield-iof" />
        </Field>
        <Field label={`IR withheld (${(detail.ir_rate * 100).toFixed(1)}% of the yield after IOF)`}>
          <Input inputMode="decimal" value={ir} onChange={(e) => setIr(e.target.value)} data-testid="yield-ir" />
        </Field>
        <Field label="Net yield (what stays in the account)">
          <p className="py-2 text-sm font-medium" data-testid="yield-net">
            {invalid ? "—" : formatMoney(net)}
          </p>
        </Field>
        <Field label="Notes" className="col-span-2">
          <Input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Optional" data-testid="yield-notes" />
        </Field>
      </div>

      <p className="text-xs text-muted-foreground" data-testid="yield-iof-note">
        {iofNote(detail)}
      </p>
      {check && (
        <p className={check.matches ? "text-sm text-muted-foreground" : "text-sm text-amber-600"} data-testid="yield-check">
          {check.text}
        </p>
      )}
      {error && (
        <p className="text-sm text-destructive" data-testid="statement-error">
          {error}
        </p>
      )}

      <div className="flex flex-wrap gap-2">
        <Button type="button" data-testid="statement-correct" disabled={busy || invalid} onClick={() => confirmMutation.mutate()}>
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
        {current && current.fixed_income && (
          <YieldReviewForm
            key={current.id}
            workspaceId={workspaceId}
            item={current}
            detail={current.fixed_income}
            total={ordered.length}
            onSkip={skipCurrent}
          />
        )}
        {current && !current.fixed_income && (
          <ReviewForm key={current.id} workspaceId={workspaceId} item={current} total={ordered.length} onSkip={skipCurrent} />
        )}
      </DialogContent>
    </Dialog>
  );
}

export default StatementReviewDialog;
