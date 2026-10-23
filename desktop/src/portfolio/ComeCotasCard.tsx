import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// Come-cotas dos fundos abertos (estimativa): o usuário escolhe o tipo de cada fundo e lança o valor da cota; a conta
// (base, alíquota, cotas descontadas) vem pronta de `get_funds_overview`. Aqui só cadastra e exibe. O desconto real do
// fundo não é registrado (próxima fatia): depois que ele acontecer, lance as cotas retiradas como uma venda.

type FundTerm = "longo_prazo" | "curto_prazo" | "acoes";

type QuotaRow = { id: number; asset_id: number; quota_date: string; value: number };

type FundRow = {
  asset_id: number;
  ticker: string;
  name: string;
  term: FundTerm | null;
  quantity: number;
  average_cost: number | null;
  quotas: QuotaRow[];
};

type Estimate = {
  asset_id: number;
  status: "estimated" | "exempt" | "no_term" | "no_quota" | "no_position";
  reference_date: string;
  quotas: number;
  quota_value: number | null;
  quota_date: string | null;
  basis_per_quota: number | null;
  basis_source: "cost" | "last_come_cotas" | null;
  gain: number;
  rate: number;
  tax: number;
  quotas_deducted: number;
  stale_quota: boolean;
};

type FundsOverview = { funds: FundRow[]; estimates: Estimate[] };

const TERM_LABELS: Record<FundTerm, string> = {
  longo_prazo: "Long term (15%)",
  curto_prazo: "Short term (20%)",
  acoes: "Equity (no come-cotas)",
};

function formatBrl(value: number): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency: "BRL" });
}

function formatQuotas(value: number): string {
  return value.toLocaleString("pt-BR", { maximumFractionDigits: 4 });
}

function formatDate(date: string): string {
  const [y, m, d] = date.slice(0, 10).split("-");
  return `${d}/${m}/${y}`;
}

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

function EstimateLine({ estimate }: { estimate: Estimate }) {
  switch (estimate.status) {
    case "no_position":
      return <p className="text-sm text-muted-foreground">No quotas in the portfolio.</p>;
    case "no_term":
      return <p className="text-sm text-muted-foreground">Choose the fund type to estimate the come-cotas.</p>;
    case "no_quota":
      return <p className="text-sm text-muted-foreground">Enter a quota value to estimate the come-cotas.</p>;
    case "exempt":
      return <p className="text-sm text-muted-foreground">Equity funds have no come-cotas.</p>;
    case "estimated":
      return (
        <div className="flex flex-col gap-1 text-sm">
          <p>
            Next come-cotas on <span className="font-medium">{formatDate(estimate.reference_date)}</span>:{" "}
            <span className="font-medium">{formatBrl(estimate.tax)}</span> at {(estimate.rate * 100).toFixed(1)}% on a
            gain of {formatBrl(estimate.gain)} ({formatQuotas(estimate.quotas_deducted)} quotas of{" "}
            {formatQuotas(estimate.quotas)}).
          </p>
          <p className="text-xs text-muted-foreground">
            Quota {estimate.quota_value !== null ? formatBrl(estimate.quota_value) : "—"} on{" "}
            {estimate.quota_date ? formatDate(estimate.quota_date) : "—"} against a basis of{" "}
            {estimate.basis_per_quota !== null ? formatBrl(estimate.basis_per_quota) : "—"} per quota (
            {estimate.basis_source === "last_come_cotas" ? "the quota value at the last come-cotas" : "your average cost"}
            ).
            {estimate.stale_quota && (
              <Badge variant="outline" className="ml-2" data-testid={`come-cotas-stale-${estimate.asset_id}`}>
                old quota
              </Badge>
            )}
          </p>
        </div>
      );
  }
}

export default function ComeCotasCard({ workspaceId }: { workspaceId: number }) {
  const queryClient = useQueryClient();
  const [quotaFundId, setQuotaFundId] = useState<number | null>(null);
  const [quotaDate, setQuotaDate] = useState(todayIso());
  const [quotaValue, setQuotaValue] = useState("");

  const overviewQuery = useQuery<FundsOverview, AppError>({
    queryKey: ["funds-overview", workspaceId],
    queryFn: () => invoke("get_funds_overview", { workspaceId }),
  });

  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["funds-overview", workspaceId] });

  const setTermMutation = useMutation<void, AppError, { assetId: number; term: FundTerm }>({
    mutationFn: ({ assetId, term }) => invoke("set_fund_term", { assetId, term }),
    onSuccess: refresh,
  });

  const saveQuotaMutation = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("upsert_fund_quota", {
        request: { asset_id: quotaFundId, quota_date: quotaDate, value: Number(quotaValue) },
      }),
    onSuccess: () => {
      setQuotaFundId(null);
      setQuotaValue("");
      refresh();
      // O valor da cota também é o preço da posição.
      void queryClient.invalidateQueries({ queryKey: ["positions"] });
    },
  });

  const deleteQuotaMutation = useMutation<void, AppError, number>({
    mutationFn: (quotaId) => invoke("delete_fund_quota", { quotaId }),
    onSuccess: refresh,
  });

  const funds = overviewQuery.data?.funds ?? [];
  const estimates = new Map((overviewQuery.data?.estimates ?? []).map((e) => [e.asset_id, e]));
  if (funds.length === 0) return null;

  function onSubmitQuota(event: FormEvent) {
    event.preventDefault();
    saveQuotaMutation.mutate();
  }

  return (
    <Card data-testid="come-cotas-card">
      <CardHeader>
        <CardTitle>Funds — come-cotas (estimate)</CardTitle>
        <CardDescription>
          Open funds withhold income tax from you in quotas on the last business day of May and November: 15% on
          long-term funds, 20% on short-term ones, none on equity funds. Choose each fund's type and enter its quota
          value; Anchor estimates the next deduction from your latest quota value (it does not project to the date) and
          your position. After the fund deducts, register the quotas taken as a sale so the position stays right — a
          dedicated entry and the tax at redemption are not handled yet. This is an estimate based on secondary
          sources — confirm with your fund statement.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {funds.map((fund) => {
          const estimate = estimates.get(fund.asset_id);
          return (
            <div key={fund.asset_id} className="flex flex-col gap-3 rounded-md border p-3" data-testid={`fund-${fund.ticker}`}>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <span className="font-medium">{fund.ticker}</span>
                  <span className="block text-xs text-muted-foreground">
                    {fund.name} · {formatQuotas(fund.quantity)} quotas
                    {fund.average_cost !== null && ` · average cost ${formatBrl(fund.average_cost)}`}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <Select
                    value={fund.term ?? ""}
                    onValueChange={(term) => setTermMutation.mutate({ assetId: fund.asset_id, term: term as FundTerm })}
                  >
                    <SelectTrigger className="w-52" data-testid={`fund-term-${fund.ticker}`}>
                      <SelectValue placeholder="Fund type…" />
                    </SelectTrigger>
                    <SelectContent>
                      {(Object.keys(TERM_LABELS) as FundTerm[]).map((term) => (
                        <SelectItem key={term} value={term}>
                          {TERM_LABELS[term]}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Button variant="outline" size="sm" onClick={() => setQuotaFundId(fund.asset_id)}>
                    Add quota value
                  </Button>
                </div>
              </div>

              {estimate && (
                <div data-testid={`come-cotas-estimate-${fund.ticker}`}>
                  <EstimateLine estimate={estimate} />
                </div>
              )}

              {fund.quotas.length > 0 && (
                <p className="flex flex-wrap gap-2 text-xs text-muted-foreground">
                  {[...fund.quotas].reverse().slice(0, 6).map((q) => (
                    <span key={q.id} className="inline-flex items-center gap-1 rounded border px-2 py-0.5">
                      {formatDate(q.quota_date)}: {formatBrl(q.value)}
                      <button
                        type="button"
                        aria-label={`Remove the quota value of ${formatDate(q.quota_date)}`}
                        className="text-muted-foreground hover:text-foreground"
                        disabled={deleteQuotaMutation.isPending}
                        onClick={() => deleteQuotaMutation.mutate(q.id)}
                      >
                        ×
                      </button>
                    </span>
                  ))}
                </p>
              )}

              {quotaFundId === fund.asset_id && (
                <form onSubmit={onSubmitQuota} className="grid grid-cols-1 items-end gap-3 sm:grid-cols-3">
                  <Field label="Date">
                    <Input
                      type="date"
                      required
                      max={todayIso()}
                      value={quotaDate}
                      onChange={(e) => setQuotaDate(e.currentTarget.value)}
                      data-testid="fund-quota-date"
                    />
                  </Field>
                  <Field label="Quota value (BRL)">
                    <Input
                      type="number"
                      step="0.00000001"
                      min={0}
                      required
                      value={quotaValue}
                      onChange={(e) => setQuotaValue(e.currentTarget.value)}
                      data-testid="fund-quota-value"
                    />
                  </Field>
                  <div className="flex gap-2">
                    <Button type="submit" size="sm" disabled={saveQuotaMutation.isPending} data-testid="fund-quota-save">
                      Save
                    </Button>
                    <Button type="button" variant="outline" size="sm" onClick={() => setQuotaFundId(null)}>
                      Cancel
                    </Button>
                  </div>
                  {saveQuotaMutation.isError && (
                    <p className="text-sm text-destructive sm:col-span-3">{saveQuotaMutation.error.message}</p>
                  )}
                </form>
              )}
            </div>
          );
        })}
        {setTermMutation.isError && <p className="text-sm text-destructive">{setTermMutation.error.message}</p>}
      </CardContent>
    </Card>
  );
}
