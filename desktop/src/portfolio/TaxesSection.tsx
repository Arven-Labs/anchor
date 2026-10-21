import { Fragment, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { BankAccountView } from "../finances/types";
import Field from "../components/Field";
import SaleSimulationCard from "./SaleSimulationCard";
import { Badge } from "@/components/ui/badge";
import { BUCKET_LABELS, type Bucket, type LossBalance } from "./taxLosses";
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

// Cripto tem apuração e DARF próprios (código 4600): `crypto_months` fica fora de `months`.
type CryptoSale = {
  tx_id: number;
  portfolio_id: number;
  asset_id: number;
  ticker: string;
  date: string;
  quantity: number;
  sale_value: number;
  proceeds: number;
  cost: number;
  gain: number;
  cost_unknown: boolean;
};

type CryptoMonth = {
  period: string;
  sales_total: number;
  exempt: boolean;
  gain: number;
  rate: number;
  tax: number;
  carried_in: number;
  due: number;
  carried_out: number;
  due_date: string;
  darf_code: number;
  sales: CryptoSale[];
};

// Exterior (Lei 14.754/2023): apuração anual em reais, sem DARF mensal, então não há pagamento a marcar.
// `fx_source`: a PTAX do Banco Central ou, de reserva, o câmbio de mercado.
type FxSource = "ptax" | "market";

type ForeignSale = {
  tx_id: number;
  ticker: string;
  currency: string;
  date: string;
  quantity: number;
  fx_rate: number;
  fx_source: FxSource;
  sale_value: number;
  proceeds: number;
  cost: number;
  gain: number;
  cost_unknown: boolean;
};

type ForeignDividendItem = {
  tx_id: number;
  ticker: string;
  currency: string;
  date: string;
  fx_rate: number;
  fx_source: FxSource;
  gross: number;
  withheld: number;
};

type ForeignYear = {
  year: number;
  capital_gain: number;
  dividends: number;
  withheld: number;
  loss_carried_in: number;
  taxable_base: number;
  rate: number;
  tax_before_credit: number;
  credit_used: number;
  tax_due: number;
  loss_carried_out: number;
  sales: ForeignSale[];
  dividend_items: ForeignDividendItem[];
};

type PaymentKind = "b3" | "crypto";

// O que o diálogo do DARF precisa de um mês, venha da B3 ou da cripto.
type DarfTarget = { kind: PaymentKind; period: string; due: number; due_date: string; darf_code: number };

type TaxWarning =
  | { kind: "cost_unknown"; ticker: string; date: string }
  | { kind: "crypto_sales_non_brl"; period: string }
  | { kind: "foreign_sales"; year: number }
  | { kind: "foreign_fx_missing"; year: number }
  | { kind: "foreign_fx_approximate"; year: number }
  | { kind: "option_exercised"; ticker: string; date: string };

type TaxPayment = {
  id: number;
  workspace_id: number;
  period: string;
  amount: number;
  paid_date: string;
  general_transaction_id: number | null;
  kind: PaymentKind;
  created_at: string;
};

type TaxAssessmentView = {
  assessment: {
    months: MonthAssessment[];
    crypto_months: CryptoMonth[];
    foreign_years: ForeignYear[];
    warnings: TaxWarning[];
    loss_balances: LossBalance[];
  };
  payments: TaxPayment[];
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
    case "crypto_sales_non_brl":
      return `You sold crypto in ${formatPeriod(w.period)} in a foreign currency (a foreign exchange) — that follows another rule and is not calculated by Anchor, so check whether tax is due.`;
    case "foreign_sales":
      return `You sold foreign assets in ${w.year}, before Law 14,754/2023 took effect (2024) — that year follows the old rules and is not calculated by Anchor.`;
    case "foreign_fx_missing":
      return `Some foreign trades or dividends from ${w.year} have no exchange rate (neither PTAX nor market) for their date, so they were left out of the yearly foreign assessment. Collect the exchange-rate history and open this tab again.`;
    case "foreign_fx_approximate":
      return `Some foreign trades or dividends from ${w.year} were converted at the market exchange rate because the Central Bank's PTAX was not available for them (offline, a currency the PTAX does not quote, or a long gap), so that year's values are approximate.`;
    case "option_exercised":
      return `Option ${w.ticker} was exercised on ${formatDate(w.date)}. Its premium goes into the cost of the stock bought or sold by the exercise, so it is not taxed here — update that stock's cost yourself.`;
  }
}

export default function TaxesSection({ workspaceId }: { workspaceId: number }) {
  const queryClient = useQueryClient();
  const [expanded, setExpanded] = useState<string | null>(null);
  const [darfMonth, setDarfMonth] = useState<DarfTarget | null>(null);
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

  // O DARF é em reais: só contas BRL servem (o backend também recusa as outras).
  const brlAccounts = (bankAccountsQuery.data ?? []).filter((a) => a.currency === "BRL");

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
          kind: darfMonth?.kind,
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
  const lossBalances = assessmentQuery.data?.assessment.loss_balances ?? [];
  const totalTaxSaved = lossBalances.reduce((sum, l) => sum + l.tax_saved, 0);
  const cryptoMonths = assessmentQuery.data?.assessment.crypto_months ?? [];
  const foreignYears = assessmentQuery.data?.assessment.foreign_years ?? [];
  const payments = assessmentQuery.data?.payments ?? [];
  // Um pagamento por mês e por tipo: a B3 e a cripto pagam DARFs diferentes.
  const paymentByPeriod = new Map(payments.filter((p) => p.kind === "b3").map((p) => [p.period, p]));
  const cryptoPaymentByPeriod = new Map(payments.filter((p) => p.kind === "crypto").map((p) => [p.period, p]));
  const today = todayIso();

  const unpaid = [
    ...months.filter((m) => m.due > 0 && !paymentByPeriod.has(m.period)),
    ...cryptoMonths.filter((m) => m.due > 0 && !cryptoPaymentByPeriod.has(m.period)),
  ];
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

  function openDarf(month: DarfTarget) {
    setDarfMonth(month);
    setPaidAmount(month.due.toFixed(2));
    setPaidDate(todayIso());
    setCopied(false);
  }

  function darfText(month: DarfTarget): string {
    return [
      `Revenue code: ${month.darf_code}`,
      `Assessment period: ${lastDayOfPeriod(month.period)}`,
      `Due date: ${formatDate(month.due_date)}`,
      `Principal amount: ${month.due.toFixed(2).replace(".", ",")}`,
    ].join("\n");
  }

  async function copyDarf(month: DarfTarget) {
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

      {lossBalances.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Losses to offset</CardTitle>
            <CardDescription>
              Losses from past sales offset future gains in the same group only — regular trades,
              day trade and real estate funds don't mix. They carry forward with no expiry. Savings
              are the balance times the group's current rate, if you later realize a gain of the
              same size.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
              {lossBalances.map((l) => (
                <div key={l.bucket} className="rounded-lg border border-border p-4">
                  <p className="text-sm text-muted-foreground">{BUCKET_LABELS[l.bucket]}</p>
                  <p className="text-2xl font-semibold" data-testid={`loss-balance-${l.bucket}`}>
                    {formatBrl(l.balance)}
                  </p>
                  <p className="text-sm text-muted-foreground">
                    Saves up to {formatBrl(l.tax_saved)} at {(l.rate * 100).toFixed(0)}%
                  </p>
                </div>
              ))}
            </div>
            {lossBalances.length > 1 && (
              <p className="text-sm text-muted-foreground">
                Total potential savings: {formatBrl(totalTaxSaved)}
              </p>
            )}
          </CardContent>
        </Card>
      )}

      <SaleSimulationCard workspaceId={workspaceId} />

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
                        <TableCell className="text-right font-medium" data-testid={`tax-due-${m.period}`}>
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
                            data-testid={`tax-details-${m.period}`}
                            onClick={() => setExpanded(expanded === m.period ? null : m.period)}
                          >
                            {expanded === m.period ? "Hide" : "Details"}
                          </Button>
                          {m.due > 0 && !payment && (
                            <Button size="sm" onClick={() => openDarf({ ...m, kind: "b3" })}>
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

      {cryptoMonths.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Crypto</CardTitle>
            <CardDescription>
              Gains on crypto sold in reais are exempt when the month's sales add up to the monthly limit
              (all coins together); above it the whole month's gain pays 15%. Losses are not carried
              forward. The DARF has its own revenue code (4600), separate from the stock exchange one. This
              is an estimate based on secondary sources — confirm on the Receita's GCAP before paying.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Table data-testid="crypto-tax-table">
              <TableHeader>
                <TableRow>
                  <TableHead>Month</TableHead>
                  <TableHead className="text-right">Sold</TableHead>
                  <TableHead className="text-right">Gain</TableHead>
                  <TableHead className="text-right">DARF</TableHead>
                  <TableHead>Due date</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {[...cryptoMonths].reverse().map((m) => {
                  const payment = cryptoPaymentByPeriod.get(m.period);
                  const isOverdue = !payment && m.due > 0 && m.due_date < today;
                  return (
                    <TableRow key={m.period}>
                      <TableCell>{formatPeriod(m.period)}</TableCell>
                      <TableCell className="text-right">
                        {formatBrl(m.sales_total)}
                        {m.exempt && (
                          <Badge variant="secondary" className="ml-2">
                            exempt
                          </Badge>
                        )}
                      </TableCell>
                      <TableCell className="text-right">{formatBrl(m.gain)}</TableCell>
                      <TableCell className="text-right font-medium" data-testid={`crypto-tax-due-${m.period}`}>
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
                        {m.due > 0 && !payment && (
                          <Button size="sm" data-testid={`crypto-darf-${m.period}`} onClick={() => openDarf({ ...m, kind: "crypto" })}>
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
                  );
                })}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      {foreignYears.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Foreign assets (Law 14,754/2023)</CardTitle>
            <CardDescription>
              Stocks, ETFs and REITs held abroad are taxed once a year, at a flat rate on the year's gains
              plus dividends, in reais. Each trade and dividend is converted at the exchange rate of its own
              day; losses offset gains and carry into later years; tax withheld abroad is a credit up to the
              tax due. It goes in the annual tax return, not a monthly DARF. The rate is the Central Bank's
              PTAX closing quote (selling rate; the buying rate for tax withheld abroad), and on a day with
              no quote the last one before it. Where the PTAX is missing the market rate is used instead and
              the year is flagged as approximate. This is an estimate based on secondary sources — confirm
              in the return.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Table data-testid="foreign-tax-table">
              <TableHeader>
                <TableRow>
                  <TableHead>Year</TableHead>
                  <TableHead className="text-right">Capital gain</TableHead>
                  <TableHead className="text-right">Dividends</TableHead>
                  <TableHead className="text-right">Loss used</TableHead>
                  <TableHead className="text-right">Taxable base</TableHead>
                  <TableHead className="text-right">Credit</TableHead>
                  <TableHead className="text-right">Estimated tax</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {[...foreignYears].reverse().map((y) => {
                  const key = `foreign-${y.year}`;
                  const isOpen = expanded === key;
                  return (
                    <Fragment key={y.year}>
                      <TableRow>
                        <TableCell>{y.year}</TableCell>
                        <TableCell className="text-right">{formatBrl(y.capital_gain)}</TableCell>
                        <TableCell className="text-right">{formatBrl(y.dividends)}</TableCell>
                        <TableCell className="text-right">
                          {y.loss_carried_in > 0 ? formatBrl(y.loss_carried_in - y.loss_carried_out) : "—"}
                          {y.loss_carried_out > 0 && (
                            <span className="block text-xs text-muted-foreground">
                              {formatBrl(y.loss_carried_out)} carries to next year
                            </span>
                          )}
                        </TableCell>
                        <TableCell className="text-right">{formatBrl(y.taxable_base)}</TableCell>
                        <TableCell className="text-right">{y.credit_used > 0 ? formatBrl(y.credit_used) : "—"}</TableCell>
                        <TableCell className="text-right font-medium" data-testid={`foreign-tax-due-${y.year}`}>
                          {formatBrl(y.tax_due)}
                          <span className="block text-xs text-muted-foreground">at {(y.rate * 100).toFixed(1)}%</span>
                        </TableCell>
                        <TableCell className="flex justify-end">
                          <Button
                            variant="outline"
                            size="sm"
                            data-testid={`foreign-details-${y.year}`}
                            onClick={() => setExpanded(isOpen ? null : key)}
                          >
                            {isOpen ? "Hide" : "Details"}
                          </Button>
                        </TableCell>
                      </TableRow>
                      {isOpen && (
                        <TableRow>
                          <TableCell colSpan={8} className="bg-muted/30">
                            <ForeignDetails year={y} />
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

      {!assessmentQuery.isLoading && months.length === 0 && cryptoMonths.length === 0 && foreignYears.length === 0 && (
        <p className="text-sm text-muted-foreground">
          No sales on B3, of crypto or of foreign assets yet — nothing to assess.
        </p>
      )}

      <Dialog open={darfMonth !== null} onOpenChange={(open) => !open && setDarfMonth(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>DARF — {darfMonth && formatPeriod(darfMonth.period)}</DialogTitle>
          </DialogHeader>
          {darfMonth && (
            <div className="flex flex-col gap-4">
              <pre className="rounded-md bg-muted p-3 text-sm" data-testid="darf-text">{darfText(darfMonth)}</pre>
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
                        {brlAccounts.map((a) => (
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
                {brlAccounts.length === 0 && (
                  <p className="text-xs text-muted-foreground">
                    Create a BRL bank account in General Finances first — the tax is paid in reais.
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

// Vendas e proventos do ano, já em reais, com o câmbio de cada dia.
function ForeignDetails({ year }: { year: ForeignYear }) {
  return (
    <div className="flex flex-col gap-4 p-2" data-testid={`foreign-details-panel-${year.year}`}>
      {year.sales.length > 0 && (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Date</TableHead>
              <TableHead>Asset</TableHead>
              <TableHead className="text-right">Qty</TableHead>
              <TableHead className="text-right">Rate</TableHead>
              <TableHead className="text-right">Sale</TableHead>
              <TableHead className="text-right">Cost</TableHead>
              <TableHead className="text-right">Gain</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {year.sales.map((s) => (
              <TableRow key={s.tx_id}>
                <TableCell>{formatDate(s.date)}</TableCell>
                <TableCell>
                  {s.ticker}
                  {s.cost_unknown && (
                    <Badge variant="outline" className="ml-2">
                      cost unknown
                    </Badge>
                  )}
                </TableCell>
                <TableCell className="text-right">{s.quantity}</TableCell>
                <TableCell className="text-right" data-testid={`foreign-sale-fx-${s.tx_id}`}>
                  {s.fx_rate.toFixed(4)}
                  <span className="block text-xs text-muted-foreground">{s.fx_source === "ptax" ? "PTAX" : "market"}</span>
                </TableCell>
                <TableCell className="text-right">{formatBrl(s.sale_value)}</TableCell>
                <TableCell className="text-right">{formatBrl(s.cost)}</TableCell>
                <TableCell className="text-right">{formatBrl(s.gain)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      {year.dividend_items.length > 0 && (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Date</TableHead>
              <TableHead>Asset</TableHead>
              <TableHead className="text-right">Rate</TableHead>
              <TableHead className="text-right">Dividend</TableHead>
              <TableHead className="text-right">Withheld abroad</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {year.dividend_items.map((i) => (
              <TableRow key={i.tx_id}>
                <TableCell>{formatDate(i.date)}</TableCell>
                <TableCell>{i.ticker}</TableCell>
                <TableCell className="text-right">
                  {i.fx_rate.toFixed(4)}
                  <span className="block text-xs text-muted-foreground">{i.fx_source === "ptax" ? "PTAX" : "market"}</span>
                </TableCell>
                <TableCell className="text-right">{formatBrl(i.gross)}</TableCell>
                <TableCell className="text-right">{i.withheld > 0 ? formatBrl(i.withheld) : "—"}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      <p className="text-xs text-muted-foreground">
        Tax before credit {formatBrl(year.tax_before_credit)}; credit for tax withheld abroad{" "}
        {formatBrl(year.credit_used)} (of {formatBrl(year.withheld)} withheld).
      </p>
    </div>
  );
}

function MonthDetails({ month }: { month: MonthAssessment }) {
  const hasOptions = month.sales.some((s) => s.tx_id < 0);
  return (
    <div className="flex flex-col gap-4 p-2">
      {hasOptions && (
        <p className="text-xs text-muted-foreground" data-testid="options-tax-note">
          This month includes closed option positions (they appear with their series ticker). Options
          have no R$ 20k exemption. Day trade options use the usual 1% withholding on the gain, but
          the withholding on swing options is not estimated, so that part can be slightly above the
          real amount.
        </p>
      )}
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
