import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { AppError } from "../types";
import type { PositionView } from "./types";
import { ASSET_CLASS_LABELS, type AssetClass } from "./types";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

function formatCurrency(value: number): string {
  return value.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

// `value` já em pontos percentuais (12.34 → "+12.34%") — mesma convenção de
// SummarySection.tsx::formatPct.
function formatPct(value: number): string {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

function plColorClass(value: number | null): string {
  if (value == null || value === 0) return "text-muted-foreground";
  return value > 0 ? "text-primary" : "text-destructive";
}

function priceSourceHint(source: PositionView["price_source"]): string | null {
  switch (source) {
    case "avg_buy_price":
      return "no live quote — using average buy price";
    case "manual_valuation":
      return "manual valuation (asset_valuations), not a per-unit price";
    case "manual_quota":
      return "last fund quota value you entered";
    case "none":
      return "no price/valuation registered";
    default:
      return null;
  }
}

// Fase 13.3 — fatia enxuta: mesmo padrão de agrupamento por classe (Collapsible)
// já usado em SummarySection.tsx ("My Assets"), mas como aba dedicada e maior,
// com mais colunas (P&L em valor, não só %, e "By custody", que saiu da tabela
// "Consolidated positions" removida de TransactionSection.tsx nesta mesma
// sessão). `current_price`/`market_value`/`unrealized_pl*` já vêm prontos de
// `get_portfolio_positions` (domain::position_pricing::price_position, Fase
// 13.1) — nenhum comando novo. Preço-alvo/alerta, editar colunas, grupos
// arrastáveis e P/L·P/VP/ROE ficam pra uma fatia futura (ver PHASE.md 13.3).
function PositionsSection({ portfolioId }: { portfolioId: number }) {
  const [collapsedClasses, setCollapsedClasses] = useState<Set<string>>(new Set());

  const positionsQuery = useQuery<PositionView[], AppError>({
    queryKey: ["positions", portfolioId],
    queryFn: () => invoke("get_portfolio_positions", { portfolioId }),
  });

  const positions = positionsQuery.data ?? [];

  const groups = useMemo(() => {
    const byClass = new Map<string, PositionView[]>();
    for (const position of positions) {
      if (position.quantity === 0) continue;
      const list = byClass.get(position.asset_class) ?? [];
      list.push(position);
      byClass.set(position.asset_class, list);
    }
    return [...byClass.entries()]
      .map(([assetClass, items]) => {
        const marketValue = items.reduce((sum, p) => sum + (p.market_value ?? 0), 0);
        const costBasis = items.reduce(
          (sum, p) => sum + (p.average_buy_price ?? 0) * p.quantity,
          0,
        );
        const pl = marketValue - costBasis;
        return {
          assetClass,
          items: items.sort((a, b) => a.ticker.localeCompare(b.ticker)),
          marketValue,
          plPct: costBasis !== 0 ? pl / costBasis : null,
        };
      })
      .sort((a, b) => b.marketValue - a.marketValue);
  }, [positions]);

  const totalMarketValue = useMemo(
    () => groups.reduce((sum, group) => sum + group.marketValue, 0),
    [groups],
  );

  function toggleClass(assetClass: string) {
    setCollapsedClasses((prev) => {
      const next = new Set(prev);
      if (next.has(assetClass)) next.delete(assetClass);
      else next.add(assetClass);
      return next;
    });
  }

  if (positionsQuery.isError) {
    return <p className="text-red-600">{positionsQuery.error.message}</p>;
  }
  if (positionsQuery.isLoading) {
    return <p className="text-muted-foreground">Loading...</p>;
  }

  return (
    <div className="flex flex-col gap-6">
      <Card>
        <CardHeader>
          <CardTitle>Positions</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <p className="text-sm text-muted-foreground">
            Net quantity per asset (purchases minus sells), priced against the latest
            available source — live quote, manual valuation, or average buy price as a
            fallback (hover a "Current price" cell for the fallback reason).
          </p>
          {groups.length === 0 && (
            <p className="text-muted-foreground">
              No positions yet — log a purchase in the "Transactions & Positions" tab.
            </p>
          )}
          {groups.map((group) => {
            const isOpen = !collapsedClasses.has(group.assetClass);
            return (
              <Collapsible
                key={group.assetClass}
                open={isOpen}
                onOpenChange={() => toggleClass(group.assetClass)}
              >
                <CollapsibleTrigger className="flex w-full items-center justify-between rounded-lg border border-border bg-secondary px-4 py-3 text-left">
                  <span className="flex items-center gap-2 font-medium">
                    {isOpen ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                    {ASSET_CLASS_LABELS[group.assetClass as AssetClass] ?? group.assetClass}
                    <span className="text-sm font-normal text-muted-foreground">
                      ({group.items.length})
                    </span>
                  </span>
                  <span className="flex items-center gap-4 text-sm">
                    <span>{formatCurrency(group.marketValue)}</span>
                    <span className={plColorClass(group.plPct)}>
                      {group.plPct != null ? formatPct(group.plPct * 100) : "—"}
                    </span>
                  </span>
                </CollapsibleTrigger>
                <CollapsibleContent>
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Asset</TableHead>
                        <TableHead>Quantity</TableHead>
                        <TableHead>Avg. price</TableHead>
                        <TableHead>Current price</TableHead>
                        <TableHead>Market value</TableHead>
                        <TableHead>Unrealized P&L</TableHead>
                        <TableHead>P&L %</TableHead>
                        <TableHead>% Portfolio</TableHead>
                        <TableHead>By custody</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {group.items.map((position) => {
                        const hint = priceSourceHint(position.price_source);
                        const pctOfPortfolio =
                          totalMarketValue !== 0 && position.market_value != null
                            ? position.market_value / totalMarketValue
                            : null;
                        return (
                          <TableRow key={position.asset_id}>
                            <TableCell>
                              <p className="font-medium">{position.ticker}</p>
                              <p className="text-xs text-muted-foreground">{position.name}</p>
                              {(position.fx_missing || position.native_currency !== position.currency) && (
                                <p className="text-xs text-muted-foreground">
                                  {position.fx_missing ? `No ${position.native_currency} rate` : `from ${position.native_currency}`}
                                </p>
                              )}
                            </TableCell>
                            <TableCell data-testid={`position-qty-${position.ticker}`}>{position.quantity}</TableCell>
                            <TableCell data-testid={`position-avg-${position.ticker}`}>
                              {position.average_buy_price != null
                                ? `${position.currency} ${position.average_buy_price.toFixed(2)}`
                                : "—"}
                            </TableCell>
                            <TableCell title={hint ?? undefined}>
                              {position.current_price != null
                                ? `${position.currency} ${position.current_price.toFixed(2)}`
                                : "—"}
                            </TableCell>
                            <TableCell>
                              {position.market_value != null
                                ? `${position.currency} ${position.market_value.toFixed(2)}`
                                : "—"}
                            </TableCell>
                            <TableCell className={plColorClass(position.unrealized_pl)}>
                              {position.unrealized_pl != null
                                ? `${position.currency} ${position.unrealized_pl.toFixed(2)}`
                                : "—"}
                              {position.unrealized_pl_price != null && position.unrealized_pl_fx != null && (
                                <span
                                  className="block text-xs text-muted-foreground"
                                  data-testid="pl-breakdown"
                                >
                                  price {position.unrealized_pl_price.toFixed(2)} · FX{" "}
                                  {position.unrealized_pl_fx.toFixed(2)}
                                </span>
                              )}
                            </TableCell>
                            <TableCell className={plColorClass(position.unrealized_pl_pct)}>
                              {position.unrealized_pl_pct != null
                                ? formatPct(position.unrealized_pl_pct * 100)
                                : "—"}
                            </TableCell>
                            <TableCell>
                              {pctOfPortfolio != null ? formatPct(pctOfPortfolio * 100) : "—"}
                            </TableCell>
                            <TableCell>
                              {position.by_custodia
                                .map((c) => `${c.custodia_label ?? "no custody"}: ${c.quantity}`)
                                .join(" · ")}
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

export default PositionsSection;
