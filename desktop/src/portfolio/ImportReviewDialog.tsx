import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import {
  ASSET_CLASSES,
  ASSET_CLASS_LABELS,
  TRANSACTION_TYPES,
  TRANSACTION_TYPE_LABELS,
  type AssetClass,
  type TransactionType,
} from "./types";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

// Fase 35 — revisão item a item do que uma importação propôs. Cada pendente
// aparece num popup editável: "Correct" grava o lançamento com o que está na
// tela, "Discard" recusa de vez (reimportar não traz de volta) e "Skip" deixa
// para depois. Fechar o popup não perde nada: o que não foi decidido continua
// na fila de pendentes.

export type ProposedTransaction = {
  line: number;
  transaction_date: string;
  transaction_type: string;
  ticker: string;
  asset_class: string | null;
  currency: string | null;
  quantity: number | null;
  unit_price: number | null;
  total_value: number;
  fee: number | null;
  custody: string | null;
  payment_type: string | null;
  notes: string | null;
  loss_credit_used?: number | null;
  fund_term?: string | null;
  fund_structure?: string | null;
  fund_estoque_option?: string | null;
};

export type PendingItem = {
  id: number;
  kind: string;
  source: string;
  created_at: string;
  proposed: ProposedTransaction;
  asset_class: string | null;
  needs_class: boolean;
};

type CommitResult = {
  created: number;
  skipped_duplicates: number;
  errors: { line: number; message: string }[];
};

export const PENDING_QUERY_KEY = "import_pending";

const LEDGER_QUERY_KEYS = ["transactions", "positions", "portfolio-summary", "workspace-summary", "assets"];

function numberOrNull(text: string): number | null {
  const trimmed = text.trim();
  return trimmed === "" ? null : Number(trimmed);
}

function ReviewForm({
  portfolioId,
  item,
  total,
  onSkip,
}: {
  portfolioId: number;
  item: PendingItem;
  total: number;
  onSkip: () => void;
}) {
  const queryClient = useQueryClient();
  const p = item.proposed;
  const [date, setDate] = useState(p.transaction_date);
  const [type, setType] = useState(p.transaction_type);
  const [ticker, setTicker] = useState(p.ticker);
  const [assetClass, setAssetClass] = useState(item.asset_class ?? "");
  const [quantity, setQuantity] = useState(p.quantity?.toString() ?? "");
  const [unitPrice, setUnitPrice] = useState(p.unit_price?.toString() ?? "");
  const [totalValue, setTotalValue] = useState(p.total_value.toString());
  const [fee, setFee] = useState(p.fee?.toString() ?? "");
  const [custody, setCustody] = useState(p.custody ?? "");
  const [error, setError] = useState<string | null>(null);

  const classMissing = item.needs_class && assetClass === "";

  const hintQuery = useQuery<string, AppError>({
    queryKey: ["class_tax_hint", assetClass, ticker],
    queryFn: () => invoke("class_tax_hint", { assetClass, ticker }),
    enabled: assetClass !== "",
  });

  function refresh() {
    queryClient.invalidateQueries({ queryKey: [PENDING_QUERY_KEY, portfolioId] });
    queryClient.invalidateQueries({ queryKey: ["pending-counts"] });
    for (const key of LEDGER_QUERY_KEYS) queryClient.invalidateQueries({ queryKey: [key] });
  }

  const confirmMutation = useMutation<CommitResult, AppError, void>({
    mutationFn: () => {
      const proposed: ProposedTransaction = {
        ...p,
        transaction_date: date,
        transaction_type: type,
        ticker: ticker.trim().toUpperCase(),
        quantity: numberOrNull(quantity),
        unit_price: numberOrNull(unitPrice),
        total_value: Number(totalValue),
        fee: numberOrNull(fee),
        custody: custody.trim() === "" ? null : custody.trim(),
      };
      return invoke("confirm_import_pending", {
        portfolioId,
        id: item.id,
        proposed,
        assetClass: assetClass === "" ? null : assetClass,
      });
    },
    onSuccess: (result) => {
      if (result.errors.length > 0) {
        setError(result.errors.map((e) => e.message).join("; "));
        return;
      }
      setError(null);
      refresh();
    },
    onError: (err) => setError(err.message),
  });

  const discardMutation = useMutation<void, AppError, void>({
    mutationFn: () => invoke("discard_import_pending", { portfolioId, id: item.id }),
    onSuccess: () => {
      setError(null);
      refresh();
    },
    onError: (err) => setError(err.message),
  });

  const busy = confirmMutation.isPending || discardMutation.isPending;

  return (
    <div className="flex flex-col gap-4" data-testid="import-review-form">
      <p className="text-sm text-muted-foreground" data-testid="import-review-counter">
        {total} left to review · line {p.line} · {item.source}
      </p>

      <div className="grid grid-cols-2 gap-3">
        <Field label="Date">
          <Input
            type="text"
            value={date}
            onChange={(e) => setDate(e.target.value)}
            placeholder="YYYY-MM-DD"
            data-testid="review-date"
          />
        </Field>
        <Field label="Type">
          <Select value={type} onValueChange={setType}>
            <SelectTrigger className="w-full" data-testid="review-type">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {TRANSACTION_TYPES.map((t) => (
                <SelectItem key={t} value={t}>
                  {TRANSACTION_TYPE_LABELS[t as TransactionType]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <Field label="Ticker">
          <Input value={ticker} onChange={(e) => setTicker(e.target.value)} data-testid="review-ticker" />
        </Field>
        <Field label="Custody">
          <Input value={custody} onChange={(e) => setCustody(e.target.value)} data-testid="review-custody" />
        </Field>
        <Field label="Quantity">
          <Input
            inputMode="decimal"
            value={quantity}
            onChange={(e) => setQuantity(e.target.value)}
            data-testid="review-quantity"
          />
        </Field>
        <Field label="Unit price">
          <Input
            inputMode="decimal"
            value={unitPrice}
            onChange={(e) => setUnitPrice(e.target.value)}
            data-testid="review-unit-price"
          />
        </Field>
        <Field label="Total">
          <Input
            inputMode="decimal"
            value={totalValue}
            onChange={(e) => setTotalValue(e.target.value)}
            data-testid="review-total"
          />
        </Field>
        <Field label="Fee">
          <Input inputMode="decimal" value={fee} onChange={(e) => setFee(e.target.value)} data-testid="review-fee" />
        </Field>
      </div>

      {item.needs_class && (
        <Field label="New asset: choose its class">
          <Select value={assetClass} onValueChange={setAssetClass}>
            <SelectTrigger className="w-full" data-testid="review-class">
              <SelectValue placeholder="Class" />
            </SelectTrigger>
            <SelectContent>
              {ASSET_CLASSES.map((c) => (
                <SelectItem key={c} value={c}>
                  {ASSET_CLASS_LABELS[c as AssetClass]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <p className="text-xs text-muted-foreground" data-testid="review-tax-hint">
            {assetClass === ""
              ? "The class decides how this asset is taxed. A ticker ending in 11 can be a unit, an FII or an ETF."
              : (hintQuery.data ?? "…")}
          </p>
        </Field>
      )}

      {error && (
        <p className="text-sm text-destructive" data-testid="review-error">
          {error}
        </p>
      )}

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          data-testid="review-correct"
          disabled={busy || classMissing || totalValue.trim() === "" || Number.isNaN(Number(totalValue))}
          onClick={() => confirmMutation.mutate()}
        >
          Correct
        </Button>
        <Button type="button" variant="outline" data-testid="review-discard" disabled={busy} onClick={() => discardMutation.mutate()}>
          Discard
        </Button>
        <Button type="button" variant="ghost" data-testid="review-skip" disabled={busy} onClick={onSkip}>
          Skip
        </Button>
      </div>
    </div>
  );
}

function ImportReviewDialog({
  portfolioId,
  open,
  onClose,
}: {
  portfolioId: number;
  open: boolean;
  onClose: () => void;
}) {
  const [skipped, setSkipped] = useState<number[]>([]);

  const pendingQuery = useQuery<PendingItem[], AppError>({
    queryKey: [PENDING_QUERY_KEY, portfolioId],
    queryFn: () => invoke("list_import_pending", { portfolioId }),
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
      <DialogContent className="max-w-xl" data-testid="import-review-dialog">
        <DialogHeader>
          <DialogTitle>Review imported transactions</DialogTitle>
        </DialogHeader>
        {pendingQuery.isLoading && <p className="text-sm">Loading…</p>}
        {pendingQuery.isError && <p className="text-sm text-destructive">{pendingQuery.error.message}</p>}
        {!pendingQuery.isLoading && !current && (
          <div className="flex flex-col gap-3" data-testid="import-review-empty">
            <p className="text-sm text-muted-foreground">Nothing left to review.</p>
            <Button type="button" onClick={() => handleOpenChange(false)}>
              Close
            </Button>
          </div>
        )}
        {current && (
          <ReviewForm
            key={current.id}
            portfolioId={portfolioId}
            item={current}
            total={ordered.length}
            onSkip={skipCurrent}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

export default ImportReviewDialog;
