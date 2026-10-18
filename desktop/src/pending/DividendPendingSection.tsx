import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { DividendSuggestionView, PaymentType } from "../portfolio/types";
import { PAYMENT_TYPE_LABELS } from "../portfolio/types";
import DividendConfirmDialog, {
  formatDividendDate,
  formatDividendValue,
  invalidateDividendQueries,
} from "../portfolio/DividendConfirmDialog";
import { BulkDiscardBar, RowCheckbox, bulkErrorMessage, discardMany, useBulkSelection, type BulkResult } from "./bulkDiscard";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

// Fase 35, entrega 6 — sugestões de provento que ainda esperam decisão, de uma carteira.
// Mesma fila da tela de proventos (`suggested_dividends`), só as acionáveis.

const ACTIONABLE_STATUSES = ["pending", "matched", "divergent"];

const STATUS_LABELS: Record<string, string> = {
  pending: "Pending",
  matched: "Matched",
  divergent: "Divergent",
};

function DividendPendingSection({ portfolioId }: { portfolioId: number }) {
  const queryClient = useQueryClient();
  const [confirming, setConfirming] = useState<DividendSuggestionView | null>(null);

  const suggestionsQuery = useQuery<DividendSuggestionView[], AppError>({
    queryKey: ["dividend_suggestions", portfolioId],
    queryFn: () => invoke("list_dividend_suggestions", { portfolioId }),
  });
  const items = (suggestionsQuery.data ?? []).filter((s) => ACTIONABLE_STATUSES.includes(s.status));

  const selection = useBulkSelection(items.map((item) => item.id));
  const [bulkError, setBulkError] = useState<string | null>(null);

  const discardMutation = useMutation<void, AppError, number>({
    mutationFn: (suggestionId) =>
      invoke("discard_dividend_suggestion", { request: { suggestion_id: suggestionId } }),
    onSuccess: () => invalidateDividendQueries(queryClient, portfolioId),
  });

  const bulkMutation = useMutation<BulkResult, AppError, number[]>({
    mutationFn: (ids) =>
      discardMany(ids, (id) => invoke("discard_dividend_suggestion", { request: { suggestion_id: id } })),
    onSuccess: (result, ids) => {
      setBulkError(bulkErrorMessage(result, ids.length));
      selection.keepOnly(result.failed);
      invalidateDividendQueries(queryClient, portfolioId);
    },
  });
  const busy = discardMutation.isPending || bulkMutation.isPending;

  return (
    <>
      <Card data-testid="dividend-pending">
        <CardHeader>
          <CardTitle>Dividend suggestions ({items.length})</CardTitle>
          <CardDescription>
            Dividends expected or reconstructed from your holdings that you have not confirmed or discarded yet.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          {suggestionsQuery.isError && (
            <p className="text-sm text-destructive">{suggestionsQuery.error.message}</p>
          )}
          {discardMutation.isError && <p className="text-sm text-destructive">{discardMutation.error.message}</p>}
          {bulkError && <p className="text-sm text-destructive">{bulkError}</p>}
          {items.length === 0 ? (
            <p className="text-sm text-muted-foreground" data-testid="dividend-pending-empty">
              Nothing pending.
            </p>
          ) : (
            <>
            <BulkDiscardBar
              testIdPrefix="dividend-pending"
              selection={selection}
              total={items.length}
              busy={busy}
              onDiscard={() => bulkMutation.mutate([...selection.selected])}
            />
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-8" />
                  <TableHead>Ticker</TableHead>
                  <TableHead>Payment date</TableHead>
                  <TableHead>Amount/share</TableHead>
                  <TableHead>Quantity</TableHead>
                  <TableHead>Total</TableHead>
                  <TableHead>Type</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {items.map((item) => (
                  <TableRow key={item.id} data-testid="dividend-pending-row">
                    <TableCell>
                      <RowCheckbox testIdPrefix="dividend-pending" id={item.id} selection={selection} disabled={busy} />
                    </TableCell>
                    <TableCell className="font-medium">{item.ticker}</TableCell>
                    <TableCell>{formatDividendDate(item.payment_date)}</TableCell>
                    <TableCell>{formatDividendValue(item.amount)}</TableCell>
                    <TableCell>{formatDividendValue(item.quantity)}</TableCell>
                    <TableCell>{item.total.toFixed(2)}</TableCell>
                    <TableCell>
                      {PAYMENT_TYPE_LABELS[item.payment_type as PaymentType] ?? item.payment_type}
                    </TableCell>
                    <TableCell>{STATUS_LABELS[item.status] ?? item.status}</TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-2">
                        <Button size="sm" data-testid="dividend-confirm" onClick={() => setConfirming(item)}>
                          Confirm
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          data-testid="dividend-discard"
                          disabled={busy}
                          onClick={() => discardMutation.mutate(item.id)}
                        >
                          Discard
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            </>
          )}
        </CardContent>
      </Card>
      <DividendConfirmDialog portfolioId={portfolioId} suggestion={confirming} onClose={() => setConfirming(null)} />
    </>
  );
}

export default DividendPendingSection;
