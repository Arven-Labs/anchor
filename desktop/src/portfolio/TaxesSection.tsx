import { Fragment, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { BankAccountView } from "../finances/types";
import Field from "../components/Field";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

// Fase 24 — IR sobre renda variável (B3). A apuração vem pronta do backend
// (`get_tax_assessment`, recalculada a cada abertura a partir das
// transações de TODAS as carteiras do Workspace — imposto é por CPF). Aqui só
// exibe, monta o guia do DARF e registra o pagamento.

type Bucket = "common" | "day_trade" | "fii";

type SaleResult = {
  tx_id: number;
  portfolio_id: number;
  asset_id: number;
  ticker: string;
  date: string;
  bucket: Bucket;
  is_unit: boolean;
  exemption_eligible: boolean;
  quantity: number;
  sale_value: number;
  proceeds: number;
  cost: number;
  gain: number;
  cost_unknown: boolean;
};

type BucketResult = {
  bucket: Bucket;
  result: number;
  loss_used: number;
  loss_carried: number;
  taxable: number;
  rate: number;
  tax: number;
};

type MonthAssessment = {
  period: string;
  stock_sales_total: number;
  stock_exempt: boolean;
  exempt_gain: number;
  buckets: BucketResult[];
  gross_tax: number;
  irrf_withheld: number;
  irrf_used: number;
  carried_in: number;
  due: number;
  carried_out: number;
  due_date: string;
  darf_code: number;
  sales: SaleResult[];
};

type TaxWarning =
  | { kind: "cost_unknown"; ticker: string; date: string }
  | { kind: "crypto_sales_above_exemption"; period: string; sales_total: number; limit: number }
  | { kind: "crypto_sales_non_brl"; period: string }
  | { kind: "foreign_sales"; year: number };

type TaxPayment = {
  id: number;
  workspace_id: number;
  period: string;
  amount: number;
  paid_date: string;
  general_transaction_id: number | null;
  created_at: string;
};

type TaxAssessmentView = {
  assessment: { months: MonthAssessment[]; warnings: TaxWarning[] };
  payments: TaxPayment[];
};

const BUCKET_LABELS: Record<Bucket, string> = {
  common: "Regular trades (stocks, units, ETFs, BDRs)",
  day_trade: "Day trade",
  fii: "Real estate funds (FII)",
};

// Páginas oficiais da Receita conferidas na Sessão 101: o serviço "Meu
// Imposto de Renda" (onde ficam o ReVar e a emissão de DARF) e a página de
// cálculo/pagamento de renda variável.
const MEU_IR_URL = "https://www.gov.br/pt-br/servicos/consultar-meu-imposto-de-renda";
const RECEITA_HOW_TO_PAY_URL =
  "https://www.gov.br/receitafederal/pt-br/assuntos/meu-imposto-de-renda/pagamento/renda-variavel/bolsa-de-valores-1/calculo-e-pagamento-do-imposto";

function formatBrl(value: number): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency: "BRL" });
}

function formatDate(date: string): string {
  const [y, m, d] = date.slice(0, 10).split("-");
  return `${d}/${m}/${y}`;
}

function formatPeriod(period: string): string {
  const [y, m] = period.split("-");
  return `${m}/${y}`;
}

// Período de apuração no DARF = último dia do mês da apuração.
function lastDayOfPeriod(period: string): string {
  const [y, m] = period.split("-").map(Number);
  const last = new Date(y, m, 0).getDate();
  return `${String(last).padStart(2, "0")}/${String(m).padStart(2, "0")}/${y}`;
}

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

function warningText(w: TaxWarning): string {
  switch (w.kind) {
    case "cost_unknown":
      return `${w.ticker} sold on ${formatDate(w.date)} for more shares than your recorded purchases — only the part with a known cost was counted. Register the missing purchases to fix it.`;
    case "crypto_sales_above_exemption":
      return `Crypto sales in ${formatPeriod(w.period)} (${formatBrl(w.sales_total)}) are above the ${formatBrl(w.limit)} monthly exemption — tax is likely due (DARF code 4600). Not calculated by Anchor yet.`;
    case "crypto_sales_non_brl":
      return `You sold crypto in ${formatPeriod(w.period)} in a foreign currency — check whether tax is due. Not calculated by Anchor yet.`;
    case "foreign_sales":
      return `You sold foreign assets in ${w.year}. Gains abroad are taxed yearly in the annual tax return (Law 14,754/2023) — not calculated by Anchor yet.`;
  }
}

export default function TaxesSection({ workspaceId }: { workspaceId: number }) {
  const queryClient = useQueryClient();
  const [expanded, setExpanded] = useState<string | null>(null);
  const [darfMonth, setDarfMonth] = useState<MonthAssessment | null>(null);
  const [bankAccountId, setBankAccountId] = useState("");
  const [paidDate, setPaidDate] = useState(todayIso());
  const [paidAmount, setPaidAmount] = useState("");
  const [copied, setCopied] = useState(false);

  const assessmentQuery = useQuery<TaxAssessmentView, AppError>({
    queryKey: ["tax-assessment", workspaceId],
    queryFn: () => invoke("get_tax_assessment", { workspaceId }),
  });

  const bankAccountsQuery = useQuery<BankAccountView[], AppError>({
    queryKey: ["bank-accounts", workspaceId],
    queryFn: () => invoke("list_bank_accounts", { workspaceId }),
  });

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: ["tax-assessment", workspaceId] });
    queryClient.invalidateQueries({ queryKey: ["profitability"] });
    queryClient.invalidateQueries({ queryKey: ["profitability-comparison"] });
    queryClient.invalidateQueries({ queryKey: ["general-transactions", workspaceId] });
    queryClient.invalidateQueries({ queryKey: ["bank-accounts", workspaceId] });
  }

  const markPaidMutation = useMutation<TaxPayment, AppError, void>({
    mutationFn: () =>
      invoke("mark_tax_paid", {
        request: {
          workspace_id: workspaceId,
          period: darfMonth?.period,
          amount: Number(paidAmount),
          paid_date: paidDate,
          bank_account_id: Number(bankAccountId),
        },
      }),
    onSuccess: () => {
      invalidate();
      setDarfMonth(null);
    },
  });

  const unmarkPaidMutation = useMutation<void, AppError, number>({
    mutationFn: (paymentId) => invoke("unmark_tax_paid", { paymentId }),
    onSuccess: invalidate,
  });

  const months = assessmentQuery.data?.assessment.months ?? [];
  const warnings = assessmentQuery.data?.assessment.warnings ?? [];
  const payments = assessmentQuery.data?.payments ?? [];
  const paymentByPeriod = new Map(payments.map((p) => [p.period, p]));
  const today = todayIso();

  const unpaid = months.filter((m) => m.due > 0 && !paymentByPeriod.has(m.period));
  const dueNow = unpaid.reduce((sum, m) => sum + m.due, 0);
  const nextDue = unpaid
    .map((m) => m.due_date)
    .filter((d) => d >= today)
    .sort()[0];
  const overdue = unpaid.filter((m) => m.due_date < today);
  const currentYear = today.slice(0, 4);
  const paidThisYear = payments
    .filter((p) => p.paid_date.startsWith(currentYear))
    .reduce((sum, p) => sum + p.amount, 0);

  function openDarf(month: MonthAssessment) {
    setDarfMonth(month);
    setPaidAmount(month.due.toFixed(2));
    setPaidDate(todayIso());
    setCopied(false);
  }

  function darfText(month: MonthAssessment): string {
    return [
      `Revenue code: ${month.darf_code}`,
      `Assessment period: ${lastDayOfPeriod(month.period)}`,
      `Due date: ${formatDate(month.due_date)}`,
      `Principal amount: ${month.due.toFixed(2).replace(".", ",")}`,
    ].join("\n");
  }

  async function copyDarf(month: MonthAssessment) {
    await navigator.clipboard.writeText(darfText(month));
    setCopied(true);
  }

  function handleMarkPaid(event: FormEvent) {
    event.preventDefault();
    markPaidMutation.mutate();
  }

  return (
    <div className="flex flex-col gap-6">
      <Card>
        <CardHeader>
          <CardTitle>Capital gains tax (B3)</CardTitle>
          <CardDescription>
            Calculated from the buys and sells of every portfolio in this workspace — income tax is
            per person. Covers stocks, units, ETFs, BDRs, day trade and real estate funds. Rates and
            limits come from Settings → Taxes. This is an estimate: compare with the Receita's
            ReVar before paying.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {assessmentQuery.isLoading && <p className="text-sm">Loading…</p>}
          {assessmentQuery.isError && (
            <p className="text-sm text-red-600">{assessmentQuery.error.message}</p>
          )}

          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            <div className="rounded-lg border border-border p-4">
              <p className="text-sm text-muted-foreground">Tax due now</p>
              <p className="text-2xl font-semibold">{formatBrl(dueNow)}</p>
              {overdue.length > 0 && (
                <p className="text-sm text-red-600">{overdue.length} overdue</p>
              )}
            </div>
            <div className="rounded-lg border border-border p-4">
              <p className="text-sm text-muted-foreground">Next due date</p>
              <p className="text-2xl font-semibold">{nextDue ? formatDate(nextDue) : "—"}</p>
            </div>
            <div className="rounded-lg border border-border p-4">
              <p className="text-sm text-muted-foreground">Paid in {currentYear}</p>
              <p className="text-2xl font-semibold">{formatBrl(paidThisYear)}</p>
            </div>
          </div>

          {warnings.length > 0 && (
            <div className="flex flex-col gap-2">
              {warnings.map((w, i) => (
                <p
                  key={i}
                  className="rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-sm"
                >
                  {warningText(w)}
                </p>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {months.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Monthly assessment</CardTitle>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Month</TableHead>
                  <TableHead className="text-right">Stock sales</TableHead>
                  <TableHead className="text-right">Tax</TableHead>
                  <TableHead className="text-right">Withheld (IRRF)</TableHead>
                  <TableHead className="text-right">DARF</TableHead>
                  <TableHead>Due date</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {[...months].reverse().map((m) => {
                  const payment = paymentByPeriod.get(m.period);
                  const isOverdue = !payment && m.due > 0 && m.due_date < today;
                  return (
                    <Fragment key={m.period}>
                      <TableRow>
                        <TableCell>{formatPeriod(m.period)}</TableCell>
                        <TableCell className="text-right">
                          {formatBrl(m.stock_sales_total)}
                          {m.stock_exempt && m.exempt_gain > 0 && (
                            <Badge variant="secondary" className="ml-2">
                              exempt
                            </Badge>
                          )}
                        </TableCell>
                        <TableCell className="text-right">{formatBrl(m.gross_tax)}</TableCell>
                        <TableCell className="text-right">{formatBrl(m.irrf_used)}</TableCell>
                        <TableCell className="text-right font-medium">
                          {m.due > 0 ? formatBrl(m.due) : "—"}
                          {m.carried_out > 0 && (
                            <span className="block text-xs text-muted-foreground">
                              {formatBrl(m.carried_out)} below minimum, rolls over
                            </span>
                          )}
                        </TableCell>
                        <TableCell>{m.due > 0 ? formatDate(m.due_date) : "—"}</TableCell>
                        <TableCell>
                          {payment ? (
                            <Badge>Paid {formatDate(payment.paid_date)}</Badge>
                          ) : isOverdue ? (
                            <Badge variant="destructive">Overdue</Badge>
                          ) : m.due > 0 ? (
                            <Badge variant="outline">To pay</Badge>
                          ) : (
                            <span className="text-sm text-muted-foreground">Nothing due</span>
                          )}
                        </TableCell>
                        <TableCell className="flex justify-end gap-2">
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setExpanded(expanded === m.period ? null : m.period)}
                          >
                            {expanded === m.period ? "Hide" : "Details"}
                          </Button>
                          {m.due > 0 && !payment && (
                            <Button size="sm" onClick={() => openDarf(m)}>
                              DARF
                            </Button>
                          )}
                          {payment && (
                            <Button
                              variant="outline"
                              size="sm"
                              disabled={unmarkPaidMutation.isPending}
                              onClick={() => unmarkPaidMutation.mutate(payment.id)}
                            >
                              Undo paid
                            </Button>
                          )}
                        </TableCell>
                      </TableRow>
                      {expanded === m.period && (
                        <TableRow>
                          <TableCell colSpan={8} className="bg-muted/30">
                            <MonthDetails month={m} />
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
      )}

      {!assessmentQuery.isLoading && months.length === 0 && (
        <p className="text-sm text-muted-foreground">
          No sales on B3 yet — nothing to assess.
        </p>
      )}

      <Dialog open={darfMonth !== null} onOpenChange={(open) => !open && setDarfMonth(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>DARF — {darfMonth && formatPeriod(darfMonth.period)}</DialogTitle>
          </DialogHeader>
          {darfMonth && (
            <div className="flex flex-col gap-4">
              <pre className="rounded-md bg-muted p-3 text-sm">{darfText(darfMonth)}</pre>
              <p className="text-sm text-muted-foreground">
                There's no official API to issue a DARF. Copy these fields and issue it at the
                Receita (Sicalc or ReVar, inside "Meu Imposto de Renda"), then pay at your bank.
              </p>
              <div className="flex flex-wrap gap-2">
                <Button variant="outline" onClick={() => copyDarf(darfMonth)}>
                  {copied ? "Copied" : "Copy fields"}
                </Button>
                <Button variant="outline" onClick={() => openUrl(MEU_IR_URL)}>
                  Open Meu Imposto de Renda
                </Button>
                <Button variant="ghost" onClick={() => openUrl(RECEITA_HOW_TO_PAY_URL)}>
                  How to pay (Receita)
                </Button>
              </div>

              <form onSubmit={handleMarkPaid} className="flex flex-col gap-3 border-t pt-4">
                <p className="text-sm font-medium">Already paid? Record it</p>
                <p className="text-xs text-muted-foreground">
                  Creates an expense in General Finances (category "Taxes") and deducts it from
                  your portfolio's profitability.
                </p>
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
                  <Field label="Bank account">
                    <Select value={bankAccountId} onValueChange={setBankAccountId}>
                      <SelectTrigger className="w-full">
                        <SelectValue placeholder="Choose…" />
                      </SelectTrigger>
                      <SelectContent>
                        {(bankAccountsQuery.data ?? []).map((a) => (
                          <SelectItem key={a.id} value={String(a.id)}>
                            {a.nome}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                  <Field label="Paid on">
                    <Input
                      type="date"
                      required
                      value={paidDate}
                      onChange={(e) => setPaidDate(e.currentTarget.value)}
                    />
                  </Field>
                  <Field label="Amount paid">
                    <Input
                      type="number"
                      step="0.01"
                      required
                      value={paidAmount}
                      onChange={(e) => setPaidAmount(e.currentTarget.value)}
                    />
                  </Field>
                </div>
                {(bankAccountsQuery.data ?? []).length === 0 && (
                  <p className="text-xs text-muted-foreground">
                    Create a bank account in General Finances first.
                  </p>
                )}
                {markPaidMutation.isError && (
                  <p className="text-sm text-red-600">{markPaidMutation.error.message}</p>
                )}
                <Button
                  type="submit"
                  className="w-fit"
                  disabled={
                    bankAccountId === "" || Number(paidAmount) <= 0 || markPaidMutation.isPending
                  }
                >
                  {markPaidMutation.isPending ? "Saving…" : "Mark as paid"}
                </Button>
              </form>
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function MonthDetails({ month }: { month: MonthAssessment }) {
  return (
    <div className="flex flex-col gap-4 p-2">
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
        {month.buckets.map((b) => (
          <div key={b.bucket} className="rounded-md border border-border bg-background p-3 text-sm">
            <p className="font-medium">{BUCKET_LABELS[b.bucket]}</p>
            <p>Result: {formatBrl(b.result)}</p>
            {b.loss_used > 0 && <p>Past losses used: {formatBrl(b.loss_used)}</p>}
            <p>
              Taxable: {formatBrl(b.taxable)} × {(b.rate * 100).toFixed(1)}% = {formatBrl(b.tax)}
            </p>
            {b.loss_carried > 0 && (
              <p className="text-muted-foreground">Losses to offset later: {formatBrl(b.loss_carried)}</p>
            )}
          </div>
        ))}
      </div>
      {month.stock_exempt && month.exempt_gain > 0 && (
        <p className="text-sm text-muted-foreground">
          Stock sales this month were within the exemption limit — {formatBrl(month.exempt_gain)} of
          stock gains are tax-free (units, ETFs and BDRs never are).
        </p>
      )}
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Date</TableHead>
            <TableHead>Asset</TableHead>
            <TableHead>Type</TableHead>
            <TableHead className="text-right">Qty</TableHead>
            <TableHead className="text-right">Sale</TableHead>
            <TableHead className="text-right">Cost</TableHead>
            <TableHead className="text-right">Gain</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {month.sales.map((s, i) => (
            <TableRow key={`${s.tx_id}-${s.bucket}-${i}`}>
              <TableCell>{formatDate(s.date)}</TableCell>
              <TableCell>
                {s.ticker}
                {s.is_unit && (
                  <Badge variant="outline" className="ml-2">
                    unit
                  </Badge>
                )}
                {s.cost_unknown && (
                  <Badge variant="destructive" className="ml-2">
                    cost unknown
                  </Badge>
                )}
              </TableCell>
              <TableCell>{BUCKET_LABELS[s.bucket]}</TableCell>
              <TableCell className="text-right">{s.quantity}</TableCell>
              <TableCell className="text-right">{formatBrl(s.proceeds)}</TableCell>
              <TableCell className="text-right">{formatBrl(s.cost)}</TableCell>
              <TableCell className={`text-right ${s.gain < 0 ? "text-red-600" : ""}`}>
                {formatBrl(s.gain)}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}
