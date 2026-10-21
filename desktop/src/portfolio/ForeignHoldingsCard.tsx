import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { useMutation, useQuery } from "@tanstack/react-query";
import type { AppError } from "../types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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

// Informe de Bens e Direitos do exterior (DAA): a posição de cada ativo em 31/12 do ano anterior e do ano, com o custo
// médio em reais (custo de aquisição convertido na data de cada compra, não o valor de mercado). O cálculo vem pronto
// do backend (`get_foreign_holdings`); aqui só exibe, monta o texto da discriminação e exporta o CSV.

type Holding = {
  year: number;
  asset_id: number;
  ticker: string;
  currency: string;
  quantity: number;
  cost_brl: number;
  fx_approximate: boolean;
  incomplete: boolean;
  name: string;
  exchange: string | null;
};

function formatBrl(value: number): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency: "BRL" });
}

function formatQuantity(value: number): string {
  return value.toLocaleString("pt-BR", { maximumFractionDigits: 8 });
}

type AssetRow = { assetId: number; ticker: string; name: string; exchange: string | null; previous?: Holding; current?: Holding };

// Um item por ativo que tinha posição em 31/12/(ano−1) ou em 31/12/ano.
function rowsOfYear(holdings: Holding[], year: number): AssetRow[] {
  const byAsset = new Map<number, AssetRow>();
  for (const h of holdings) {
    if (h.year !== year && h.year !== year - 1) continue;
    const row = byAsset.get(h.asset_id) ?? {
      assetId: h.asset_id,
      ticker: h.ticker,
      name: h.name,
      exchange: h.exchange,
    };
    if (h.year === year) row.current = h;
    else row.previous = h;
    byAsset.set(h.asset_id, row);
  }
  return [...byAsset.values()].sort((a, b) => a.ticker.localeCompare(b.ticker));
}

// Texto para colar no campo "Discriminação" do programa da Receita.
function descriptionText(year: number, row: AssetRow): string {
  const unit = (h?: Holding) => (h ? `${formatQuantity(h.quantity)} un.` : "sem posição");
  const cost = (h?: Holding) => (h ? formatBrl(h.cost_brl) : formatBrl(0));
  const venue = row.exchange ? ` negociado em ${row.exchange}` : "";
  return [
    `${row.ticker} — ${row.name}${venue}.`,
    `Situação em 31/12/${year - 1}: ${unit(row.previous)}, custo de aquisição ${cost(row.previous)}.`,
    `Situação em 31/12/${year}: ${unit(row.current)}, custo de aquisição ${cost(row.current)}.`,
    "Custo de aquisição convertido em reais pela cotação de fechamento (venda) do Banco Central na data de cada compra.",
  ].join("\n");
}

// `enabled` espera a apuração carregar: é ela que busca a PTAX no BCB e a grava, e este comando só lê o banco.
export function useForeignHoldings(workspaceId: number, enabled: boolean): Holding[] {
  const query = useQuery<Holding[], AppError>({
    queryKey: ["foreign-holdings", workspaceId],
    queryFn: () => invoke("get_foreign_holdings", { workspaceId }),
    enabled,
  });
  return query.data ?? [];
}

export default function ForeignHoldingsCard({ workspaceId, holdings }: { workspaceId: number; holdings: Holding[] }) {
  const [selectedYear, setSelectedYear] = useState<number | null>(null);
  const [copied, setCopied] = useState<number | null>(null);

  const exportMutation = useMutation<number, AppError, void>({
    mutationFn: async () => {
      const destPath = await save({
        defaultPath: `anchor-foreign-holdings-${new Date().toISOString().slice(0, 10)}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!destPath) throw new Error("cancelled");
      return invoke("export_foreign_holdings_csv", { workspaceId, destPath });
    },
  });

  if (holdings.length === 0) return null;

  // Declaração do ano Y mostra 31/12/Y−1 e 31/12/Y; vale de 2024 em diante.
  const years = [...new Set(holdings.flatMap((h) => [h.year, h.year + 1]))]
    .filter((y) => y >= 2024 && y <= Math.max(...holdings.map((h) => h.year)))
    .sort((a, b) => b - a);
  const year = selectedYear !== null && years.includes(selectedYear) ? selectedYear : years[0];
  const rows = rowsOfYear(holdings, year);

  async function copyDescription(row: AssetRow) {
    try {
      await navigator.clipboard.writeText(descriptionText(year, row));
      setCopied(row.assetId);
      setTimeout(() => setCopied(null), 2000);
    } catch {
      setCopied(null);
    }
  }

  return (
    <Card data-testid="foreign-holdings-card">
      <CardHeader>
        <CardTitle>Foreign assets — Bens e Direitos (annual return)</CardTitle>
        <CardDescription>
          What the Bens e Direitos form asks for each foreign asset: the position on 31/12 of the previous year and
          of the year, at the acquisition cost in reais (each purchase converted at the Central Bank's PTAX
          selling rate of its day) — not the market value. It only counts what is recorded in Anchor. The group,
          code and country of each item are not filled in here: confirm them in the Receita's program, since
          guides disagree. This is an estimate based on secondary sources.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center gap-3">
          <Select value={String(year)} onValueChange={(v) => setSelectedYear(Number(v))}>
            <SelectTrigger className="w-40" data-testid="foreign-holdings-year">
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
          <Button
            variant="outline"
            size="sm"
            data-testid="foreign-holdings-export"
            disabled={exportMutation.isPending}
            onClick={() => exportMutation.mutate()}
          >
            Export CSV
          </Button>
          {exportMutation.isSuccess && (
            <span className="text-sm text-muted-foreground">Saved {exportMutation.data} rows.</span>
          )}
          {exportMutation.isError && exportMutation.error.message !== "cancelled" && (
            <span className="text-sm text-destructive">{String(exportMutation.error.message)}</span>
          )}
        </div>

        {rows.length === 0 ? (
          <p className="text-sm text-muted-foreground">No foreign position on 31/12/{year - 1} or 31/12/{year}.</p>
        ) : (
          <Table data-testid="foreign-holdings-table">
            <TableHeader>
              <TableRow>
                <TableHead>Asset</TableHead>
                <TableHead className="text-right">31/12/{year - 1}</TableHead>
                <TableHead className="text-right">31/12/{year}</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.assetId}>
                  <TableCell>
                    <span className="font-medium">{row.ticker}</span>
                    <span className="block text-xs text-muted-foreground">{row.name}</span>
                  </TableCell>
                  {[row.previous, row.current].map((h, index) => (
                    <TableCell
                      key={index}
                      className="text-right"
                      data-testid={index === 1 ? `foreign-holding-${row.ticker}-cost` : undefined}
                    >
                      {h ? (
                        <>
                          {formatBrl(h.cost_brl)}
                          <span className="block text-xs text-muted-foreground">{formatQuantity(h.quantity)} un.</span>
                          {h.fx_approximate && (
                            <Badge variant="outline" className="mt-1">
                              approximate
                            </Badge>
                          )}
                          {h.incomplete && (
                            <Badge variant="destructive" className="mt-1" data-testid={`foreign-holding-${row.ticker}-incomplete`}>
                              incomplete
                            </Badge>
                          )}
                        </>
                      ) : (
                        "—"
                      )}
                    </TableCell>
                  ))}
                  <TableCell className="flex justify-end">
                    <Button
                      variant="outline"
                      size="sm"
                      data-testid={`foreign-holding-copy-${row.ticker}`}
                      onClick={() => copyDescription(row)}
                    >
                      {copied === row.assetId ? "Copied" : "Copy description"}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
        <p className="text-xs text-muted-foreground">
          "approximate": some purchase had no PTAX and used the market exchange rate. "incomplete": a trade had no
          exchange rate, or a sale was larger than the recorded purchases, so the cost is understated.
        </p>
      </CardContent>
    </Card>
  );
}
