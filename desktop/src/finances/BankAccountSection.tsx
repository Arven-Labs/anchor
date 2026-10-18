import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { BankAccountView } from "./types";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import Field from "../components/Field";
import { CURRENCIES } from "../stock-lookup/FxLookupSection";
import ConvertedHint from "./ConvertedHint";
import EstimateYieldDialog from "./EstimateYieldDialog";
import ReconcileBalanceDialog from "./ReconcileBalanceDialog";
import { useBaseCurrency } from "./useBaseCurrency";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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

type CreateBankAccountRequest = {
  workspace_id: number;
  nome: string;
  titular: string;
  currency: string;
};

type UpdateBankAccountRequest = {
  bank_account_id: number;
  nome: string;
  titular: string;
  currency: string;
  mirrored_in_portfolio?: boolean;
};

function BankAccountSection({ workspaceId }: { workspaceId: number }) {
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [nome, setNome] = useState("");
  const [titular, setTitular] = useState("");
  const [currency, setCurrency] = useState("BRL");
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<number | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editNome, setEditNome] = useState("");
  const [editTitular, setEditTitular] = useState("");
  const [editCurrency, setEditCurrency] = useState("BRL");
  const [reconcilingAccount, setReconcilingAccount] = useState<BankAccountView | null>(null);
  const [estimatingAccount, setEstimatingAccount] = useState<BankAccountView | null>(null);

  const queryClient = useQueryClient();
  const baseCurrency = useBaseCurrency(workspaceId);

  const accountsQuery = useQuery<BankAccountView[], AppError>({
    queryKey: ["bank-accounts", workspaceId],
    queryFn: () => invoke("list_bank_accounts", { workspaceId }),
  });

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: ["bank-accounts", workspaceId] });
  }

  const createMutation = useMutation<BankAccountView, AppError, CreateBankAccountRequest>({
    mutationFn: (request) => invoke("create_bank_account", { request }),
    onSuccess: () => {
      invalidate();
      setNome("");
      setTitular("");
      setCurrency("BRL");
      setIsAddDialogOpen(false);
    },
  });

  const updateMutation = useMutation<BankAccountView, AppError, UpdateBankAccountRequest>({
    mutationFn: (request) => invoke("update_bank_account", { request }),
    onSuccess: () => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["net-worth", workspaceId] });
      setEditingId(null);
    },
  });

  // Marca "já na carteira": o saldo deixa de contar no caixa do patrimônio na parte que a renda fixa importada cobre.
  function toggleMirrored(account: BankAccountView) {
    updateMutation.mutate({
      bank_account_id: account.id,
      nome: account.nome,
      titular: account.titular,
      currency: account.currency,
      mirrored_in_portfolio: !account.mirrored_in_portfolio,
    });
  }

  const deleteMutation = useMutation<void, AppError, number>({
    mutationFn: (bankAccountId) => invoke("delete_bank_account", { bankAccountId }),
    onSuccess: () => {
      invalidate();
      setConfirmingDeleteId(null);
    },
  });

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    createMutation.mutate({ workspace_id: workspaceId, nome, titular, currency });
  }

  function handleDeleteClick(id: number) {
    if (confirmingDeleteId === id) {
      deleteMutation.mutate(id);
    } else {
      setConfirmingDeleteId(id);
    }
  }

  function startEditing(account: BankAccountView) {
    setEditingId(account.id);
    setEditNome(account.nome);
    setEditTitular(account.titular);
    setEditCurrency(account.currency);
  }

  function handleSaveEdit() {
    if (editingId === null) return;
    updateMutation.mutate({
      bank_account_id: editingId,
      nome: editNome,
      titular: editTitular,
      currency: editCurrency,
    });
  }

  const accounts = accountsQuery.data ?? [];

  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle>Bank accounts</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="mb-4 text-sm text-muted-foreground">
            Accounts used to organize general cash flow (income/expenses). Balance is always
            derived from the transactions below — never entered directly.
          </p>

          <div className="mb-4 flex justify-end">
            <Button type="button" data-testid="account-new" onClick={() => setIsAddDialogOpen(true)}>
              New account
            </Button>
          </div>

          {accountsQuery.isError && (
            <p className="mb-3 text-red-600">{accountsQuery.error.message}</p>
          )}

          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Holder</TableHead>
                <TableHead>Currency</TableHead>
                <TableHead>Balance</TableHead>
                <TableHead>Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {accounts.length === 0 && (
                <TableRow>
                  <TableCell colSpan={5} className="text-center text-muted-foreground">
                    No bank account registered yet.
                  </TableCell>
                </TableRow>
              )}
              {accounts.map((account) => {
                const isConfirming = confirmingDeleteId === account.id;
                const isEditing = editingId === account.id;
                return (
                  <TableRow key={account.id}>
                    {isEditing ? (
                      <>
                        <TableCell>
                          <Input
                            value={editNome}
                            onChange={(e) => setEditNome(e.currentTarget.value)}
                          />
                        </TableCell>
                        <TableCell>
                          <Input
                            value={editTitular}
                            onChange={(e) => setEditTitular(e.currentTarget.value)}
                          />
                        </TableCell>
                        <TableCell>
                          {account.in_use ? (
                            <span title="The currency can't change once the account has transactions, debts or loans.">
                              {account.currency}
                            </span>
                          ) : (
                            <Select value={editCurrency} onValueChange={setEditCurrency}>
                              <SelectTrigger data-testid="account-edit-currency" className="w-24">
                                <SelectValue>{editCurrency}</SelectValue>
                              </SelectTrigger>
                              <SelectContent>
                                {CURRENCIES.map(([code, name]) => (
                                  <SelectItem key={code} value={code}>
                                    {code} — {name}
                                  </SelectItem>
                                ))}
                              </SelectContent>
                            </Select>
                          )}
                        </TableCell>
                        <TableCell>{account.balance.toFixed(2)}</TableCell>
                        <TableCell className="flex gap-2">
                          <Button size="sm" onClick={handleSaveEdit} disabled={updateMutation.isPending}>
                            Save
                          </Button>
                          <Button size="sm" variant="outline" onClick={() => setEditingId(null)}>
                            Cancel
                          </Button>
                        </TableCell>
                      </>
                    ) : (
                      <>
                        <TableCell>{account.nome}</TableCell>
                        <TableCell>{account.titular}</TableCell>
                        <TableCell data-testid="account-currency">{account.currency}</TableCell>
                        <TableCell>
                          {account.balance.toFixed(2)}
                          <ConvertedHint
                            currency={account.currency}
                            baseCurrency={baseCurrency}
                            inBase={account.balance_in_base}
                          />
                          {account.mirrored_in_portfolio && (
                            <Badge
                              variant="secondary"
                              className="ml-2"
                              data-testid="account-mirrored-badge"
                              title="The fixed-income positions in the Portfolio are subtracted from this balance in net worth, so they are not counted twice."
                            >
                              In Portfolio
                            </Badge>
                          )}
                          {(account.auto_yield_pct_of_cdi ?? 0) > 0 && (
                            <Badge
                              variant="outline"
                              className="ml-2"
                              data-testid="account-auto-yield-badge"
                              title="After you finish reviewing a statement, the yield of this account's redemptions is estimated for you."
                            >
                              Auto yield {Math.round((account.auto_yield_pct_of_cdi ?? 0) * 100)}% CDI
                            </Badge>
                          )}
                        </TableCell>
                        <TableCell className="flex flex-wrap gap-2">
                          <Button size="sm" variant="outline" onClick={() => startEditing(account)}>
                            Edit
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            data-testid="account-reconcile"
                            title="Enter the balance your bank shows and book the difference as yield"
                            onClick={() => setReconcilingAccount(account)}
                          >
                            Reconcile balance
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            data-testid="account-estimate"
                            title="Estimate the yield of each redemption from the account's flows and the Selic rate"
                            onClick={() => setEstimatingAccount(account)}
                          >
                            Estimate yield
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            data-testid="account-mirrored-toggle"
                            disabled={updateMutation.isPending}
                            title="Turn on when the fixed income held in this account is also imported as Portfolio positions"
                            onClick={() => toggleMirrored(account)}
                          >
                            {account.mirrored_in_portfolio ? "Count as cash" : "Already in Portfolio"}
                          </Button>
                          <Button
                            variant={isConfirming ? "destructive" : "outline"}
                            size="sm"
                            onClick={() => handleDeleteClick(account.id)}
                          >
                            {isConfirming ? "Confirm?" : "Delete"}
                          </Button>
                        </TableCell>
                      </>
                    )}
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      {estimatingAccount && (
        <EstimateYieldDialog
          account={estimatingAccount}
          workspaceId={workspaceId}
          onClose={() => setEstimatingAccount(null)}
        />
      )}

      {reconcilingAccount && (
        <ReconcileBalanceDialog
          account={reconcilingAccount}
          workspaceId={workspaceId}
          onClose={() => setReconcilingAccount(null)}
        />
      )}

      <Dialog open={isAddDialogOpen} onOpenChange={setIsAddDialogOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>New account</DialogTitle>
          </DialogHeader>

          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <Field label="Name">
              <Input
                required
                data-testid="account-name"
                placeholder="e.g.: Nubank PJ"
                value={nome}
                onChange={(e) => setNome(e.currentTarget.value)}
              />
            </Field>
            <Field label="Holder">
              <Input
                required
                data-testid="account-holder"
                placeholder="e.g.: CPF/CNPJ"
                value={titular}
                onChange={(e) => setTitular(e.currentTarget.value)}
              />
            </Field>
            <Field label="Currency">
              <Select value={currency} onValueChange={setCurrency}>
                <SelectTrigger data-testid="account-currency-select" className="w-full">
                  <SelectValue>{currency}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {CURRENCIES.map(([code, name]) => (
                    <SelectItem key={code} value={code}>
                      {code} — {name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <p className="text-xs text-muted-foreground">
              The currency can't change once the account has transactions, debts or loans.
            </p>

            {createMutation.isError && (
              <p className="text-red-600">{createMutation.error.message}</p>
            )}

            <Button
              type="submit"
              data-testid="account-create"
              disabled={createMutation.isPending}
              className="w-fit"
            >
              {createMutation.isPending ? "Adding..." : "Add account"}
            </Button>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}

export default BankAccountSection;
