import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import ImportReviewDialog, { PENDING_QUERY_KEY, type PendingItem } from "./ImportReviewDialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

// Fase 35 — fila de pendentes: o que uma importação propôs e o dono do projeto
// ainda não aprovou nem recusou (fechou o popup de revisão no meio, ou pulou).
// A mecânica é a mesma para qualquer tipo de item (`kind`); por ora só
// lançamentos da carteira.

const KIND_LABELS: Record<string, string> = {
  transaction: "Transaction",
};

function PendingItemsSection({
  portfolioId,
  reviewOpen,
  onReviewOpenChange,
}: {
  portfolioId: number;
  reviewOpen: boolean;
  onReviewOpenChange: (open: boolean) => void;
}) {
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);

  const pendingQuery = useQuery<PendingItem[], AppError>({
    queryKey: [PENDING_QUERY_KEY, portfolioId],
    queryFn: () => invoke("list_import_pending", { portfolioId }),
  });
  const items = pendingQuery.data ?? [];

  const discardMutation = useMutation<void, AppError, number>({
    mutationFn: (id) => invoke("discard_import_pending", { portfolioId, id }),
    onSuccess: () => {
      setError(null);
      queryClient.invalidateQueries({ queryKey: [PENDING_QUERY_KEY, portfolioId] });
    },
    onError: (err) => setError(err.message),
  });

  return (
    <>
      <Card data-testid="pending-items">
        <CardHeader>
          <CardTitle>Pending review ({items.length})</CardTitle>
          <CardDescription>
            Transactions proposed by an import that you have not approved or discarded yet.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          {pendingQuery.isError && <p className="text-sm text-destructive">{pendingQuery.error.message}</p>}
          {error && <p className="text-sm text-destructive">{error}</p>}
          {items.length === 0 ? (
            <p className="text-sm text-muted-foreground" data-testid="pending-empty">
              Nothing pending.
            </p>
          ) : (
            <>
              <div>
                <Button type="button" data-testid="pending-review" onClick={() => onReviewOpenChange(true)}>
                  Review {items.length} item(s)
                </Button>
              </div>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Kind</TableHead>
                    <TableHead>Date</TableHead>
                    <TableHead>Type</TableHead>
                    <TableHead>Ticker</TableHead>
                    <TableHead>Qty</TableHead>
                    <TableHead>Total</TableHead>
                    <TableHead />
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {items.map((item) => (
                    <TableRow key={item.id} data-testid="pending-row">
                      <TableCell>{KIND_LABELS[item.kind] ?? item.kind}</TableCell>
                      <TableCell>{item.proposed.transaction_date}</TableCell>
                      <TableCell>{item.proposed.transaction_type}</TableCell>
                      <TableCell>{item.proposed.ticker || "—"}</TableCell>
                      <TableCell>{item.proposed.quantity ?? "—"}</TableCell>
                      <TableCell>{item.proposed.total_value.toFixed(2)}</TableCell>
                      <TableCell className="text-right">
                        <Button
                          size="sm"
                          variant="outline"
                          disabled={discardMutation.isPending}
                          onClick={() => discardMutation.mutate(item.id)}
                        >
                          Discard
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </>
          )}
        </CardContent>
      </Card>
      <ImportReviewDialog
        portfolioId={portfolioId}
        open={reviewOpen}
        onClose={() => onReviewOpenChange(false)}
      />
    </>
  );
}

export default PendingItemsSection;
