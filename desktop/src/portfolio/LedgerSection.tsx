import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { AppError } from "../types";
import {
  ASSET_CLASSES,
  ASSET_CLASS_LABELS,
  TRANSACTION_TYPE_LABELS,
  type AssetClass,
  type TransactionType,
  type TransactionView,
} from "./types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import ImportExportPanel from "./ImportExportPanel";
import { PENDING_QUERY_KEY, type PendingItem } from "./ImportReviewDialog";
import PendingNotice from "../pending/PendingNotice";
import { useOpenPending } from "../pending/PendingNav";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
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

// Grupo sintético pra aporte/retirada (sem asset_id/asset_class) na tabela
// agrupada por classe — sempre renderizado por último (ver `groups` abaixo).
const CASH_FLOW_GROUP = "__cash_flow__";

// Par divergente validado (skill dataviz, `validate_palette.js
// "#3987e5,#e25c5c" --mode dark --surface "#111820" --pairs all`): 5/5
// checks OK, pior ΔE CVD 20.6 (protan), normal-vision 29.7. Azul já é o
// mesmo usado pra "Stock (B3)" na donut do Resumo (SummarySection.tsx);
// vermelho é o `--destructive` do tema (index.css) — nenhuma cor nova.
const BUY_COLOR = "#3987e5";
const SELL_COLOR = "#e25c5c";
const GRID_COLOR = "#1f2630"; // --border
const AXIS_COLOR = "#9fb1c2"; // --muted-foreground
const TOOLTIP_BG = "#111820"; // --card
const REFERENCE_COLOR = "#3a4553";

const MONTH_LABELS = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun",
  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

function monthLabel(yearMonth: string): string {
  const [year, month] = yearMonth.split("-");
  return `${MONTH_LABELS[Number(month) - 1]}/${year.slice(2)}`;
}

function formatCurrency(value: number): string {
  return value.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function formatQuantity(value: number | null): string {
  return value === null ? "—" : value.toString();
}

type PeriodKey = "12m" | "24m" | "all";

const PERIODS: { key: PeriodKey; label: string }[] = [
  { key: "12m", label: "12M" },
  { key: "24m", label: "24M" },
  { key: "all", label: "All" },
];

const PERIOD_MONTHS: Record<Exclude<PeriodKey, "all">, number> = { "12m": 12, "24m": 24 };

// Corta a lista já carregada (sem refetch) pra janela escolhida, contada a
// partir do lançamento mais recente do conjunto filtrado — mesmo espírito de
// `stock-lookup/PriceHistoryChart.tsx::filterByPeriod`.
function filterByPeriod(txs: TransactionView[], period: PeriodKey): TransactionView[] {
  if (period === "all" || txs.length === 0) return txs;
  const latest = txs.reduce(
    (max, tx) => (tx.transaction_date > max ? tx.transaction_date : max),
    txs[0].transaction_date,
  );
  const cutoff = new Date(latest);
  cutoff.setMonth(cutoff.getMonth() - PERIOD_MONTHS[period]);
  const cutoffStr = cutoff.toISOString().slice(0, 10);
  return txs.filter((tx) => tx.transaction_date >= cutoffStr);
}

function ContributionsTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { payload: { label: string; buys: number; sells: number } }[];
}) {
  if (!active || !payload?.length) return null;
  const row = payload[0].payload;
  return (
    <div
      className="rounded-lg border border-border px-3 py-2 text-sm"
      style={{ background: TOOLTIP_BG }}
    >
      <p className="font-medium">{row.label}</p>
      <p style={{ color: BUY_COLOR }}>Buys: {formatCurrency(row.buys)}</p>
      <p style={{ color: SELL_COLOR }}>Sells: {formatCurrency(Math.abs(row.sells))}</p>
    </div>
  );
}

function OrderTypeBadge({ type }: { type: string }) {
  const label = TRANSACTION_TYPE_LABELS[type as TransactionType] ?? type;
  if (type === "compra") {
    return (
      <Badge className="bg-blue-100 text-blue-800 dark:bg-blue-950 dark:text-blue-300">
        {label}
      </Badge>
    );
  }
  if (type === "venda") {
    return (
      <Badge className="bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-300">{label}</Badge>
    );
  }
  return <Badge variant="secondary">{label}</Badge>;
}

function LedgerSection({ portfolioId }: { portfolioId: number }) {
  const [period, setPeriod] = useState<PeriodKey>("12m");
  const [assetClassFilter, setAssetClassFilter] = useState<string>("all");
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());
  const openPending = useOpenPending();
  const pendingQuery = useQuery<PendingItem[], AppError>({
    queryKey: [PENDING_QUERY_KEY, portfolioId],
    queryFn: () => invoke("list_import_pending", { portfolioId }),
  });
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<number | null>(null);

  const queryClient = useQueryClient();

  const transactionsQuery = useQuery<TransactionView[], AppError>({
    queryKey: ["transactions", portfolioId],
    queryFn: () => invoke("list_transactions", { portfolioId }),
  });

  const deleteMutation = useMutation<void, AppError, number>({
    mutationFn: (transactionId) => invoke("delete_transaction", { transactionId }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["transactions", portfolioId] });
      queryClient.invalidateQueries({ queryKey: ["positions", portfolioId] });
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

  const transactions = transactionsQuery.data ?? [];

  const chartData = useMemo(() => {
    let tradeTxs = transactions.filter(
      (tx) => tx.transaction_type === "compra" || tx.transaction_type === "venda",
    );
    if (assetClassFilter !== "all") {
      tradeTxs = tradeTxs.filter((tx) => tx.asset_class === assetClassFilter);
    }
    tradeTxs = filterByPeriod(tradeTxs, period);

    const byMonth = new Map<string, { buys: number; sells: number }>();
    for (const tx of tradeTxs) {
      const ym = tx.transaction_date.slice(0, 7);
      const entry = byMonth.get(ym) ?? { buys: 0, sells: 0 };
      // Na moeda base (Fase 19); lançamento sem cotação do par fica fora do gráfico.
      const value = tx.total_value_in_base ?? 0;
      if (tx.transaction_type === "compra") entry.buys += value;
      else entry.sells += value;
      byMonth.set(ym, entry);
    }
    return [...byMonth.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([ym, { buys, sells }]) => ({
        year_month: ym,
        label: monthLabel(ym),
        buys,
        sells: -sells, // espelhado abaixo da linha zero
      }));
  }, [transactions, assetClassFilter, period]);

  // Agrupamento sempre sobre a lista completa (sem os filtros do gráfico) —
  // mesmo padrão de "Meus Ativos" em SummarySection.tsx.
  const groups = useMemo(() => {
    const byGroup = new Map<string, TransactionView[]>();
    for (const tx of transactions) {
      const key = tx.asset_class ?? CASH_FLOW_GROUP;
      const list = byGroup.get(key) ?? [];
      list.push(tx);
      byGroup.set(key, list);
    }
    const entries = [...byGroup.entries()].map(([key, items]) => ({
      key,
      label: key === CASH_FLOW_GROUP ? "Cash flow" : ASSET_CLASS_LABELS[key as AssetClass] ?? key,
      items: [...items].sort(
        (a, b) => b.transaction_date.localeCompare(a.transaction_date) || b.id - a.id,
      ),
    }));
    entries.sort((a, b) => {
      if (a.key === CASH_FLOW_GROUP) return 1;
      if (b.key === CASH_FLOW_GROUP) return -1;
      return b.items.length - a.items.length;
    });
    return entries;
  }, [transactions]);

  function toggleGroup(key: string) {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  if (transactionsQuery.isError) {
    return <p className="text-red-600">{transactionsQuery.error.message}</p>;
  }
  if (transactionsQuery.isLoading) {
    return <p className="text-muted-foreground">Loading...</p>;
  }

  return (
    <div className="flex flex-col gap-6">
      <ImportExportPanel
        portfolioId={portfolioId}
        onStaged={() => openPending({ kind: "portfolio", portfolioId })}
      />
      <PendingNotice
        count={pendingQuery.data?.length ?? 0}
        focus={{ kind: "portfolio", portfolioId }}
        testId="pending-notice-portfolio"
      />
      <Card>
        <CardHeader>
          <CardTitle>Monthly contributions</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="flex flex-wrap items-center gap-4">
            <div className="flex gap-1">
              {PERIODS.map((p) => (
                <Button
                  key={p.key}
                  type="button"
                  size="sm"
                  variant={period === p.key ? "default" : "outline"}
                  onClick={() => setPeriod(p.key)}
                >
                  {p.label}
                </Button>
              ))}
            </div>
            <Select value={assetClassFilter} onValueChange={setAssetClassFilter}>
              <SelectTrigger className="w-[200px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All classes</SelectItem>
                {ASSET_CLASSES.map((assetClass) => (
                  <SelectItem key={assetClass} value={assetClass}>
                    {ASSET_CLASS_LABELS[assetClass]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {chartData.length === 0 ? (
            <p className="text-muted-foreground">No buy/sell transactions in this period yet.</p>
          ) : (
            <ResponsiveContainer width="100%" height={260}>
              <BarChart data={chartData} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
                <CartesianGrid stroke={GRID_COLOR} vertical={false} />
                <XAxis dataKey="label" stroke={AXIS_COLOR} fontSize={12} tickLine={false} />
                <YAxis
                  stroke={AXIS_COLOR}
                  fontSize={12}
                  tickLine={false}
                  axisLine={false}
                  width={72}
                  tickFormatter={(value: number) => formatCurrency(value)}
                />
                <ReferenceLine y={0} stroke={REFERENCE_COLOR} />
                <Tooltip content={<ContributionsTooltip />} cursor={{ fill: GRID_COLOR }} />
                <Legend formatter={(value) => (value === "buys" ? "Buys" : "Sells")} />
                <Bar dataKey="buys" name="buys" fill={BUY_COLOR} radius={[4, 4, 4, 4]} />
                <Bar dataKey="sells" name="sells" fill={SELL_COLOR} radius={[4, 4, 4, 4]} />
              </BarChart>
            </ResponsiveContainer>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Transactions</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          {groups.length === 0 && <p className="text-muted-foreground">No transactions yet.</p>}
          {groups.map((group) => {
            const isOpen = !collapsedGroups.has(group.key);
            return (
              <Collapsible
                key={group.key}
                open={isOpen}
                onOpenChange={() => toggleGroup(group.key)}
              >
                <CollapsibleTrigger className="flex w-full items-center justify-between rounded-lg border border-border bg-secondary px-4 py-3 text-left">
                  <span className="flex items-center gap-2 font-medium">
                    {isOpen ? (
                      <ChevronDown className="size-4" />
                    ) : (
                      <ChevronRight className="size-4" />
                    )}
                    {group.label}
                    <span className="text-sm font-normal text-muted-foreground">
                      ({group.items.length})
                    </span>
                  </span>
                </CollapsibleTrigger>
                <CollapsibleContent>
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Asset</TableHead>
                        <TableHead>Investment type</TableHead>
                        <TableHead>Order type</TableHead>
                        <TableHead>Quantity</TableHead>
                        <TableHead>Unit price</TableHead>
                        <TableHead>Total</TableHead>
                        <TableHead>Running qty</TableHead>
                        <TableHead>Date</TableHead>
                        <TableHead>Source</TableHead>
                        <TableHead>Actions</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {group.items.map((tx) => {
                        const isConfirming = confirmingDeleteId === tx.id;
                        return (
                          <TableRow key={tx.id}>
                            <TableCell>{tx.ticker ?? "—"}</TableCell>
                            <TableCell>
                              {tx.asset_class
                                ? (ASSET_CLASS_LABELS[tx.asset_class as AssetClass] ??
                                  tx.asset_class)
                                : "—"}
                            </TableCell>
                            <TableCell>
                              <OrderTypeBadge type={tx.transaction_type} />
                            </TableCell>
                            <TableCell>{formatQuantity(tx.quantity)}</TableCell>
                            <TableCell>{tx.unit_price?.toFixed(2) ?? "—"}</TableCell>
                            <TableCell>
                              {tx.total_value.toFixed(2)}
                              {tx.currency !== tx.base_currency && (
                                <span className="ml-1 text-xs text-muted-foreground" data-testid="ledger-converted">
                                  {tx.currency} ·{" "}
                                  {tx.total_value_in_base != null
                                    ? `≈ ${tx.total_value_in_base.toFixed(2)} ${tx.base_currency}`
                                    : `no ${tx.currency}/${tx.base_currency} rate`}
                                </span>
                              )}
                            </TableCell>
                            <TableCell>{formatQuantity(tx.running_quantity)}</TableCell>
                            <TableCell>{tx.transaction_date}</TableCell>
                            <TableCell>
                              <Badge variant="outline">Manual</Badge>
                            </TableCell>
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
                </CollapsibleContent>
              </Collapsible>
            );
          })}
        </CardContent>
      </Card>
    </div>
  );
}

export default LedgerSection;
