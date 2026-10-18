import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import StatementReviewDialog, { STATEMENT_PENDING_KEY, type StatementPendingItem } from "./StatementReviewDialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

// Fase 35, entrega 3 — fila de pendentes do extrato bancário: o que uma importação
// propôs e o dono do projeto ainda não aprovou nem recusou. Mesma fila (`import_pending`)
// dos lançamentos de carteira, mostrada aqui por Workspace.

const TYPE_LABELS: Record<string, string> = {
  receita: "Income",
  despesa: "Expense",
  transferencia: "Transfer",
};

function StatementPendingSection({
  workspaceId,
  reviewOpen,
  onReviewOpenChange,
}: {
  workspaceId: number;
  reviewOpen: boolean;
  onReviewOpenChange: (open: boolean) => void;
}) {
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);

  const pendingQuery = useQuery<StatementPendingItem[], AppError>({
    queryKey: [STATEMENT_PENDING_KEY, workspaceId],
    queryFn: () => invoke("list_statement_pending", { workspaceId }),
  });
  const items = pendingQuery.data ?? [];

  const discardMutation = useMutation<void, AppError, number>({
    mutationFn: (id) => invoke("discard_statement_pending", { workspaceId, id }),
    onSuccess: () => {
      setError(null);
      queryClient.invalidateQueries({ queryKey: [STATEMENT_PENDING_KEY, workspaceId] });
    },
    onError: (err) => setError(err.message),
  });

  return (
    <>
      <Card data-testid="statement-pending">
        <CardHeader>
          <CardTitle>Statement pending review ({items.length})</CardTitle>
          <CardDescription>
            Bank statement lines proposed by an import that you have not approved or discarded yet.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          {pendingQuery.isError && <p className="text-sm text-destructive">{pendingQuery.error.message}</p>}
          {error && <p className="text-sm text-destructive">{error}</p>}
          {items.length === 0 ? (
            <p className="text-sm text-muted-foreground" data-testid="statement-pending-empty">
              Nothing pending.
            </p>
          ) : (
            <>
              <div>
                <Button type="button" data-testid="statement-review-open" onClick={() => onReviewOpenChange(true)}>
                  Review {items.length} item(s)
                </Button>
              </div>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Date</TableHead>
                    <TableHead>Account</TableHead>
                    <TableHead>Kind</TableHead>
                    <TableHead>Description</TableHead>
                    <TableHead>Amount</TableHead>
                    <TableHead />
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {items.map((item) => (
                    <TableRow key={item.id} data-testid="statement-pending-row">
                      <TableCell>{item.proposed.transaction_date}</TableCell>
                      <TableCell>{item.bank_account_name}</TableCell>
                      <TableCell>{TYPE_LABELS[item.suggested_type] ?? item.suggested_type}</TableCell>
                      <TableCell>{item.proposed.description || item.proposed.label}</TableCell>
                      <TableCell>{item.proposed.valor.toFixed(2)}</TableCell>
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
      <StatementReviewDialog workspaceId={workspaceId} open={reviewOpen} onClose={() => onReviewOpenChange(false)} />
    </>
  );
}

export default StatementPendingSection;
