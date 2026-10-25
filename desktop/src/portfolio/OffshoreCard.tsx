import { Fragment, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Card,
  CardContent,
  CardDescription,
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

// Entidades controladas no exterior (Lei 14.754/2023): o usuário cadastra a entidade e informa o resultado de cada
// exercício; a apuração (lucro × participação × PTAX de 31/12, 15%, crédito do imposto pago lá fora) vem pronta de
// `get_tax_assessment` (`offshore_years`). Aqui só cadastra e exibe.

export type OffshoreLine = {
  entity_id: number;
  entity_name: string;
  country: string;
  currency: string;
  ownership_percent: number;
  profit: number;
  fx_rate: number;
  fx_source: "ptax" | "market";
  profit_share_brl: number;
  loss_carried_in: number;
  taxable_base: number;
  tax_before_credit: number;
  tax_paid_abroad_brl: number;
  credit_used: number;
  tax_due: number;
  loss_carried_out: number;
};

export type OffshoreYear = {
  year: number;
  rate: number;
  profit_share_brl: number;
  taxable_base: number;
  tax_before_credit: number;
  credit_used: number;
  tax_due: number;
  lines: OffshoreLine[];
};

// Entidade ativa (art. 6º): o imposto vem das distribuições recebidas, por ano de recebimento (`offshore_active_years`).
export type ActiveDistributionLine = {
  distribution_id: number;
  entity_id: number;
  entity_name: string;
  country: string;
  currency: string;
  received_on: string;
  amount: number;
  // A parte coberta pelo crédito de 2023 (art. 14 §4º III), na moeda da entidade; `amount_brl` é só a parte tributável.
  covered_amount: number;
  fx_rate: number;
  fx_source: "ptax" | "market";
  amount_brl: number;
};

export type OffshoreActiveYear = {
  year: number;
  rate: number;
  taxable_base: number;
  tax_due: number;
  lines: ActiveDistributionLine[];
};

type OffshoreResultRow = {
  id: number;
  entity_id: number;
  year: number;
  profit: number;
  tax_paid_abroad: number;
};

type OffshoreDistributionRow = {
  id: number;
  entity_id: number;
  origin_year: number;
  received_on: string;
  amount: number;
};

type Regime = "automatico" | "ativa";

type OffshoreEntityRow = {
  id: number;
  workspace_id: number;
  name: string;
  country: string;
  currency: string;
  ownership_percent: number;
  regime: Regime;
  update_2023_equity: number | null;
  update_2023_profits: number | null;
  notes: string | null;
  results: OffshoreResultRow[];
  distributions: OffshoreDistributionRow[];
};

type EntityForm = { name: string; country: string; currency: string; ownership: string; regime: Regime };

const EMPTY_FORM: EntityForm = { name: "", country: "", currency: "USD", ownership: "100", regime: "automatico" };

function formatBrl(value: number): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency: "BRL" });
}

function formatAmount(value: number, currency: string): string {
  return value.toLocaleString("en-US", { style: "currency", currency });
}

// As distribuições tributadas na disponibilização: todas as da entidade ativa e, na comum, só as de lucro de 2023 ou antes.
function taxedDistributions(entity: OffshoreEntityRow): OffshoreDistributionRow[] {
  return entity.regime === "ativa" ? entity.distributions : entity.distributions.filter((d) => d.origin_year === 2023);
}

// Atualização opcional de 2023 da participação (art. 14 §1º IV da Lei 14.754): o patrimônio líquido proporcional à
// participação em 31/12/2023 e, dentro dele, a parcela de lucros, os dois na moeda da entidade, como saem das demonstrações
// financeiras. O app converte pela PTAX de venda do último dia útil de dezembro de 2023 (a conta fica no cartão de Bens e
// Direitos). Só vale para quem optou e pagou os 8% até 31/05/2024; o app não confere o direito à opção nem calcula o imposto.
function UpdateEditor({ workspaceId, entity }: { workspaceId: number; entity: OffshoreEntityRow }) {
  const queryClient = useQueryClient();
  const [equity, setEquity] = useState(entity.update_2023_equity === null ? "" : String(entity.update_2023_equity));
  const [profits, setProfits] = useState(entity.update_2023_profits === null ? "" : String(entity.update_2023_profits));
  const save = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("set_offshore_update_2023", {
        request: {
          entity_id: entity.id,
          equity: equity.trim() === "" ? null : Number(equity),
          profits: profits.trim() === "" ? null : Number(profits),
        },
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["offshore-entities", workspaceId] });
      void queryClient.invalidateQueries({ queryKey: ["offshore-credits", workspaceId] });
      void queryClient.invalidateQueries({ queryKey: ["tax-assessment", workspaceId] });
    },
  });
  return (
    <div className="flex flex-col gap-1" data-testid={`offshore-update2023-${entity.id}`}>
      <div className="flex flex-wrap items-center gap-1">
        <Input
          className="h-7 w-44 text-xs"
          type="number"
          step="0.01"
          min={0}
          placeholder={`Equity 31/12/2023, ${entity.currency}`}
          aria-label={`Proportional equity of ${entity.name} on 31/12/2023`}
          value={equity}
          data-testid={`offshore-update2023-equity-${entity.id}`}
          onChange={(e) => setEquity(e.target.value)}
        />
        <Input
          className="h-7 w-44 text-xs"
          type="number"
          step="0.01"
          min={0}
          placeholder={`of which profits, ${entity.currency}`}
          aria-label={`Profits share of the equity of ${entity.name} on 31/12/2023`}
          value={profits}
          data-testid={`offshore-update2023-profits-${entity.id}`}
          onChange={(e) => setProfits(e.target.value)}
        />
        <Button
          variant="outline"
          size="sm"
          disabled={save.isPending}
          data-testid={`offshore-update2023-save-${entity.id}`}
          onClick={() => save.mutate()}
        >
          Save
        </Button>
        <span className="text-xs text-muted-foreground">Art. 14 update (8% paid by 31/05/2024); empty equity = none</span>
      </div>
      {save.isError && <span className="text-xs text-destructive">{save.error.message}</span>}
    </div>
  );
}

export default function OffshoreCard({
  workspaceId,
  years,
  activeYears,
}: {
  workspaceId: number;
  years: OffshoreYear[];
  activeYears: OffshoreActiveYear[];
}) {
  const queryClient = useQueryClient();
  const [expanded, setExpanded] = useState<number | null>(null);
  const [distEntityId, setDistEntityId] = useState<number | null>(null);
  const [distDate, setDistDate] = useState("");
  const [distAmount, setDistAmount] = useState("");
  const [distFrom2023, setDistFrom2023] = useState(false);
  const [formOpen, setFormOpen] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [form, setForm] = useState<EntityForm>(EMPTY_FORM);
  const [resultEntityId, setResultEntityId] = useState<number | null>(null);
  const [resultYear, setResultYear] = useState(String(new Date().getFullYear() - 1));
  const [resultProfit, setResultProfit] = useState("");
  const [resultTax, setResultTax] = useState("0");

  const entitiesQuery = useQuery<OffshoreEntityRow[], AppError>({
    queryKey: ["offshore-entities", workspaceId],
    queryFn: () => invoke("list_offshore_entities", { workspaceId }),
  });
  const entities = entitiesQuery.data ?? [];

  // Mudar o cadastro muda a apuração (e a PTAX a buscar): refaz as duas consultas.
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["offshore-entities", workspaceId] });
    void queryClient.invalidateQueries({ queryKey: ["tax-assessment", workspaceId] });
    void queryClient.invalidateQueries({ queryKey: ["offshore-credits", workspaceId] });
  };

  const saveEntityMutation = useMutation<unknown, AppError, void>({
    mutationFn: () => {
      const entity = {
        name: form.name,
        country: form.country,
        currency: form.currency,
        ownership_percent: Number(form.ownership),
        regime: form.regime,
        notes: null,
      };
      return editingId === null
        ? invoke("create_offshore_entity", { request: { workspace_id: workspaceId, ...entity } })
        : invoke("update_offshore_entity", { request: { id: editingId, ...entity } });
    },
    onSuccess: () => {
      closeForm();
      refresh();
    },
  });

  const deleteEntityMutation = useMutation<void, AppError, number>({
    mutationFn: (entityId) => invoke("delete_offshore_entity", { entityId }),
    onSuccess: refresh,
  });

  const saveResultMutation = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("upsert_offshore_result", {
        request: {
          entity_id: resultEntityId,
          year: Number(resultYear),
          profit: Number(resultProfit),
          tax_paid_abroad: Number(resultTax || "0"),
        },
      }),
    onSuccess: () => {
      setResultEntityId(null);
      setResultProfit("");
      setResultTax("0");
      refresh();
    },
  });

  // Distribuição de entidade ativa: sem ano de origem, o ano do recebimento é o que vale (o comando ignora `origin_year`),
  // exceto a marcação "lucro de 2023" (2023), que consome o crédito da atualização do art. 14.
  const addDistributionMutation = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("add_offshore_distribution", {
        request: {
          entity_id: distEntityId,
          // Entidade comum: este formulário só lança lucro de 2023 ou antes (art. 6º I); o de 2024 em diante já foi tributado em 31/12.
          origin_year: distFrom2023 || entities.find((e) => e.id === distEntityId)?.regime === "automatico" ? 2023 : 0,
          received_on: distDate,
          amount: Number(distAmount),
        },
      }),
    onSuccess: () => {
      setDistEntityId(null);
      setDistDate("");
      setDistAmount("");
      setDistFrom2023(false);
      refresh();
    },
  });

  const deleteDistributionMutation = useMutation<void, AppError, number>({
    mutationFn: (distributionId) => invoke("delete_offshore_distribution", { distributionId }),
    onSuccess: refresh,
  });

  const deleteResultMutation = useMutation<void, AppError, number>({
    mutationFn: (resultId) => invoke("delete_offshore_result", { resultId }),
    onSuccess: refresh,
  });

  function closeForm() {
    setFormOpen(false);
    setEditingId(null);
    setForm(EMPTY_FORM);
    saveEntityMutation.reset();
  }

  function openEdit(entity: OffshoreEntityRow) {
    setEditingId(entity.id);
    setForm({
      name: entity.name,
      country: entity.country,
      currency: entity.currency,
      ownership: String(entity.ownership_percent),
      regime: entity.regime,
    });
    setFormOpen(true);
  }

  function onSubmitEntity(event: FormEvent) {
    event.preventDefault();
    saveEntityMutation.mutate();
  }

  function onSubmitResult(event: FormEvent) {
    event.preventDefault();
    saveResultMutation.mutate();
  }

  function onSubmitDistribution(event: FormEvent) {
    event.preventDefault();
    addDistributionMutation.mutate();
  }

  return (
    <Card data-testid="offshore-card">
      <CardHeader>
        <CardTitle>Controlled entities abroad (Law 14,754/2023)</CardTitle>
        <CardDescription>
          The profit of a company you control abroad is taxed every 31/12 at a flat rate, whether or not it was
          distributed. Register the entity and, for each closed year, its profit (or loss) and the tax it paid
          there, both in the entity's currency. Anchor takes your share, converts it at the Central Bank's PTAX of
          31/12 (selling rate; the buying rate for the tax paid abroad, which is a credit up to the tax due) and
          estimates the tax. A loss only offsets later profits of the same entity. It goes in the annual return,
          not a monthly DARF. Trusts and the legal test of "control" are not handled. This is an estimate based on
          secondary sources — confirm in the return.
        </CardDescription>
        <CardDescription>
          The 31/12 rule applies to entities in a favored-tax country or privileged regime, or whose own active
          income is under 60% of the total (art. 5, §5). A controlled entity outside that (an operating company) is
          marked <em>Active</em>: it is taxed only when profit is made available to you (art. 6) — register each
          distribution you receive and Anchor taxes it at 15% at the PTAX of the day it was received. Anchor does not
          check which case an entity falls in — you choose. Credit, lending to you and the option of art. 8 (declaring
          the entity's assets as your own) are not handled.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {years.length > 0 && (
          <Table data-testid="offshore-tax-table">
            <TableHeader>
              <TableRow>
                <TableHead>Year</TableHead>
                <TableHead className="text-right">Your share of the profit</TableHead>
                <TableHead className="text-right">Taxable base</TableHead>
                <TableHead className="text-right">Credit</TableHead>
                <TableHead className="text-right">Estimated tax</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {[...years].reverse().map((y) => {
                const isOpen = expanded === y.year;
                return (
                  <Fragment key={y.year}>
                    <TableRow>
                      <TableCell>{y.year}</TableCell>
                      <TableCell className="text-right">{formatBrl(y.profit_share_brl)}</TableCell>
                      <TableCell className="text-right">{formatBrl(y.taxable_base)}</TableCell>
                      <TableCell className="text-right">{y.credit_used > 0 ? formatBrl(y.credit_used) : "—"}</TableCell>
                      <TableCell className="text-right font-medium" data-testid={`offshore-tax-due-${y.year}`}>
                        {formatBrl(y.tax_due)}
                        <span className="block text-xs text-muted-foreground">at {(y.rate * 100).toFixed(1)}%</span>
                      </TableCell>
                      <TableCell className="flex justify-end">
                        <Button
                          variant="outline"
                          size="sm"
                          data-testid={`offshore-details-${y.year}`}
                          onClick={() => setExpanded(isOpen ? null : y.year)}
                        >
                          {isOpen ? "Hide" : "Details"}
                        </Button>
                      </TableCell>
                    </TableRow>
                    {isOpen && (
                      <TableRow>
                        <TableCell colSpan={6} className="bg-muted/30">
                          <Table>
                            <TableHeader>
                              <TableRow>
                                <TableHead>Entity</TableHead>
                                <TableHead className="text-right">Profit</TableHead>
                                <TableHead className="text-right">Share</TableHead>
                                <TableHead className="text-right">Rate (31/12)</TableHead>
                                <TableHead className="text-right">Loss used</TableHead>
                                <TableHead className="text-right">Base</TableHead>
                                <TableHead className="text-right">Tax paid abroad</TableHead>
                                <TableHead className="text-right">Tax</TableHead>
                              </TableRow>
                            </TableHeader>
                            <TableBody>
                              {y.lines.map((l) => (
                                <TableRow key={l.entity_id}>
                                  <TableCell>
                                    {l.entity_name}
                                    <span className="block text-xs text-muted-foreground">{l.country}</span>
                                  </TableCell>
                                  <TableCell className="text-right">{formatAmount(l.profit, l.currency)}</TableCell>
                                  <TableCell className="text-right">
                                    {l.ownership_percent}% = {formatBrl(l.profit_share_brl)}
                                  </TableCell>
                                  <TableCell className="text-right">
                                    {l.fx_rate.toFixed(4)}
                                    <span className="block text-xs text-muted-foreground">
                                      {l.fx_source === "ptax" ? "PTAX" : "market"}
                                    </span>
                                  </TableCell>
                                  <TableCell className="text-right">
                                    {l.loss_carried_in > 0 ? formatBrl(l.loss_carried_in - l.loss_carried_out) : "—"}
                                    {l.loss_carried_out > 0 && (
                                      <span className="block text-xs text-muted-foreground">
                                        {formatBrl(l.loss_carried_out)} carries on
                                      </span>
                                    )}
                                  </TableCell>
                                  <TableCell className="text-right">{formatBrl(l.taxable_base)}</TableCell>
                                  <TableCell className="text-right">
                                    {l.tax_paid_abroad_brl > 0 ? formatBrl(l.tax_paid_abroad_brl) : "—"}
                                  </TableCell>
                                  <TableCell className="text-right">{formatBrl(l.tax_due)}</TableCell>
                                </TableRow>
                              ))}
                            </TableBody>
                          </Table>
                        </TableCell>
                      </TableRow>
                    )}
                  </Fragment>
                );
              })}
            </TableBody>
          </Table>
        )}

        {activeYears.length > 0 && (
          <Table data-testid="offshore-active-table">
            <TableHeader>
              <TableRow>
                <TableHead>Year received</TableHead>
                <TableHead>Distributions taxed on receipt</TableHead>
                <TableHead className="text-right">Taxable base</TableHead>
                <TableHead className="text-right">Estimated tax</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {[...activeYears].reverse().map((y) => (
                <TableRow key={y.year}>
                  <TableCell>{y.year}</TableCell>
                  <TableCell className="text-xs text-muted-foreground">
                    {y.lines
                      .map((l) => {
                        const covered =
                          l.covered_amount > 0
                            ? `, ${formatAmount(l.covered_amount, l.currency)} covered by the 2023 credit`
                            : "";
                        // Inteira coberta sem câmbio: a taxa é 0 e não há o que converter.
                        const rate =
                          l.fx_rate > 0 ? ` at ${l.fx_rate.toFixed(4)} (${l.fx_source === "ptax" ? "PTAX" : "market"})` : "";
                        return `${l.entity_name}: ${formatAmount(l.amount, l.currency)} on ${l.received_on}${rate}${covered}`;
                      })
                      .join("; ")}
                  </TableCell>
                  <TableCell className="text-right">{formatBrl(y.taxable_base)}</TableCell>
                  <TableCell className="text-right font-medium" data-testid={`offshore-active-tax-due-${y.year}`}>
                    {formatBrl(y.tax_due)}
                    <span className="block text-xs text-muted-foreground">at {(y.rate * 100).toFixed(1)}%</span>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}

        {entities.map((entity) => (
          <div key={entity.id} className="flex flex-col gap-2 rounded-md border p-3" data-testid={`offshore-entity-${entity.id}`}>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div>
                <span className="font-medium">{entity.name}</span>
                {entity.regime === "ativa" && (
                  <Badge variant="secondary" className="ml-2" data-testid={`offshore-entity-active-${entity.id}`}>
                    Active — taxed when distributed
                  </Badge>
                )}
                <span className="block text-xs text-muted-foreground">
                  {entity.country} · {entity.currency} · {entity.ownership_percent}% owned
                </span>
              </div>
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  data-testid={`offshore-add-distribution-${entity.id}`}
                  onClick={() => {
                    setDistFrom2023(false);
                    setDistEntityId(entity.id);
                  }}
                >
                  {entity.regime === "ativa" ? "Add distribution" : "Add 2023-profits distribution"}
                </Button>
                <Button variant="outline" size="sm" onClick={() => setResultEntityId(entity.id)}>
                  Add year
                </Button>
                <Button variant="outline" size="sm" onClick={() => openEdit(entity)}>
                  Edit
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={deleteEntityMutation.isPending}
                  onClick={() => deleteEntityMutation.mutate(entity.id)}
                >
                  Delete
                </Button>
              </div>
            </div>
            {entity.results.length > 0 && (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Year</TableHead>
                    <TableHead className="text-right">Profit (loss)</TableHead>
                    <TableHead className="text-right">Tax paid abroad</TableHead>
                    <TableHead />
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {entity.results.map((r) => (
                    <TableRow key={r.id}>
                      <TableCell>{r.year}</TableCell>
                      <TableCell className="text-right">{formatAmount(r.profit, entity.currency)}</TableCell>
                      <TableCell className="text-right">
                        {r.tax_paid_abroad > 0 ? formatAmount(r.tax_paid_abroad, entity.currency) : "—"}
                      </TableCell>
                      <TableCell className="flex justify-end">
                        <Button
                          variant="outline"
                          size="sm"
                          disabled={deleteResultMutation.isPending}
                          onClick={() => deleteResultMutation.mutate(r.id)}
                        >
                          Remove
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
            {taxedDistributions(entity).length > 0 && (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Received on</TableHead>
                    <TableHead className="text-right">Distribution</TableHead>
                    <TableHead />
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {taxedDistributions(entity).map((d) => (
                    <TableRow key={d.id}>
                      <TableCell>
                        {d.received_on}
                        {d.origin_year === 2023 && (
                          <Badge variant="secondary" className="ml-2" data-testid={`offshore-distribution-2023-${d.id}`}>
                            2023 profits
                          </Badge>
                        )}
                      </TableCell>
                      <TableCell className="text-right">{formatAmount(d.amount, entity.currency)}</TableCell>
                      <TableCell className="flex justify-end">
                        <Button
                          variant="outline"
                          size="sm"
                          disabled={deleteDistributionMutation.isPending}
                          onClick={() => deleteDistributionMutation.mutate(d.id)}
                        >
                          Remove
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
            <UpdateEditor
              key={`${entity.id}:${entity.update_2023_equity}:${entity.update_2023_profits}`}
              workspaceId={workspaceId}
              entity={entity}
            />
            {distEntityId === entity.id && (
              <form onSubmit={onSubmitDistribution} className="grid grid-cols-1 items-end gap-3 sm:grid-cols-3">
                <Field label="Received on">
                  <Input
                    type="date"
                    required
                    min="2024-01-01"
                    value={distDate}
                    onChange={(e) => setDistDate(e.currentTarget.value)}
                    data-testid="offshore-distribution-date"
                  />
                </Field>
                <Field label={`Amount received, ${entity.currency}`}>
                  <Input
                    type="number"
                    step="0.01"
                    min={0.01}
                    required
                    value={distAmount}
                    onChange={(e) => setDistAmount(e.currentTarget.value)}
                    data-testid="offshore-distribution-amount"
                  />
                </Field>
                {entity.regime === "automatico" && (
                  <p className="text-sm text-muted-foreground sm:col-span-3" data-testid="offshore-distribution-2023-note">
                    Profits of 2023 or earlier are taxed at 15% when received (art. 6º, I), minus the part covered by the
                    2023 update. Profits of 2024 onwards were already taxed on 31/12: record them in the Bens e Direitos card.
                  </p>
                )}
                {entity.regime === "ativa" && (entity.update_2023_profits ?? 0) > 0 && (
                  <label className="flex items-center gap-2 text-sm sm:col-span-3">
                    <input
                      type="checkbox"
                      checked={distFrom2023}
                      onChange={(e) => setDistFrom2023(e.currentTarget.checked)}
                      data-testid="offshore-distribution-from-2023"
                    />
                    Profits of 2023 or earlier (covered by the 2023 update; only the excess is taxed)
                  </label>
                )}
                <div className="flex gap-2">
                  <Button
                    type="submit"
                    size="sm"
                    disabled={addDistributionMutation.isPending}
                    data-testid="offshore-distribution-save"
                  >
                    Save
                  </Button>
                  <Button type="button" variant="outline" size="sm" onClick={() => setDistEntityId(null)}>
                    Cancel
                  </Button>
                </div>
                {addDistributionMutation.isError && (
                  <p className="text-sm text-destructive sm:col-span-3">{addDistributionMutation.error.message}</p>
                )}
              </form>
            )}
            {resultEntityId === entity.id && (
              <form onSubmit={onSubmitResult} className="grid grid-cols-1 items-end gap-3 sm:grid-cols-4">
                <Field label="Year">
                  <Input
                    type="number"
                    required
                    min={2024}
                    value={resultYear}
                    onChange={(e) => setResultYear(e.currentTarget.value)}
                    data-testid="offshore-result-year"
                  />
                </Field>
                <Field label={`Profit (loss), ${entity.currency}`}>
                  <Input
                    type="number"
                    step="0.01"
                    required
                    value={resultProfit}
                    onChange={(e) => setResultProfit(e.currentTarget.value)}
                    data-testid="offshore-result-profit"
                  />
                </Field>
                <Field label={`Tax paid abroad, ${entity.currency}`}>
                  <Input
                    type="number"
                    step="0.01"
                    min={0}
                    value={resultTax}
                    onChange={(e) => setResultTax(e.currentTarget.value)}
                    data-testid="offshore-result-tax"
                  />
                </Field>
                <div className="flex gap-2">
                  <Button type="submit" size="sm" disabled={saveResultMutation.isPending} data-testid="offshore-result-save">
                    Save
                  </Button>
                  <Button type="button" variant="outline" size="sm" onClick={() => setResultEntityId(null)}>
                    Cancel
                  </Button>
                </div>
                {saveResultMutation.isError && (
                  <p className="text-sm text-destructive sm:col-span-4">{saveResultMutation.error.message}</p>
                )}
              </form>
            )}
          </div>
        ))}

        {formOpen ? (
          <form onSubmit={onSubmitEntity} className="grid grid-cols-1 items-end gap-3 sm:grid-cols-6">
            <Field label="Name">
              <Input
                required
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.currentTarget.value })}
                data-testid="offshore-entity-name"
              />
            </Field>
            <Field label="Country">
              <Input
                required
                value={form.country}
                onChange={(e) => setForm({ ...form, country: e.currentTarget.value })}
                data-testid="offshore-entity-country"
              />
            </Field>
            <Field label="Currency">
              <Input
                required
                maxLength={3}
                value={form.currency}
                onChange={(e) => setForm({ ...form, currency: e.currentTarget.value.toUpperCase() })}
                data-testid="offshore-entity-currency"
              />
            </Field>
            <Field label="Ownership (%)">
              <Input
                type="number"
                step="0.01"
                min={0.01}
                max={100}
                required
                value={form.ownership}
                onChange={(e) => setForm({ ...form, ownership: e.currentTarget.value })}
                data-testid="offshore-entity-ownership"
              />
            </Field>
            <Field label="Taxed">
              <Select value={form.regime} onValueChange={(v) => setForm({ ...form, regime: v as Regime })}>
                <SelectTrigger data-testid="offshore-entity-regime">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="automatico">Every 31/12 (art. 5)</SelectItem>
                  <SelectItem value="ativa">When distributed — active (art. 6)</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            <div className="flex gap-2">
              <Button type="submit" size="sm" disabled={saveEntityMutation.isPending} data-testid="offshore-entity-save">
                {editingId === null ? "Add" : "Save"}
              </Button>
              <Button type="button" variant="outline" size="sm" onClick={closeForm}>
                Cancel
              </Button>
            </div>
            {saveEntityMutation.isError && (
              <p className="text-sm text-destructive sm:col-span-6">{saveEntityMutation.error.message}</p>
            )}
          </form>
        ) : (
          <div>
            <Button variant="outline" size="sm" data-testid="offshore-add-entity" onClick={() => setFormOpen(true)}>
              Add entity
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
