import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { BankAccountView, GeneralTransactionCategory } from "./types";
import {
  GENERAL_TRANSACTION_TYPES,
  GENERAL_TRANSACTION_TYPE_LABELS,
  type GeneralTransaction,
  type GeneralTransactionType,
} from "./types";
import { formatMoney } from "@/lib/utils";
import CategoryManagerDialog from "./CategoryManagerDialog";
import ConvertedHint from "./ConvertedHint";
import { useBaseCurrency } from "./useBaseCurrency";
import TransferDialog from "./TransferDialog";
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
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

type CreateGeneralTransactionRequest = {
  workspace_id: number;
  bank_account_id: number;
  transaction_type: string;
  category_id: number | null;
  valor: number;
  transaction_date: string;
  notes: string | null;
};

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

// Fase 19 — o valor do lançamento está na moeda da conta; o código aparece
// junto do nome quando não é BRL (a moeda de antes de existir moeda por conta).
function accountLabel(accounts: BankAccountView[], id: number): string {
  const account = accounts.find((a) => a.id === id);
  if (!account) return "—";
  return account.currency === "BRL" ? account.nome : `${account.nome} (${account.currency})`;
}

function categoryLabel(categories: GeneralTransactionCategory[], id: number | null): string {
  if (id === null) return "—";
  return categories.find((c) => c.id === id)?.nome ?? "—";
}

function GeneralTransactionSection({ workspaceId }: { workspaceId: number }) {
  const [isNewTransactionDialogOpen, setIsNewTransactionDialogOpen] = useState(false);
  const [isCategoryDialogOpen, setIsCategoryDialogOpen] = useState(false);
  const [isTransferDialogOpen, setIsTransferDialogOpen] = useState(false);
  const [transactionType, setTransactionType] = useState<GeneralTransactionType>("receita");
  const [bankAccountId, setBankAccountId] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const [valor, setValor] = useState("");
  const [transactionDate, setTransactionDate] = useState(todayIso());
  const [notes, setNotes] = useState("");
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<number | null>(null);

  const queryClient = useQueryClient();

  const accountsQuery = useQuery<BankAccountView[], AppError>({
    queryKey: ["bank-accounts", workspaceId],
    queryFn: () => invoke("list_bank_accounts", { workspaceId }),
  });
  const categoriesQuery = useQuery<GeneralTransactionCategory[], AppError>({
    queryKey: ["general-transaction-categories", workspaceId],
    queryFn: () => invoke("list_general_transaction_categories", { workspaceId }),
  });
  const transactionsQuery = useQuery<GeneralTransaction[], AppError>({
    queryKey: ["general-transactions", workspaceId],
    queryFn: () => invoke("list_general_transactions", { workspaceId }),
  });

  const baseCurrency = useBaseCurrency(workspaceId);
  const accounts = accountsQuery.data ?? [];
  const categories = categoriesQuery.data ?? [];
  const transactions = transactionsQuery.data ?? [];

  function invalidateAfterWrite() {
    queryClient.invalidateQueries({ queryKey: ["general-transactions", workspaceId] });
    queryClient.invalidateQueries({ queryKey: ["bank-accounts", workspaceId] });
    queryClient.invalidateQueries({ queryKey: ["net-worth", workspaceId] });
  }

  // Fase 19 — as duas pernas de uma transferência viram uma linha só (a lista vem
  // ordenada por data; a primeira perna encontrada representa o par).
  const legsByTransfer = new Map<number, GeneralTransaction[]>();
  for (const tx of transactions) {
    if (tx.transfer_id !== null) {
      legsByTransfer.set(tx.transfer_id, [...(legsByTransfer.get(tx.transfer_id) ?? []), tx]);
    }
  }
  const shownTransfers = new Set<number>();
  const rows = transactions.filter((tx) => {
    if (tx.transfer_id === null) return true;
    if (shownTransfers.has(tx.transfer_id)) return false;
    shownTransfers.add(tx.transfer_id);
    return true;
  });

  function transferSummary(transferId: number): { accounts: string; value: string } | null {
    const legs = legsByTransfer.get(transferId) ?? [];
    const sent = legs.find((l) => l.transaction_type === "transferencia_saida");
    const received = legs.find((l) => l.transaction_type === "transferencia_entrada");
    if (!sent || !received) return null;
    const fromAccount = accounts.find((a) => a.id === sent.bank_account_id);
    const toAccount = accounts.find((a) => a.id === received.bank_account_id);
    const sentText = `${sent.valor.toFixed(2)} ${fromAccount?.currency ?? ""}`.trim();
    const receivedText = `${received.valor.toFixed(2)} ${toAccount?.currency ?? ""}`.trim();
    return {
      accounts: `${fromAccount?.nome ?? "—"} → ${toAccount?.nome ?? "—"}`,
      value:
        fromAccount?.currency === toAccount?.currency ? sentText : `${sentText} → ${receivedText}`,
    };
  }

  const createMutation = useMutation<
    GeneralTransaction,
    AppError,
    CreateGeneralTransactionRequest
  >({
    mutationFn: (request) => invoke("create_general_transaction", { request }),
    onSuccess: () => {
      invalidateAfterWrite();
      setCategoryId("");
      setValor("");
      setNotes("");
      setIsNewTransactionDialogOpen(false);
    },
  });

  const deleteMutation = useMutation<void, AppError, number>({
    mutationFn: (generalTransactionId) =>
      invoke("delete_general_transaction", { generalTransactionId }),
    onSuccess: () => {
      invalidateAfterWrite();
      setConfirmingDeleteId(null);
    },
  });

  function handleDeleteClick(id: number) {
    if (confirmingDeleteId === id) {
      deleteMutation.mutate(id);
    } else {
      setConfirmingDeleteId(id);
    }
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    createMutation.mutate({
      workspace_id: workspaceId,
      bank_account_id: Number(bankAccountId),
      transaction_type: transactionType,
      category_id: categoryId !== "" ? Number(categoryId) : null,
      valor: Number(valor),
      transaction_date: transactionDate,
      notes: notes.trim() === "" ? null : notes,
    });
  }

  const canSubmit = bankAccountId !== "" && valor !== "";

  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle>Transaction history</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="mb-4 flex justify-end gap-2">
            <Button type="button" variant="outline" onClick={() => setIsCategoryDialogOpen(true)}>
              Manage categories
            </Button>
            <Button
              type="button"
              variant="outline"
              data-testid="transfer-new"
              onClick={() => setIsTransferDialogOpen(true)}
            >
              New transfer
            </Button>
            <Button type="button" onClick={() => setIsNewTransactionDialogOpen(true)}>
              New transaction
            </Button>
          </div>

          {transactionsQuery.isError && (
            <p className="mb-3 text-red-600">{transactionsQuery.error.message}</p>
          )}
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Date</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Account</TableHead>
                <TableHead>Category</TableHead>
                <TableHead>Value</TableHead>
                <TableHead>Notes</TableHead>
                <TableHead>Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {transactions.length === 0 && (
                <TableRow>
                  <TableCell colSpan={7} className="text-center text-muted-foreground">
                    No transactions yet.
                  </TableCell>
                </TableRow>
              )}
              {rows.map((tx) => {
                const isConfirming = confirmingDeleteId === tx.id;
                const transfer = tx.transfer_id !== null ? transferSummary(tx.transfer_id) : null;
                if (transfer) {
                  return (
                    <TableRow key={`transfer-${tx.transfer_id}`} data-testid="transfer-row">
                      <TableCell>{tx.transaction_date}</TableCell>
                      <TableCell>Transfer</TableCell>
                      <TableCell>{transfer.accounts}</TableCell>
                      <TableCell>—</TableCell>
                      <TableCell data-testid="transfer-value">{transfer.value}</TableCell>
                      <TableCell>{tx.notes ?? "—"}</TableCell>
                      <TableCell>
                        <Button
                          variant={isConfirming ? "destructive" : "outline"}
                          size="sm"
                          onClick={() => handleDeleteClick(tx.id)}
                        >
                          {isConfirming ? "Confirm? (deletes both legs)" : "Delete"}
                        </Button>
                      </TableCell>
                    </TableRow>
                  );
                }
                return (
                  <TableRow key={tx.id}>
                    <TableCell>{tx.transaction_date}</TableCell>
                    <TableCell>
                      {GENERAL_TRANSACTION_TYPE_LABELS[tx.transaction_type as GeneralTransactionType] ??
                        tx.transaction_type}
                    </TableCell>
                    <TableCell>{accountLabel(accounts, tx.bank_account_id)}</TableCell>
                    <TableCell>{categoryLabel(categories, tx.category_id)}</TableCell>
                    <TableCell>
                      {formatMoney(tx.valor, tx.currency)}
                      <ConvertedHint currency={tx.currency} baseCurrency={baseCurrency} inBase={tx.valor_in_base} />
                    </TableCell>
                    <TableCell>{tx.notes ?? "—"}</TableCell>
                    <TableCell>
                      <Button
                        variant={isConfirming ? "destructive" : "outline"}
                        size="sm"
                        onClick={() => handleDeleteClick(tx.id)}
                      >
                        {isConfirming ? "Confirm?" : "Delete"}
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Dialog open={isNewTransactionDialogOpen} onOpenChange={setIsNewTransactionDialogOpen}>
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>New transaction</DialogTitle>
          </DialogHeader>

          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <Field label="Type">
                <Select
                  value={transactionType}
                  onValueChange={(value) => setTransactionType(value as GeneralTransactionType)}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {GENERAL_TRANSACTION_TYPES.map((key) => (
                      <SelectItem key={key} value={key}>
                        {GENERAL_TRANSACTION_TYPE_LABELS[key]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>

              <Field label="Account">
                <Select value={bankAccountId} onValueChange={setBankAccountId}>
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="Select an account" />
                  </SelectTrigger>
                  <SelectContent>
                    {accounts.map((account) => (
                      <SelectItem key={account.id} value={String(account.id)}>
                        {account.nome} ({account.titular}) · {account.currency}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            </div>

            <Field label="Date">
              <Input
                required
                type="date"
                value={transactionDate}
                onChange={(e) => setTransactionDate(e.currentTarget.value)}
              />
            </Field>

            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <Field label="Category (optional)">
                <div className="flex items-center gap-2">
                  <Select value={categoryId} onValueChange={setCategoryId}>
                    <SelectTrigger className="w-full">
                      <SelectValue placeholder="No category" />
                    </SelectTrigger>
                    <SelectContent>
                      {categories.map((category) => (
                        <SelectItem key={category.id} value={String(category.id)}>
                          {category.nome}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => setIsCategoryDialogOpen(true)}
                  >
                    New
                  </Button>
                </div>
              </Field>
              <Field label="Value">
                <Input
                  required
                  type="number"
                  step="any"
                  value={valor}
                  onChange={(e) => setValor(e.currentTarget.value)}
                />
              </Field>
            </div>

            <Field label="Notes (optional)">
              <Input value={notes} onChange={(e) => setNotes(e.currentTarget.value)} />
            </Field>

            {createMutation.isError && (
              <p className="text-red-600">{createMutation.error.message}</p>
            )}

            <Button type="submit" disabled={!canSubmit || createMutation.isPending} className="w-fit">
              {createMutation.isPending ? "Logging..." : "Log transaction"}
            </Button>
          </form>
        </DialogContent>
      </Dialog>

      <TransferDialog
        workspaceId={workspaceId}
        accounts={accounts}
        open={isTransferDialogOpen}
        onOpenChange={setIsTransferDialogOpen}
        onDone={invalidateAfterWrite}
      />

      <CategoryManagerDialog
        workspaceId={workspaceId}
        open={isCategoryDialogOpen}
        onOpenChange={setIsCategoryDialogOpen}
      />
    </>
  );
}

export default GeneralTransactionSection;
