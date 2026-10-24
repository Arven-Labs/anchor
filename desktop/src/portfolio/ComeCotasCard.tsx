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

// Come-cotas dos fundos abertos (estimativa): o usuário escolhe o tipo de cada fundo; o valor da cota vem do informe
// diário da CVM (com o CNPJ do fundo) ou é lançado à mão; a conta (base, alíquota, cotas descontadas) vem pronta de
// `get_funds_overview`. Aqui só cadastra e exibe. O desconto real do fundo é registrado como uma transação "Come-cotas"
// no formulário de lançamentos; esta tela só lista os já registrados. Cada venda do fundo aparece como resgate, com o
// IOF e o IR estimados (informativo: o imposto sai na fonte).

type FundTerm = "longo_prazo" | "curto_prazo" | "acoes" | "entidade_investimento";

type FundStructure = "aberto" | "fechado" | "exclusivo";

type EstoqueOption = "geral" | "opcao_8";

type EstoqueEstimate = { date: string; quota_value: number; rate: number; base: number; tax: number };

type EstoqueStatus =
  | { kind: "not_applicable" }
  | { kind: "not_taxed" }
  | { kind: "needs_quota" }
  | { kind: "estimated"; estimate: EstoqueEstimate };

type QuotaRow = { id: number; asset_id: number; quota_date: string; value: number; source: "manual" | "cvm" };

type CvmStatus =
  | { kind: "no_cnpj" }
  | { kind: "pending" }
  | { kind: "ok" }
  | { kind: "not_found" }
  | { kind: "needs_subclass"; options: string[] }
  | { kind: "error"; message: string };

type DeductionRow = {
  tx_id: number;
  date: string;
  quotas: number;
  quota_value: number;
  tax: number;
  // Rendimento, em BRL, que o crédito de prejuízo cobriu nesse desconto; null = não informado.
  loss_credit_used: number | null;
};

type LotPart = { lot_date: string; quotas: number; days: number; rate: number; gain: number; iof: number; ir: number };

type RedemptionRow = {
  tx_id: number;
  date: string;
  quotas: number;
  quota_value: number;
  gross: number;
  cost: number;
  iof: number;
  ir: number;
  come_cotas_credit: number;
  net: number;
  lots: LotPart[];
  uncovered_quotas: number;
  excess_come_cotas: number;
  loss_created: number;
  loss_credit_used: number;
  ir_before_compensation: number;
  is_amortization: boolean;
};

type LossCredit = {
  administrator: string | null;
  term: FundTerm;
  tickers: string[];
  asset_ids: number[];
  amount: number;
};

type FundRow = {
  asset_id: number;
  ticker: string;
  name: string;
  term: FundTerm | null;
  quantity: number;
  average_cost: number | null;
  cnpj: string | null;
  cvm_subclass: string | null;
  cvm_status: CvmStatus;
  administrator: string | null;
  structure: FundStructure;
  estoque_option: EstoqueOption | null;
  estoque: EstoqueStatus;
  last_cvm_date: string | null;
  quotas: QuotaRow[];
  deductions: DeductionRow[];
  redemptions: RedemptionRow[];
  // Come-cotas registrados feitos com saldo de prejuízo no grupo: a administradora pode ter usado o crédito ali.
  // informed: null = o crédito usado não foi informado; número = o valor informado, acima do saldo.
  credit_notes: { asset_id: number; date: string; balance: number; informed: number | null }[];
};

type Estimate = {
  asset_id: number;
  status: "estimated" | "exempt" | "no_term" | "no_quota" | "no_position";
  reference_date: string;
  quotas: number;
  quota_value: number | null;
  quota_date: string | null;
  basis_per_quota: number | null;
  basis_source: "cost" | "last_come_cotas" | "estoque" | null;
  gain: number;
  rate: number;
  tax: number;
  quotas_deducted: number;
  stale_quota: boolean;
  last_reference_unrecorded: string | null;
  loss_credit_used: number;
  tax_before_compensation: number;
};

type FundsOverview = { funds: FundRow[]; estimates: Estimate[]; loss_credits: LossCredit[] };

const TERM_LABELS: Record<FundTerm, string> = {
  longo_prazo: "Long term (15%)",
  curto_prazo: "Short term (20%)",
  acoes: "Equity (no come-cotas)",
  entidade_investimento: "Investment entity: FIP, FIDC, ETF (15%, no come-cotas)",
};

const STRUCTURE_LABELS: Record<FundStructure, string> = {
  aberto: "Open fund",
  fechado: "Closed fund",
  exclusivo: "Exclusive fund",
};

const ESTOQUE_LABELS: Record<EstoqueOption | "none", string> = {
  none: "Not taxed / disputed (not estimated)",
  geral: "General rule: 15% (art. 27)",
  opcao_8: "Option: 8% (art. 28)",
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

// Come-cotas pago acima do IR devido, somado nos resgates e amortizações do fundo. É só informação: não entra no líquido.
function refundableTotal(redemptions: RedemptionRow[]): number {
  return redemptions.reduce((sum, r) => sum + r.excess_come_cotas, 0);
}

// IR e IOF estimados de um resgate (venda do fundo), com o detalhe por lote numa área expansível.
function RedemptionLine({ redemption, ticker }: { redemption: RedemptionRow; ticker: string }) {
  return (
    <details className="text-sm" data-testid={`fund-redemption-${ticker}-${redemption.tx_id}`}>
      <summary className="cursor-pointer">
        {redemption.is_amortization ? (
          <>
            {formatDate(redemption.date)}: amortization of {formatBrl(redemption.gross)} (quota{" "}
            {formatBrl(redemption.quota_value)}, the quotas stay)
          </>
        ) : (
          <>
            {formatDate(redemption.date)}: {formatQuotas(redemption.quotas)} quotas at{" "}
            {formatBrl(redemption.quota_value)} = {formatBrl(redemption.gross)} gross
          </>
        )}{" "}
        — IOF {formatBrl(redemption.iof)}, IR{" "}
        <span className="font-medium">{formatBrl(redemption.ir)}</span>, net{" "}
        <span className="font-medium">{formatBrl(redemption.net)}</span>
        {redemption.uncovered_quotas > 0 && (
          <Badge variant="destructive" className="ml-2">
            incomplete
          </Badge>
        )}
        {redemption.excess_come_cotas > 0 && (
          <Badge variant="outline" className="ml-2">
            excess come-cotas
          </Badge>
        )}
        {redemption.loss_created > 0 && (
          <Badge variant="outline" className="ml-2" data-testid={`fund-loss-${ticker}-${redemption.tx_id}`}>
            loss {formatBrl(redemption.loss_created)} to offset
          </Badge>
        )}
        {redemption.loss_credit_used > 0 && (
          <Badge variant="outline" className="ml-2" data-testid={`fund-credit-${ticker}-${redemption.tx_id}`}>
            loss credit {formatBrl(redemption.loss_credit_used)} used
          </Badge>
        )}
      </summary>
      <div className="mt-1 flex flex-col gap-1 pl-4 text-xs text-muted-foreground">
        {redemption.lots.map((lot, index) => (
          <span key={index}>
            Lot of {formatDate(lot.lot_date)}: {formatQuotas(lot.quotas)} quotas held {lot.days} days at{" "}
            {(lot.rate * 100).toFixed(1)}% — gain {formatBrl(lot.gain)}, IOF {formatBrl(lot.iof)}, IR {formatBrl(lot.ir)}
          </span>
        ))}
        {redemption.come_cotas_credit > 0 && (
          <span>Come-cotas already paid, credited against this IR: {formatBrl(redemption.come_cotas_credit)}</span>
        )}
        {redemption.excess_come_cotas > 0 && (
          <span>
            Come-cotas paid above the IR due: {formatBrl(redemption.excess_come_cotas)} (not in the net value; see the
            note above).
          </span>
        )}
        {redemption.loss_credit_used > 0 && (
          <span>
            Loss from earlier redemptions in the same group offset {formatBrl(redemption.loss_credit_used)} of the
            gain: IR {formatBrl(redemption.ir)} instead of {formatBrl(redemption.ir_before_compensation)}.
          </span>
        )}
        {redemption.loss_created > 0 && (
          <span>
            This redemption lost {formatBrl(redemption.loss_created)}: it can offset the gains of later redemptions and
            come-cotas in the same group (same administrator and fund type).
          </span>
        )}
        {redemption.uncovered_quotas > 0 && (
          <span>
            {formatQuotas(redemption.uncovered_quotas)} quotas of this redemption have no recorded purchase, so the
            cost is missing.
          </span>
        )}
      </div>
    </details>
  );
}

function EstimateLine({ estimate }: { estimate: Estimate }) {
  switch (estimate.status) {
    case "no_position":
      return <p className="text-sm text-muted-foreground">No quotas in the portfolio.</p>;
    case "no_term":
      return <p className="text-sm text-muted-foreground">Choose the fund type to estimate the come-cotas.</p>;
    case "no_quota":
      return (
        <p className="text-sm text-muted-foreground">
          Enter the fund's CNPJ to read its quota from the CVM, or add a quota value yourself, to estimate the come-cotas.
        </p>
      );
    case "exempt":
      return (
        <p className="text-sm text-muted-foreground">
          Equity funds and investment entities (FIP, FIDC, ETF) have no come-cotas.
        </p>
      );
    case "estimated":
      return (
        <div className="flex flex-col gap-1 text-sm">
          <p>
            Next come-cotas on <span className="font-medium">{formatDate(estimate.reference_date)}</span>:{" "}
            <span className="font-medium">{formatBrl(estimate.tax)}</span> at {(estimate.rate * 100).toFixed(1)}% on a
            gain of {formatBrl(estimate.gain)} ({formatQuotas(estimate.quotas_deducted)} quotas of{" "}
            {formatQuotas(estimate.quotas)}).
          </p>
          {estimate.loss_credit_used > 0 && (
            <p className="text-xs text-muted-foreground" data-testid={`come-cotas-credit-${estimate.asset_id}`}>
              A loss credit of {formatBrl(estimate.loss_credit_used)} already reduced the gain: the tax would be{" "}
              {formatBrl(estimate.tax_before_compensation)} without it.
            </p>
          )}
          <p className="text-xs text-muted-foreground">
            Quota {estimate.quota_value !== null ? formatBrl(estimate.quota_value) : "—"} on{" "}
            {estimate.quota_date ? formatDate(estimate.quota_date) : "—"} against a basis of{" "}
            {estimate.basis_per_quota !== null ? formatBrl(estimate.basis_per_quota) : "—"} per quota (
            {estimate.basis_source === "last_come_cotas"
              ? "the quota value at the last come-cotas"
              : estimate.basis_source === "estoque"
                ? "your average cost plus the 2023 stock already taxed"
                : "your average cost"}
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

// Onde a cota do fundo vem: o CNPJ (e a subclasse, quando o informe só as tem) para a leitura do informe diário da CVM.
function CvmSource({ fund, onChanged }: { fund: FundRow; onChanged: () => void }) {
  const [cnpj, setCnpj] = useState(fund.cnpj ?? "");
  const status = fund.cvm_status;
  const showSubclass = status.kind === "needs_subclass" || fund.cvm_subclass !== null;
  const options = status.kind === "needs_subclass" ? status.options : fund.cvm_subclass ? [fund.cvm_subclass] : [];

  const saveMutation = useMutation<void, AppError, { cnpj: string; subclass: string }>({
    mutationFn: ({ cnpj: nextCnpj, subclass }) =>
      invoke("set_fund_cvm", { request: { asset_id: fund.asset_id, cnpj: nextCnpj, subclass } }),
    onSuccess: onChanged,
  });

  return (
    <div className="flex flex-col gap-2" data-testid={`fund-cvm-${fund.ticker}`}>
      <form
        className="flex flex-wrap items-end gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          // A subclasse é de um CNPJ: trocar o CNPJ a descarta, salvar o mesmo CNPJ a mantém.
          saveMutation.mutate({ cnpj, subclass: cnpj.trim() === (fund.cnpj ?? "") ? (fund.cvm_subclass ?? "") : "" });
        }}
      >
        <Field label="Fund CNPJ (CVM)">
          <Input
            value={cnpj}
            placeholder="00.000.000/0000-00"
            onChange={(e) => setCnpj(e.currentTarget.value)}
            className="w-56"
            data-testid={`fund-cnpj-${fund.ticker}`}
          />
        </Field>
        <Button
          type="submit"
          variant="outline"
          size="sm"
          disabled={saveMutation.isPending}
          data-testid={`fund-cnpj-save-${fund.ticker}`}
        >
          Save CNPJ
        </Button>
        {showSubclass && (
          <Select
            value={fund.cvm_subclass ?? ""}
            onValueChange={(subclass) => saveMutation.mutate({ cnpj, subclass })}
          >
            <SelectTrigger className="w-56" data-testid={`fund-subclass-${fund.ticker}`}>
              <SelectValue placeholder="CVM subclass…" />
            </SelectTrigger>
            <SelectContent>
              {options.map((option) => (
                <SelectItem key={option} value={option}>
                  {option}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
      </form>
      <p className="text-xs text-muted-foreground" data-testid={`fund-cvm-status-${fund.ticker}`}>
        {status.kind === "no_cnpj" && "No CNPJ: the quota value is yours to enter."}
        {status.kind === "pending" && "Not read from the CVM yet."}
        {status.kind === "ok" && (
          <>
            <Badge variant="outline" className="mr-2">
              CVM
            </Badge>
            Latest quota from the CVM daily report{fund.last_cvm_date ? `: ${formatDate(fund.last_cvm_date)}` : ""}.
          </>
        )}
        {status.kind === "not_found" && "The CVM daily report has no quota for this CNPJ (or this subclass)."}
        {status.kind === "needs_subclass" &&
          (fund.term === null
            ? "This CNPJ has only subclasses in the CVM report. Choose the fund type first, then the subclass."
            : "This CNPJ has only subclasses in the CVM report: choose one.")}
        {status.kind === "error" && `Could not read the CVM report: ${status.message}`}
      </p>
      {saveMutation.isError && <p className="text-sm text-destructive">{saveMutation.error.message}</p>}
    </div>
  );
}

// Fundo fechado ou exclusivo (Lei 14.754/2023): o IR do art. 17 é o mesmo do aberto; o que muda é o estoque de rendimento
// até 31/12/2023 (arts. 27 e 28), que o usuário diz se foi tributado a 15%, a 8% ou nada. A estimativa precisa da cota de
// 31/12/2023 (ou de até 10 dias antes), lançada ou lida da CVM.
function ClosedFundFields({ fund, onChanged }: { fund: FundRow; onChanged: () => void }) {
  const structureMutation = useMutation<void, AppError, FundStructure>({
    mutationFn: (structure) => invoke("set_fund_structure", { assetId: fund.asset_id, structure }),
    onSuccess: onChanged,
  });
  const estoqueMutation = useMutation<void, AppError, EstoqueOption | "none">({
    mutationFn: (option) =>
      invoke("set_fund_estoque", { assetId: fund.asset_id, option: option === "none" ? null : option }),
    onSuccess: onChanged,
  });
  const showEstoque = fund.estoque.kind !== "not_applicable" || (fund.structure !== "aberto" && fund.term !== null);
  const status = fund.estoque;

  return (
    <div className="flex flex-col gap-2" data-testid={`fund-structure-${fund.ticker}`}>
      <div className="flex flex-wrap items-end gap-2">
        <Field label="Fund structure">
          <Select
            value={fund.structure}
            disabled={fund.term === null}
            onValueChange={(value) => structureMutation.mutate(value as FundStructure)}
          >
            <SelectTrigger className="w-52" data-testid={`fund-structure-select-${fund.ticker}`}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {(Object.keys(STRUCTURE_LABELS) as FundStructure[]).map((structure) => (
                <SelectItem key={structure} value={structure}>
                  {STRUCTURE_LABELS[structure]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        {showEstoque && (
          <Field label="Income accrued up to 2023 (stock)">
            <Select
              value={fund.estoque_option ?? "none"}
              onValueChange={(value) => estoqueMutation.mutate(value as EstoqueOption | "none")}
            >
              <SelectTrigger className="w-72" data-testid={`fund-estoque-select-${fund.ticker}`}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {(Object.keys(ESTOQUE_LABELS) as (EstoqueOption | "none")[]).map((option) => (
                  <SelectItem key={option} value={option}>
                    {ESTOQUE_LABELS[option]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
        )}
      </div>
      <p className="text-xs text-muted-foreground" data-testid={`fund-estoque-status-${fund.ticker}`}>
        {fund.term === null && "Choose the fund type first."}
        {fund.term !== null && status.kind === "not_applicable" && fund.structure === "aberto" &&
          "Open funds already paid come-cotas: they have no 2023 stock to tax."}
        {fund.term !== null && status.kind === "not_applicable" && fund.structure !== "aberto" &&
          "The 2023 stock only applies to long-term and short-term funds."}
        {status.kind === "not_taxed" && "The 2023 stock is not estimated. Pick how it was taxed to count it."}
        {status.kind === "needs_quota" &&
          "Add the fund's quota value for 31/12/2023 (or up to 10 days before) to estimate the 2023 stock."}
        {status.kind === "estimated" && (
          <>
            2023 stock: gain of {formatBrl(status.estimate.base)} up to {formatDate(status.estimate.date)} (quota{" "}
            {formatBrl(status.estimate.quota_value)}) taxed at {(status.estimate.rate * 100).toFixed(0)}% ={" "}
            <span className="font-medium">{formatBrl(status.estimate.tax)}</span>, paid by you in cash. It counts as tax
            already paid when the fund is redeemed. Whether the tax is due is disputed in court.
          </>
        )}
      </p>
      {(structureMutation.isError || estoqueMutation.isError) && (
        <p className="text-sm text-destructive">{(structureMutation.error ?? estoqueMutation.error)?.message}</p>
      )}
    </div>
  );
}

// Quem compensa prejuízo com quem: a lei só deixa entre fundos da mesma administradora (ou do mesmo intermediário), do mesmo
// tipo. O texto é livre; fundos com o mesmo texto (sem diferença de caixa ou de espaços) formam um grupo.
function AdministratorField({ fund, onChanged }: { fund: FundRow; onChanged: () => void }) {
  const [administrator, setAdministrator] = useState(fund.administrator ?? "");

  const saveMutation = useMutation<void, AppError, string>({
    mutationFn: (value) => invoke("set_fund_administrator", { assetId: fund.asset_id, administrator: value }),
    onSuccess: onChanged,
  });

  return (
    <div className="flex flex-col gap-1" data-testid={`fund-administrator-${fund.ticker}`}>
      <form
        className="flex flex-wrap items-end gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          saveMutation.mutate(administrator);
        }}
      >
        <Field label="Administrator / broker (to offset losses)">
          <Input
            value={administrator}
            placeholder="e.g. XP, BTG"
            disabled={fund.term === null}
            onChange={(e) => setAdministrator(e.currentTarget.value)}
            className="w-56"
            data-testid={`fund-administrator-input-${fund.ticker}`}
          />
        </Field>
        <Button
          type="submit"
          variant="outline"
          size="sm"
          disabled={fund.term === null || saveMutation.isPending}
          data-testid={`fund-administrator-save-${fund.ticker}`}
        >
          Save
        </Button>
      </form>
      <p className="text-xs text-muted-foreground">
        {fund.term === null
          ? "Choose the fund type first."
          : fund.administrator === null
            ? "Without it, this fund only offsets losses against its own later gains."
            : "Funds of the same type with the same administrator offset losses against each other's later gains."}
      </p>
      {saveMutation.isError && <p className="text-sm text-destructive">{saveMutation.error.message}</p>}
    </div>
  );
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

  // O CNPJ ou a subclasse mudou: a cota (e o preço da posição) vem de outra série.
  const cvmChanged = () => {
    refresh();
    void queryClient.invalidateQueries({ queryKey: ["positions"] });
  };

  const cvmRefreshMutation = useMutation<FundsOverview, AppError, void>({
    mutationFn: () => invoke("refresh_fund_quotas_now", { workspaceId }),
    onSuccess: (overview) => {
      queryClient.setQueryData(["funds-overview", workspaceId], overview);
      void queryClient.invalidateQueries({ queryKey: ["positions"] });
    },
  });

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
  const lossCredits = overviewQuery.data?.loss_credits ?? [];
  if (funds.length === 0) return null;

  function onSubmitQuota(event: FormEvent) {
    event.preventDefault();
    saveQuotaMutation.mutate();
  }

  return (
    <Card data-testid="come-cotas-card">
      <CardHeader>
        <CardTitle>Funds — come-cotas and redemption (estimate)</CardTitle>
        <CardDescription>
          Open funds withhold income tax from you in quotas on the last business day of May and November: 15% on
          long-term funds, 20% on short-term ones, none on equity funds. Choose each fund's type and give its CNPJ:
          Anchor reads the quota value from the CVM daily report (it lags one or two business days), and you can still
          enter a quota value yourself — yours is never overwritten. It estimates the next deduction from the latest
          quota value (it does not project to the date) and your position. After the fund deducts, register it as a Come-cotas transaction (quotas taken and the quota
          value of the day): the position, the average cost and the next estimate follow. A Sell of fund quotas is
          read as a redemption: enter the gross value (before IR and IOF) as the total, and Anchor estimates what the
          fund withheld — IR by the regressive table on each purchase lot (oldest first), IOF under 30 days, minus the
          come-cotas already paid. It is informational: the tax leaves the redemption at the source, so there is no
          DARF. A loss on a redemption offsets the gains of later redemptions and come-cotas in the same fund or in
          another fund of the same type and the same administrator (or broker): give each fund's administrator so Anchor
          can group them; without it a fund only offsets itself. A recorded come-cotas only uses up the credit if you
          fill in the loss credit used in its transaction (Anchor cannot know how much the administrator offset there:
          without the value it only warns when the come-cotas could have used it) and the credit has no expiry in this
          estimate. This is an estimate based on secondary sources — confirm with your fund statement.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="outline"
            size="sm"
            disabled={cvmRefreshMutation.isPending}
            onClick={() => cvmRefreshMutation.mutate()}
            data-testid="fund-cvm-refresh"
          >
            {cvmRefreshMutation.isPending ? "Reading the CVM…" : "Update from CVM"}
          </Button>
          {cvmRefreshMutation.isError && (
            <span className="text-sm text-destructive" data-testid="fund-cvm-refresh-error">
              {cvmRefreshMutation.error.message}
            </span>
          )}
        </div>
        {lossCredits.length > 0 && (
          <div className="flex flex-col gap-1 rounded-md border border-dashed p-3 text-sm" data-testid="fund-loss-credits">
            <span className="text-xs text-muted-foreground">
              Loss to offset against later gains and come-cotas (same administrator and fund type)
            </span>
            {lossCredits.map((credit) => (
              <span key={`${credit.administrator ?? ""}:${credit.term}:${credit.tickers.join(",")}`}>
                <span className="font-medium">{formatBrl(credit.amount)}</span> — {credit.tickers.join(", ")} (
                {credit.administrator ?? "no administrator"}, {TERM_LABELS[credit.term]})
              </span>
            ))}
          </div>
        )}
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

              {/* A chave segue o CNPJ gravado: o campo é estado local e remonta com o valor que chegou do banco. */}
              <CvmSource key={`${fund.asset_id}:${fund.cnpj ?? ""}`} fund={fund} onChanged={cvmChanged} />

              <ClosedFundFields
                key={`${fund.asset_id}:${fund.term ?? ""}:${fund.structure}:${fund.estoque_option ?? ""}`}
                fund={fund}
                onChanged={refresh}
              />

              <AdministratorField
                key={`${fund.asset_id}:${fund.term ?? ""}:${fund.administrator ?? ""}`}
                fund={fund}
                onChanged={refresh}
              />

              {estimate && (
                <div data-testid={`come-cotas-estimate-${fund.ticker}`}>
                  <EstimateLine estimate={estimate} />
                </div>
              )}

              {estimate?.last_reference_unrecorded && (
                <p className="text-sm text-amber-500" data-testid={`come-cotas-unrecorded-${fund.ticker}`}>
                  The come-cotas of {formatDate(estimate.last_reference_unrecorded)} is not recorded. If the fund
                  already took the quotas, add a Come-cotas entry in Transactions so the position and the next
                  estimate are right.
                </p>
              )}

              {fund.credit_notes.length > 0 && (
                <div className="flex flex-col gap-1 text-sm text-amber-500" data-testid={`credit-notes-${fund.ticker}`}>
                  {fund.credit_notes.map((note) => (
                    <p key={note.date}>
                      {note.informed === null ? (
                        <>
                          The come-cotas of {formatDate(note.date)} was recorded while the group had{" "}
                          {formatBrl(note.balance)} of loss credit: the administrator may have used part of it, so the
                          credit balance shown may be higher than the real one. Fill in the loss credit used in that
                          transaction to settle it.
                        </>
                      ) : (
                        <>
                          The come-cotas of {formatDate(note.date)} says {formatBrl(note.informed)} of loss credit was
                          used, but the group only had {formatBrl(note.balance)}: only that balance was taken off.
                          Check the value.
                        </>
                      )}
                    </p>
                  ))}
                </div>
              )}

              {fund.deductions.length > 0 && (
                <div className="flex flex-col gap-1 text-sm" data-testid={`come-cotas-deductions-${fund.ticker}`}>
                  <span className="text-xs text-muted-foreground">Recorded come-cotas</span>
                  {[...fund.deductions].reverse().map((d) => (
                    <span key={d.tx_id}>
                      {formatDate(d.date)}: {formatQuotas(d.quotas)} quotas at {formatBrl(d.quota_value)} ={" "}
                      <span className="font-medium">{formatBrl(d.tax)}</span>
                      {d.loss_credit_used !== null && <> · loss credit used {formatBrl(d.loss_credit_used)}</>}
                    </span>
                  ))}
                </div>
              )}

              {fund.redemptions.length > 0 && (
                <div className="flex flex-col gap-1" data-testid={`fund-redemptions-${fund.ticker}`}>
                  <span className="text-xs text-muted-foreground">Redemptions (estimated IR and IOF)</span>
                  {refundableTotal(fund.redemptions) > 0.005 && (
                    <span className="text-xs" data-testid={`fund-refundable-${fund.ticker}`}>
                      Come-cotas paid above the IR due:{" "}
                      <span className="font-medium">{formatBrl(refundableTotal(fund.redemptions))}</span>. The law
                      provides no refund of this excess (IN RFB 1.585/2015, art. 15; Lei 14.754/2023, art. 17, §6). It
                      is tax paid on income that later vanished, and it comes back as a loss to offset later gains.
                      The app does not add it to the loss credit yet, and it is not included in the net values below.
                    </span>
                  )}
                  {[...fund.redemptions].reverse().map((r) => (
                    <RedemptionLine key={r.tx_id} redemption={r} ticker={fund.ticker} />
                  ))}
                </div>
              )}

              {fund.quotas.length > 0 && (
                <p className="flex flex-wrap gap-2 text-xs text-muted-foreground">
                  {[...fund.quotas].reverse().slice(0, 6).map((q) => (
                    <span key={q.id} className="inline-flex items-center gap-1 rounded border px-2 py-0.5">
                      {formatDate(q.quota_date)}: {formatBrl(q.value)}
                      {q.source === "cvm" && <span className="text-[10px] uppercase">CVM</span>}
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
