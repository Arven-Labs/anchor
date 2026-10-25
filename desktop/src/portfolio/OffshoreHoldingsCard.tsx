import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import Field from "../components/Field";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

// Bens e Direitos das entidades controladas no exterior (Lei 14.754, art. 5º §§10 a 12): o lucro tributado em 31/12 entra na
// ficha como "crédito de dividendo a receber" da controlada, por ano de origem; a distribuição desse lucro reduz o crédito
// pelo valor originalmente declarado em reais e não é tributada de novo. O cálculo vem pronto do backend
// (`get_offshore_credits`); aqui só exibe, monta o texto da discriminação e cadastra as distribuições.

type CreditRow = {
  entity_id: number;
  entity_name: string;
  country: string;
  currency: string;
  origin_year: number;
  as_of_year: number;
  taxed_brl: number;
  distributed_brl: number;
  balance_brl: number;
  over_distributed_brl: number;
};

type DistributionView = {
  id: number;
  entity_id: number;
  origin_year: number;
  received_on: string;
  amount: number;
  reduction_brl: number | null;
};

// A atualização de 2023 (art. 14) de uma controlada, já em reais.
type UpdateRow = {
  entity_id: number;
  entity_name: string;
  country: string;
  currency: string;
  active_regime: boolean;
  equity: number;
  profits: number;
  fx_rate: number;
  fx_source: "ptax" | "market";
  equity_brl: number;
  profits_brl: number;
  additional_cost_brl: number;
};

type OffshoreCredits = { rows: CreditRow[]; distributions: DistributionView[]; updates: UpdateRow[] };

// O ano de origem do crédito que vem da atualização de 2023 (e não de um lucro tributado em 31/12).
const UPDATE_YEAR = 2023;

function formatBrl(value: number): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency: "BRL" });
}

function formatDate(date: string): string {
  const [y, m, d] = date.slice(0, 10).split("-");
  return `${d}/${m}/${y}`;
}

type CreditLine = {
  key: string;
  entityId: number;
  entityName: string;
  country: string;
  currency: string;
  originYear: number;
  previous?: CreditRow;
  current?: CreditRow;
};

// Um item por entidade e ano de origem que tinha crédito em 31/12/(ano−1) ou em 31/12/ano.
function linesOfYear(rows: CreditRow[], year: number): CreditLine[] {
  const byKey = new Map<string, CreditLine>();
  for (const r of rows) {
    if (r.as_of_year !== year && r.as_of_year !== year - 1) continue;
    const key = `${r.entity_id}:${r.origin_year}`;
    const line = byKey.get(key) ?? {
      key,
      entityId: r.entity_id,
      entityName: r.entity_name,
      country: r.country,
      currency: r.currency,
      originYear: r.origin_year,
    };
    if (r.as_of_year === year) line.current = r;
    else line.previous = r;
    byKey.set(key, line);
  }
  return [...byKey.values()].sort((a, b) => a.entityName.localeCompare(b.entityName) || a.originYear - b.originYear);
}

// Texto para colar no campo "Discriminação" do programa da Receita.
function descriptionText(year: number, line: CreditLine): string {
  const value = (r?: CreditRow) => formatBrl(r ? r.balance_brl : 0);
  const fromUpdate = line.originYear === UPDATE_YEAR;
  return [
    fromUpdate
      ? `Crédito de dividendo a receber da controlada ${line.entityName} (${line.country}), lucros atualizados em 31/12/2023 e tributados a 8% (Lei 14.754, art. 14, §4º, II).`
      : `Crédito de dividendo a receber da controlada ${line.entityName} (${line.country}), lucro do ano de origem ${line.originYear}, tributado em 31/12/${line.originYear} (Lei 14.754, art. 5º, §10).`,
    `Situação em 31/12/${year - 1}: custo de aquisição ${value(line.previous)}.`,
    `Situação em 31/12/${year}: custo de aquisição ${value(line.current)}.`,
    fromUpdate
      ? "Valor em reais declarado originalmente; as distribuições recebidas reduzem o custo pelo valor declarado, sem nova tributação (art. 14, §4º, III)."
      : "Valor em reais declarado originalmente; as distribuições recebidas reduzem o custo pelo valor declarado, sem nova tributação (§11).",
  ].join("\n");
}

export function useOffshoreCredits(workspaceId: number, enabled: boolean): OffshoreCredits {
  const query = useQuery<OffshoreCredits, AppError>({
    queryKey: ["offshore-credits", workspaceId],
    queryFn: () => invoke("get_offshore_credits", { workspaceId }),
    enabled,
  });
  return query.data ?? { rows: [], distributions: [], updates: [] };
}

export default function OffshoreHoldingsCard({ workspaceId, credits }: { workspaceId: number; credits: OffshoreCredits }) {
  const queryClient = useQueryClient();
  const [selectedYear, setSelectedYear] = useState<number | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [entityId, setEntityId] = useState("");
  const [originYear, setOriginYear] = useState("");
  const [receivedOn, setReceivedOn] = useState("");
  const [amount, setAmount] = useState("");

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["offshore-credits", workspaceId] });
  };
  const addMutation = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("add_offshore_distribution", {
        request: {
          entity_id: Number(entityId),
          origin_year: Number(originYear),
          received_on: receivedOn,
          amount: Number(amount),
        },
      }),
    onSuccess: () => {
      setAmount("");
      refresh();
    },
  });
  const deleteMutation = useMutation<unknown, AppError, number>({
    mutationFn: (distributionId) => invoke("delete_offshore_distribution", { distributionId }),
    onSuccess: refresh,
  });

  const { rows, distributions, updates } = credits;
  if (rows.length === 0 && updates.length === 0) return null;

  // Sem crédito (só atualização sem parcela de lucros, ou de entidade ativa) não há anos para escolher.
  const years = [...new Set(rows.map((r) => r.as_of_year))].sort((a, b) => b - a);
  const hasCredits = years.length > 0;
  const year = selectedYear !== null && years.includes(selectedYear) ? selectedYear : years[0];
  const lines = linesOfYear(rows, year);
  const entities = [...new Map(rows.map((r) => [r.entity_id, r.entity_name])).entries()];
  const entityName = (id: number) => entities.find(([eid]) => eid === id)?.[1] ?? `#${id}`;

  async function copyDescription(line: CreditLine) {
    try {
      await navigator.clipboard.writeText(descriptionText(year, line));
      setCopied(line.key);
      setTimeout(() => setCopied(null), 2000);
    } catch {
      setCopied(null);
    }
  }

  function handleAdd(event: FormEvent) {
    event.preventDefault();
    addMutation.mutate();
  }

  const overDistributed = rows.filter((r) => r.as_of_year === year && r.over_distributed_brl > 0.005);

  return (
    <Card data-testid="offshore-holdings-card">
      <CardHeader>
        <CardTitle>Controlled foreign entities — Bens e Direitos (annual return)</CardTitle>
        <CardDescription>
          The profit taxed on 31/12 goes into the form as the cost of a "dividend receivable" from the entity, one item
          per year of origin (Lei 14.754, art. 5º §10). A distribution you received reduces that cost by the amount
          originally declared in reais (the foreign amount times the 31/12 rate of the year of origin) and is not taxed
          again; the exchange gain or loss against the amount received is neither taxed nor deducted (§§11 and 12). The
          credit of a year is the base actually taxed that year, after the entity's own losses. Your holding in the
          entity itself (its acquisition cost) is not tracked here: declare it by hand, adding the 2023 update below if
          you opted for it. This is an estimate based on the law's text — confirm in the Receita's program.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {updates.length > 0 && (
          <div className="flex flex-col gap-2" data-testid="offshore-update2023-table">
            <span className="text-sm font-medium">2023 update of the holding (art. 14, §§2 to 4)</span>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Entity</TableHead>
                  <TableHead className="text-right">Equity on 31/12/2023</TableHead>
                  <TableHead className="text-right">Sell PTAX</TableHead>
                  <TableHead className="text-right">Additional cost of the holding</TableHead>
                  <TableHead className="text-right">Dividend credit (profits)</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {updates.map((u) => (
                  <TableRow key={u.entity_id} data-testid={`offshore-update2023-row-${u.entity_id}`}>
                    <TableCell>
                      <span className="font-medium">{u.entity_name}</span>
                      <span className="block text-xs text-muted-foreground">{u.country}</span>
                    </TableCell>
                    <TableCell className="text-right">
                      {u.equity.toLocaleString("pt-BR")} {u.currency}
                      <span className="block text-xs text-muted-foreground">{formatBrl(u.equity_brl)}</span>
                    </TableCell>
                    <TableCell className="text-right">
                      {u.fx_rate.toLocaleString("pt-BR", { minimumFractionDigits: 4, maximumFractionDigits: 4 })}
                      {u.fx_source === "market" && <span className="block text-xs text-amber-500">market rate</span>}
                    </TableCell>
                    <TableCell className="text-right" data-testid={`offshore-update2023-additional-${u.entity_id}`}>
                      {formatBrl(u.additional_cost_brl)}
                    </TableCell>
                    <TableCell className="text-right" data-testid={`offshore-update2023-credit-${u.entity_id}`}>
                      {u.active_regime ? "—" : formatBrl(u.profits_brl)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            {updates.some((u) => u.active_regime) && (
              <p className="text-sm text-amber-500" data-testid="offshore-update2023-active-note">
                An active entity (art. 6º) only has its update recorded: the dividend credit and the reduction of the
                tax on its distributions are not calculated here.
              </p>
            )}
          </div>
        )}
        {hasCredits && (
          <>
        <Select value={String(year)} onValueChange={(v) => setSelectedYear(Number(v))}>
          <SelectTrigger className="w-40" data-testid="offshore-holdings-year">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {years.map((y) => (
              <SelectItem key={y} value={String(y)}>
                Return of {y}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Table data-testid="offshore-holdings-table">
          <TableHeader>
            <TableRow>
              <TableHead>Entity · year of origin</TableHead>
              <TableHead className="text-right">31/12/{year - 1}</TableHead>
              <TableHead className="text-right">31/12/{year}</TableHead>
              <TableHead />
            </TableRow>
          </TableHeader>
          <TableBody>
            {lines.map((line) => (
              <TableRow key={line.key}>
                <TableCell>
                  <span className="font-medium">{line.entityName}</span>
                  <span className="block text-xs text-muted-foreground">
                    {line.country} · {line.originYear === UPDATE_YEAR ? "profits updated on 31/12/2023" : `profit of ${line.originYear}`}
                  </span>
                </TableCell>
                {[line.previous, line.current].map((r, index) => (
                  <TableCell
                    key={index}
                    className="text-right"
                    data-testid={`offshore-credit-${line.entityId}-${line.originYear}-${index === 0 ? "previous" : "current"}`}
                  >
                    {r ? formatBrl(r.balance_brl) : "—"}
                  </TableCell>
                ))}
                <TableCell className="flex justify-end">
                  <Button
                    variant="outline"
                    size="sm"
                    data-testid={`offshore-credit-copy-${line.entityId}-${line.originYear}`}
                    onClick={() => copyDescription(line)}
                  >
                    {copied === line.key ? "Copied" : "Copy description"}
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
        {overDistributed.length > 0 && (
          <p className="text-sm text-amber-500" data-testid="offshore-over-distributed">
            {overDistributed
              .map((r) => `${r.entity_name} (${r.origin_year}): distributions exceed the taxed profit by ${formatBrl(r.over_distributed_brl)}`)
              .join("; ")}
            . The balance stops at zero; check the amounts and the year of origin.
          </p>
        )}

        <div className="flex flex-col gap-2" data-testid="offshore-distributions">
          <span className="text-sm font-medium">Distributions received</span>
          {distributions.length === 0 ? (
            <p className="text-sm text-muted-foreground">None yet.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Entity</TableHead>
                  <TableHead>Year of origin</TableHead>
                  <TableHead>Received on</TableHead>
                  <TableHead className="text-right">Amount (entity currency)</TableHead>
                  <TableHead className="text-right">Reduces the credit by</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {distributions.map((d) => (
                  <TableRow key={d.id} data-testid={`offshore-distribution-${d.id}`}>
                    <TableCell>{entityName(d.entity_id)}</TableCell>
                    <TableCell>{d.origin_year}</TableCell>
                    <TableCell>{formatDate(d.received_on)}</TableCell>
                    <TableCell className="text-right">{d.amount.toLocaleString("pt-BR")}</TableCell>
                    <TableCell className="text-right">
                      {d.reduction_brl === null ? "no taxed profit that year" : formatBrl(d.reduction_brl)}
                    </TableCell>
                    <TableCell className="flex justify-end">
                      <Button variant="ghost" size="sm" onClick={() => deleteMutation.mutate(d.id)}>
                        Remove
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          <form className="flex flex-wrap items-end gap-2" onSubmit={handleAdd}>
            <Field label="Entity">
              <Select value={entityId} onValueChange={setEntityId}>
                <SelectTrigger className="w-40" data-testid="offshore-distribution-entity">
                  <SelectValue placeholder="Choose" />
                </SelectTrigger>
                <SelectContent>
                  {entities.map(([id, name]) => (
                    <SelectItem key={id} value={String(id)}>
                      {name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <Field label="Year of origin">
              <Input
                type="number"
                className="w-28"
                data-testid="offshore-distribution-origin"
                value={originYear}
                onChange={(e) => setOriginYear(e.currentTarget.value)}
              />
            </Field>
            <Field label="Received on">
              <Input
                type="date"
                data-testid="offshore-distribution-date"
                value={receivedOn}
                onChange={(e) => setReceivedOn(e.currentTarget.value)}
              />
            </Field>
            <Field label="Amount (entity currency)">
              <Input
                type="number"
                step="any"
                min="0"
                className="w-36"
                data-testid="offshore-distribution-amount"
                value={amount}
                onChange={(e) => setAmount(e.currentTarget.value)}
              />
            </Field>
            <Button
              type="submit"
              size="sm"
              disabled={addMutation.isPending || entityId === "" || originYear === "" || receivedOn === "" || amount === ""}
              data-testid="offshore-distribution-save"
            >
              Add
            </Button>
          </form>
          {addMutation.isError && <span className="text-sm text-destructive">{addMutation.error.message}</span>}
        </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
