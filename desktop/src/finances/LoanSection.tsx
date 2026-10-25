import { Fragment, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { BankAccountView, Borrower, LoanView, NetWorthSummary } from "./types";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import Field from "../components/Field";
import { CURRENCIES } from "../stock-lookup/FxLookupSection";
import { formatMoney } from "@/lib/utils";
import ConvertedHint from "./ConvertedHint";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

// Sentinela do `Select` pra "sem conta": Radix não aceita `value=""`.
const NO_ACCOUNT = "none";

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

// Mesmo padrão de `KpiCard` de `LiabilitySection.tsx` (duplicado de
// propósito, convenção do projeto). Os totais na moeda base saem por
// `formatMoney`, com o código da moeda na frente (Sessão 229).
function KpiCard({ title, value, subtext }: { title: string; value: string; subtext?: string }) {
  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <p className="text-sm text-muted-foreground">{title}</p>
      <p className="mt-1 text-2xl font-semibold">{value}</p>
      {subtext && <p className="mt-1 text-xs text-muted-foreground">{subtext}</p>}
    </div>
  );
}

function optionalAccountId(value: string): number | null {
  return value === NO_ACCOUNT ? null : Number(value);
}

// Fase 29 — empréstimos concedidos a terceiros (lado oposto de
// `LiabilitySection`). Saldo a receber, juros e "extra recebido" vêm
// derivados do backend. Conta bancária é opcional: com conta, o empréstimo
// e cada baixa geram lançamento em Transactions; sem conta, só registro.
function LoanSection({ workspaceId }: { workspaceId: number }) {
  const queryClient = useQueryClient();

  const [isLoanDialogOpen, setIsLoanDialogOpen] = useState(false);
  const [borrowerId, setBorrowerId] = useState("");
  const [loanAccountId, setLoanAccountId] = useState(NO_ACCOUNT);
  // Só vale sem conta; com conta o backend usa a moeda dela.
  const [loanCurrency, setLoanCurrency] = useState("BRL");
  const [principal, setPrincipal] = useState("");
  const [taxa, setTaxa] = useState("");
  const [dataInicio, setDataInicio] = useState(todayIso());
  const [includeInNetWorth, setIncludeInNetWorth] = useState(true);
  const [loanNotes, setLoanNotes] = useState("");

  const [newBorrowerName, setNewBorrowerName] = useState("");
  const [confirmingDeleteBorrowerId, setConfirmingDeleteBorrowerId] = useState<number | null>(null);
  const [confirmingDeleteLoanId, setConfirmingDeleteLoanId] = useState<number | null>(null);
  const [expandedLoanId, setExpandedLoanId] = useState<number | null>(null);

  const [repayingLoan, setRepayingLoan] = useState<LoanView | null>(null);
  const [repayDate, setRepayDate] = useState(todayIso());
  const [repayReceived, setRepayReceived] = useState("");
  const [repayWrittenOff, setRepayWrittenOff] = useState("");
  const [repayAccountId, setRepayAccountId] = useState(NO_ACCOUNT);
  const [repayNotes, setRepayNotes] = useState("");

  const loansQuery = useQuery<LoanView[], AppError>({
    queryKey: ["loans", workspaceId],
    queryFn: () => invoke("list_loans", { workspaceId }),
  });
  const borrowersQuery = useQuery<Borrower[], AppError>({
    queryKey: ["borrowers", workspaceId],
    queryFn: () => invoke("list_borrowers", { workspaceId }),
  });
  const accountsQuery = useQuery<BankAccountView[], AppError>({
    queryKey: ["bank-accounts", workspaceId],
    queryFn: () => invoke("list_bank_accounts", { workspaceId }),
  });
  const netWorthQuery = useQuery<NetWorthSummary, AppError>({
    queryKey: ["net-worth", workspaceId],
    queryFn: () => invoke("get_net_worth_summary", { workspaceId }),
  });

  const loans = loansQuery.data ?? [];
  const borrowers = borrowersQuery.data ?? [];
  const accounts = accountsQuery.data ?? [];

  function invalidateAll() {
    queryClient.invalidateQueries({ queryKey: ["loans", workspaceId] });
    queryClient.invalidateQueries({ queryKey: ["bank-accounts", workspaceId] });
    queryClient.invalidateQueries({ queryKey: ["general-transactions", workspaceId] });
    queryClient.invalidateQueries({ queryKey: ["net-worth", workspaceId] });
  }

  const createBorrowerMutation = useMutation<Borrower, AppError, string>({
    mutationFn: (nome) =>
      invoke("create_borrower", { request: { workspace_id: workspaceId, nome, notes: null } }),
    onSuccess: (created) => {
      queryClient.invalidateQueries({ queryKey: ["borrowers", workspaceId] });
      setNewBorrowerName("");
      setBorrowerId(String(created.id));
    },
  });

  const deleteBorrowerMutation = useMutation<void, AppError, number>({
    mutationFn: (id) => invoke("delete_borrower", { borrowerId: id }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["borrowers", workspaceId] });
      setConfirmingDeleteBorrowerId(null);
    },
    onError: () => setConfirmingDeleteBorrowerId(null),
  });

  const createLoanMutation = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("create_loan", {
        request: {
          workspace_id: workspaceId,
          borrower_id: Number(borrowerId),
          bank_account_id: optionalAccountId(loanAccountId),
          currency: loanCurrency,
          principal: Number(principal),
          taxa_mensal_percentual: taxa !== "" ? Number(taxa) : null,
          data_inicio: dataInicio,
          include_in_net_worth: includeInNetWorth,
          notes: loanNotes,
        },
      }),
    onSuccess: () => {
      invalidateAll();
      setIsLoanDialogOpen(false);
      setPrincipal("");
      setTaxa("");
      setLoanNotes("");
      setLoanAccountId(NO_ACCOUNT);
      setLoanCurrency("BRL");
      setDataInicio(todayIso());
    },
  });

  const toggleNetWorthMutation = useMutation<unknown, AppError, LoanView>({
    mutationFn: (loan) =>
      invoke("update_loan", {
        request: {
          loan_id: loan.id,
          taxa_mensal_percentual: loan.taxa_mensal_percentual,
          include_in_net_worth: !loan.include_in_net_worth,
          notes: loan.notes,
        },
      }),
    onSuccess: invalidateAll,
  });

  const deleteLoanMutation = useMutation<void, AppError, number>({
    mutationFn: (id) => invoke("delete_loan", { loanId: id }),
    onSuccess: () => {
      invalidateAll();
      setConfirmingDeleteLoanId(null);
    },
  });

  const repayMutation = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("register_loan_repayment", {
        request: {
          loan_id: repayingLoan?.id,
          data: repayDate,
          valor_recebido: Number(repayReceived),
          valor_baixado: Number(repayWrittenOff),
          bank_account_id: optionalAccountId(repayAccountId),
          notes: repayNotes,
        },
      }),
    onSuccess: () => {
      invalidateAll();
      setRepayingLoan(null);
    },
  });

  const deleteRepaymentMutation = useMutation<void, AppError, number>({
    mutationFn: (id) => invoke("delete_loan_repayment", { repaymentId: id }),
    onSuccess: invalidateAll,
  });

  function openRepay(loan: LoanView) {
    setRepayingLoan(loan);
    setRepayDate(todayIso());
    setRepayReceived("");
    setRepayWrittenOff("");
    setRepayAccountId(loan.bank_account_id != null ? String(loan.bank_account_id) : NO_ACCOUNT);
    setRepayNotes("");
    repayMutation.reset();
  }

  // O valor baixado acompanha o recebido até o usuário mexer nele: o caso
  // comum é baixar exatamente o que entrou, e o campo separado existe só pro
  // caso "a pessoa devolveu um pouquinho a mais".
  function handleReceivedChange(value: string) {
    if (repayWrittenOff === "" || repayWrittenOff === repayReceived) {
      setRepayWrittenOff(value);
    }
    setRepayReceived(value);
  }

  function handleDeleteLoan(id: number) {
    if (confirmingDeleteLoanId === id) {
      deleteLoanMutation.mutate(id);
    } else {
      setConfirmingDeleteLoanId(id);
    }
  }

  function handleDeleteBorrower(id: number) {
    if (confirmingDeleteBorrowerId === id) {
      deleteBorrowerMutation.mutate(id);
    } else {
      setConfirmingDeleteBorrowerId(id);
    }
  }

  function handleCreateBorrower(event: FormEvent) {
    event.preventDefault();
    if (newBorrowerName.trim() !== "") createBorrowerMutation.mutate(newBorrowerName);
  }

  function handleCreateLoan(event: FormEvent) {
    event.preventDefault();
    createLoanMutation.mutate();
  }

  function handleRepay(event: FormEvent) {
    event.preventDefault();
    repayMutation.mutate();
  }

  const netWorth = netWorthQuery.data;
  const baseCurrency = netWorth?.base_currency;
  // Na moeda base; empréstimo sem cotação do par fica fora (o aviso do
  // patrimônio já conta esses itens).
  const totalReceivable = loans.reduce((sum, l) => sum + (l.saldo_a_receber_in_base ?? 0), 0);
  const canCreateLoan = borrowerId !== "" && principal !== "" && Number(principal) > 0;
  const canRepay =
    repayWrittenOff !== "" &&
    Number(repayWrittenOff) > 0 &&
    repayReceived !== "" &&
    Number(repayReceived) >= 0;
  const errorMessage =
    loansQuery.error?.message ??
    borrowersQuery.error?.message ??
    deleteLoanMutation.error?.message ??
    deleteBorrowerMutation.error?.message ??
    toggleNetWorthMutation.error?.message ??
    deleteRepaymentMutation.error?.message;

  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle>Loans granted</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="mb-4 text-sm text-muted-foreground">
            Money you lent to someone. Interest is optional (leave the rate empty for none). When
            a payment comes in, the amount received and the amount written off the balance are
            separate fields — if they differ, the difference shows up as "extra received". Each
            loan can be left out of your net worth.
          </p>

          <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3">
            <KpiCard title="Total receivable" value={formatMoney(totalReceivable, baseCurrency ?? "BRL")} />
            <KpiCard
              title="Counted in net worth"
              value={formatMoney(netWorth?.total_loans_receivable ?? 0, baseCurrency ?? "BRL")}
              subtext="Only loans with the net worth switch on"
            />
            <KpiCard title="Borrowers" value={String(borrowers.length)} />
          </div>

          <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
            <form onSubmit={handleCreateBorrower} className="flex items-end gap-2">
              <Field label="New borrower">
                <Input
                  data-testid="loan-new-borrower-name"
                  placeholder="e.g.: Mom"
                  value={newBorrowerName}
                  onChange={(e) => setNewBorrowerName(e.currentTarget.value)}
                />
              </Field>
              <Button
                type="submit"
                variant="outline"
                data-testid="loan-add-borrower"
                disabled={newBorrowerName.trim() === "" || createBorrowerMutation.isPending}
              >
                Add
              </Button>
            </form>
            <Button
              type="button"
              data-testid="loan-new"
              disabled={borrowers.length === 0}
              onClick={() => {
                createLoanMutation.reset();
                setIsLoanDialogOpen(true);
              }}
            >
              New loan
            </Button>
          </div>

          {createBorrowerMutation.isError && (
            <p className="mb-3 text-red-600">{createBorrowerMutation.error.message}</p>
          )}
          {borrowers.length === 0 && (
            <p className="mb-3 text-sm text-muted-foreground">
              Add a borrower first, then register the loan.
            </p>
          )}
          {borrowers.length > 0 && (
            <div className="mb-4 flex flex-wrap gap-2">
              {borrowers.map((b) => (
                <div
                  key={b.id}
                  className="flex items-center gap-2 rounded-md border border-border px-2 py-1 text-sm"
                >
                  {b.nome}
                  <Button
                    variant={confirmingDeleteBorrowerId === b.id ? "destructive" : "ghost"}
                    size="sm"
                    onClick={() => handleDeleteBorrower(b.id)}
                  >
                    {confirmingDeleteBorrowerId === b.id ? "Confirm?" : "Remove"}
                  </Button>
                </div>
              ))}
            </div>
          )}

          {errorMessage && <p className="mb-3 text-red-600">{errorMessage}</p>}

          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Borrower</TableHead>
                <TableHead>Date</TableHead>
                <TableHead>Principal</TableHead>
                <TableHead>Rate (mo.)</TableHead>
                <TableHead>Interest</TableHead>
                <TableHead>Written off</TableHead>
                <TableHead>Extra received</TableHead>
                <TableHead>Receivable</TableHead>
                <TableHead>In net worth</TableHead>
                <TableHead>Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {loans.length === 0 && (
                <TableRow>
                  <TableCell colSpan={10} className="text-center text-muted-foreground">
                    No loan registered yet.
                  </TableCell>
                </TableRow>
              )}
              {loans.map((loan) => {
                const isExpanded = expandedLoanId === loan.id;
                return (
                  <Fragment key={loan.id}>
                    <TableRow>
                      <TableCell>
                        {loan.borrower_nome}
                        {loan.notes && (
                          <span className="block text-xs text-muted-foreground">{loan.notes}</span>
                        )}
                      </TableCell>
                      <TableCell>{loan.data_inicio}</TableCell>
                      <TableCell>
                        {loan.principal.toFixed(2)}
                        {baseCurrency && loan.currency !== baseCurrency && (
                          <span className="ml-1 text-xs text-muted-foreground">{loan.currency}</span>
                        )}
                      </TableCell>
                      <TableCell>
                        {loan.taxa_mensal_percentual != null
                          ? `${loan.taxa_mensal_percentual}%`
                          : "—"}
                      </TableCell>
                      <TableCell>{loan.accrued_interest.toFixed(2)}</TableCell>
                      <TableCell>{loan.total_written_off.toFixed(2)}</TableCell>
                      <TableCell data-testid={`loan-extra-${loan.id}`}>
                        {loan.extra_received.toFixed(2)}
                      </TableCell>
                      <TableCell className="font-medium" data-testid={`loan-receivable-${loan.id}`}>
                        {loan.saldo_a_receber.toFixed(2)}
                        <ConvertedHint
                          currency={loan.currency}
                          baseCurrency={baseCurrency}
                          inBase={loan.saldo_a_receber_in_base}
                        />
                      </TableCell>
                      <TableCell>
                        <input
                          type="checkbox"
                          aria-label={`Count loan to ${loan.borrower_nome} in net worth`}
                          checked={loan.include_in_net_worth}
                          disabled={toggleNetWorthMutation.isPending}
                          onChange={() => toggleNetWorthMutation.mutate(loan)}
                        />
                      </TableCell>
                      <TableCell className="flex flex-wrap gap-2">
                        <Button
                          size="sm"
                          data-testid={`loan-repay-${loan.id}`}
                          onClick={() => openRepay(loan)}
                        >
                          Repayment
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setExpandedLoanId(isExpanded ? null : loan.id)}
                        >
                          {isExpanded ? "Hide" : `History (${loan.repayments.length})`}
                        </Button>
                        <Button
                          variant={confirmingDeleteLoanId === loan.id ? "destructive" : "outline"}
                          size="sm"
                          onClick={() => handleDeleteLoan(loan.id)}
                        >
                          {confirmingDeleteLoanId === loan.id ? "Confirm?" : "Delete"}
                        </Button>
                      </TableCell>
                    </TableRow>
                    {isExpanded && (
                      <TableRow>
                        <TableCell colSpan={10} className="bg-muted/30">
                          {loan.repayments.length === 0 ? (
                            <p className="text-sm text-muted-foreground">No repayments yet.</p>
                          ) : (
                            <ul className="flex flex-col gap-1 text-sm">
                              {loan.repayments.map((r) => (
                                <li key={r.id} className="flex flex-wrap items-center gap-3">
                                  <span>{r.data}</span>
                                  <span>Received {r.valor_recebido.toFixed(2)}</span>
                                  <span>Written off {r.valor_baixado.toFixed(2)}</span>
                                  {r.notes && (
                                    <span className="text-muted-foreground">{r.notes}</span>
                                  )}
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    disabled={deleteRepaymentMutation.isPending}
                                    onClick={() => deleteRepaymentMutation.mutate(r.id)}
                                  >
                                    Undo
                                  </Button>
                                </li>
                              ))}
                            </ul>
                          )}
                        </TableCell>
                      </TableRow>
                    )}
                  </Fragment>
                );
              })}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Dialog open={isLoanDialogOpen} onOpenChange={setIsLoanDialogOpen}>
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>New loan</DialogTitle>
          </DialogHeader>

          <form onSubmit={handleCreateLoan} className="flex flex-col gap-4">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <Field label="Borrower">
                <Select value={borrowerId} onValueChange={setBorrowerId}>
                  <SelectTrigger className="w-full" data-testid="loan-borrower-select">
                    <SelectValue placeholder="Select a borrower" />
                  </SelectTrigger>
                  <SelectContent>
                    {borrowers.map((b) => (
                      <SelectItem key={b.id} value={String(b.id)}>
                        {b.nome}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              <Field label="Paid from account (optional)">
                <Select value={loanAccountId} onValueChange={setLoanAccountId}>
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={NO_ACCOUNT}>None — record only</SelectItem>
                    {accounts.map((a) => (
                      <SelectItem key={a.id} value={String(a.id)}>
                        {a.nome} ({a.titular}) · {a.currency}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              {loanAccountId === NO_ACCOUNT && (
                <Field label="Currency">
                  <Select value={loanCurrency} onValueChange={setLoanCurrency}>
                    <SelectTrigger className="w-full" data-testid="loan-currency-select">
                      <SelectValue>{loanCurrency}</SelectValue>
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
              )}
            </div>

            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
              <Field label="Principal">
                <Input
                  required
                  data-testid="loan-principal"
                  type="number"
                  step="any"
                  min="0"
                  value={principal}
                  onChange={(e) => setPrincipal(e.currentTarget.value)}
                />
              </Field>
              <Field label="Monthly rate % (optional)">
                <Input
                  type="number"
                  step="any"
                  min="0"
                  placeholder="No interest"
                  value={taxa}
                  onChange={(e) => setTaxa(e.currentTarget.value)}
                />
              </Field>
              <Field label="Date">
                <Input
                  required
                  type="date"
                  value={dataInicio}
                  onChange={(e) => setDataInicio(e.currentTarget.value)}
                />
              </Field>
            </div>

            <Field label="Notes (optional)">
              <Input value={loanNotes} onChange={(e) => setLoanNotes(e.currentTarget.value)} />
            </Field>

            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={includeInNetWorth}
                onChange={(e) => setIncludeInNetWorth(e.currentTarget.checked)}
              />
              Count this loan in my net worth
            </label>

            {createLoanMutation.isError && (
              <p className="text-red-600">{createLoanMutation.error.message}</p>
            )}

            <Button
              type="submit"
              data-testid="loan-create"
              disabled={!canCreateLoan || createLoanMutation.isPending}
              className="w-fit"
            >
              {createLoanMutation.isPending ? "Creating..." : "Create loan"}
            </Button>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog open={repayingLoan !== null} onOpenChange={(open) => !open && setRepayingLoan(null)}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>
              Repayment — {repayingLoan?.borrower_nome} (receivable{" "}
              {repayingLoan?.saldo_a_receber.toFixed(2)})
            </DialogTitle>
          </DialogHeader>

          <form onSubmit={handleRepay} className="flex flex-col gap-4">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
              <Field label="Date">
                <Input
                  required
                  type="date"
                  value={repayDate}
                  onChange={(e) => setRepayDate(e.currentTarget.value)}
                />
              </Field>
              <Field label="Amount received">
                <Input
                  required
                  data-testid="repay-received"
                  type="number"
                  step="any"
                  min="0"
                  value={repayReceived}
                  onChange={(e) => handleReceivedChange(e.currentTarget.value)}
                />
              </Field>
              <Field label="Amount written off">
                <Input
                  required
                  data-testid="repay-written-off"
                  type="number"
                  step="any"
                  min="0"
                  value={repayWrittenOff}
                  onChange={(e) => setRepayWrittenOff(e.currentTarget.value)}
                />
              </Field>
            </div>

            <Field label="Received into account (optional)">
              <Select value={repayAccountId} onValueChange={setRepayAccountId}>
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={NO_ACCOUNT}>None — record only</SelectItem>
                  {accounts
                    .filter((a) => a.currency === repayingLoan?.currency)
                    .map((a) => (
                      <SelectItem key={a.id} value={String(a.id)}>
                        {a.nome} ({a.titular}) · {a.currency}
                      </SelectItem>
                    ))}
                </SelectContent>
              </Select>
            </Field>

            <Field label="Notes (optional)">
              <Input value={repayNotes} onChange={(e) => setRepayNotes(e.currentTarget.value)} />
            </Field>

            <p className="text-sm text-muted-foreground">
              Written off is what reduces the balance; it can differ from what you received (e.g.
              received 1,050 on a 1,000 debt and wrote off 1,000).
            </p>

            {repayMutation.isError && <p className="text-red-600">{repayMutation.error.message}</p>}

            <Button
              type="submit"
              data-testid="repay-submit"
              disabled={!canRepay || repayMutation.isPending}
              className="w-fit"
            >
              {repayMutation.isPending ? "Saving..." : "Register repayment"}
            </Button>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}

export default LoanSection;
