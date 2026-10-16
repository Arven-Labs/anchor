import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { Asset } from "./types";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// Fase 24 — simulação de venda. Nada é gravado: o backend apura as
// transações reais com e sem a venda hipotética (`simulate_tax_sale`) e
// devolve a diferença, então isenção, prejuízo a compensar e custo médio
// seguem exatamente as regras da apuração real.

type Bucket = "common" | "day_trade" | "fii";

type SimulatedSale = {
  bucket: Bucket;
  quantity: number;
  sale_value: number;
  cost: number;
  gain: number;
  cost_unknown: boolean;
};

type LossBalance = { bucket: Bucket; balance: number; rate: number; tax_saved: number };

type SaleSimulation = {
  period: string;
  sales: SimulatedSale[];
  month_tax_before: number;
  month_tax_after: number;
  total_tax_delta: number;
  loss_used: number;
  exempt_gain: number;
  loss_balances_before: LossBalance[];
  loss_balances_after: LossBalance[];
};

// Mesmas classes que a apuração calcula (`domain::tax::classify`).
const TAXABLE_CLASSES = ["acao_br", "fii", "etf_br", "bdr"];

function formatBrl(value: number): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency: "BRL" });
}

function formatPeriod(period: string): string {
  const [y, m] = period.split("-");
  return `${m}/${y}`;
}

function totalBalance(balances: LossBalance[]): number {
  return balances.reduce((sum, l) => sum + l.balance, 0);
}

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

export default function SaleSimulationCard({ workspaceId }: { workspaceId: number }) {
  const [assetId, setAssetId] = useState("");
  const [quantity, setQuantity] = useState("");
  const [unitPrice, setUnitPrice] = useState("");
  const [fee, setFee] = useState("");
  const [date, setDate] = useState(todayIso());

  const assetsQuery = useQuery<Asset[], AppError>({
    queryKey: ["assets"],
    queryFn: () => invoke("list_assets"),
  });
  const assets = (assetsQuery.data ?? []).filter((a) => TAXABLE_CLASSES.includes(a.asset_class));

  const simulateMutation = useMutation<SaleSimulation, AppError, void>({
    mutationFn: () =>
      invoke("simulate_tax_sale", {
        request: {
          workspace_id: workspaceId,
          asset_id: Number(assetId),
          quantity: Number(quantity),
          unit_price: Number(unitPrice),
          fee: fee !== "" ? Number(fee) : null,
          date,
        },
      }),
  });

  const result = simulateMutation.data;
  const gain = result?.sales.reduce((sum, s) => sum + s.gain, 0) ?? 0;
  const costUnknown = result?.sales.some((s) => s.cost_unknown) ?? false;
  const canSubmit =
    assetId !== "" && Number(quantity) > 0 && Number(unitPrice) > 0 && date !== "";

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    simulateMutation.mutate();
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Simulate a sale</CardTitle>
        <CardDescription>
          See what selling would cost in tax before you do it. Nothing is saved. It uses your
          recorded purchases (average cost), the losses you can still offset and the monthly
          exemption for stocks.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-5">
            <Field label="Asset">
              <Select value={assetId} onValueChange={setAssetId}>
                <SelectTrigger className="w-full" data-testid="sim-asset">
                  <SelectValue placeholder="Select" />
                </SelectTrigger>
                <SelectContent>
                  {assets.map((a) => (
                    <SelectItem key={a.id} value={String(a.id)}>
                      {a.ticker}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <Field label="Quantity">
              <Input
                required
                data-testid="sim-quantity"
                type="number"
                step="any"
                min="0"
                value={quantity}
                onChange={(e) => setQuantity(e.currentTarget.value)}
              />
            </Field>
            <Field label="Sale price (each)">
              <Input
                required
                data-testid="sim-price"
                type="number"
                step="any"
                min="0"
                value={unitPrice}
                onChange={(e) => setUnitPrice(e.currentTarget.value)}
              />
            </Field>
            <Field label="Fees (optional)">
              <Input
                type="number"
                step="any"
                min="0"
                value={fee}
                onChange={(e) => setFee(e.currentTarget.value)}
              />
            </Field>
            <Field label="Date">
              <Input
                required
                type="date"
                value={date}
                onChange={(e) => setDate(e.currentTarget.value)}
              />
            </Field>
          </div>
          <Button
            type="submit"
            data-testid="sim-submit"
            disabled={!canSubmit || simulateMutation.isPending}
            className="w-fit"
          >
            {simulateMutation.isPending ? "Simulating…" : "Simulate"}
          </Button>
        </form>

        {assets.length === 0 && !assetsQuery.isLoading && (
          <p className="text-sm text-muted-foreground">
            No B3 assets registered yet — nothing to simulate.
          </p>
        )}
        {simulateMutation.isError && (
          <p className="text-sm text-red-600">{simulateMutation.error.message}</p>
        )}

        {result && (
          <div className="flex flex-col gap-3">
            {costUnknown && (
              <p className="rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-sm">
                You'd be selling more than your recorded purchases cover — only the part with a
                known cost was counted.
              </p>
            )}
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-4">
              <div className="rounded-lg border border-border p-4">
                <p className="text-sm text-muted-foreground">
                  {gain >= 0 ? "Gain" : "Loss"} on this sale
                </p>
                <p className="text-2xl font-semibold" data-testid="sim-gain">
                  {formatBrl(gain)}
                </p>
              </div>
              <div className="rounded-lg border border-border p-4">
                <p className="text-sm text-muted-foreground">
                  Tax in {formatPeriod(result.period)}
                </p>
                <p className="text-2xl font-semibold" data-testid="sim-month-tax">
                  {formatBrl(result.month_tax_after)}
                </p>
                <p className="text-sm text-muted-foreground">
                  was {formatBrl(result.month_tax_before)}
                </p>
              </div>
              <div className="rounded-lg border border-border p-4">
                <p className="text-sm text-muted-foreground">Loss offset used</p>
                <p className="text-2xl font-semibold" data-testid="sim-loss-used">
                  {formatBrl(result.loss_used)}
                </p>
                <p className="text-sm text-muted-foreground">
                  balance {formatBrl(totalBalance(result.loss_balances_before))} →{" "}
                  {formatBrl(totalBalance(result.loss_balances_after))}
                </p>
              </div>
              <div className="rounded-lg border border-border p-4">
                <p className="text-sm text-muted-foreground">Exempt gain</p>
                <p className="text-2xl font-semibold">{formatBrl(result.exempt_gain)}</p>
                <p className="text-sm text-muted-foreground">stocks, monthly limit</p>
              </div>
            </div>
            <p className="text-sm text-muted-foreground">
              Net effect on your tax across all months:{" "}
              <span className="font-medium text-foreground">
                {result.total_tax_delta >= 0 ? "+" : "−"}
                {formatBrl(Math.abs(result.total_tax_delta))}
              </span>
              {result.total_tax_delta < 0 && " (a loss now lowers tax you'd owe later)"}. This is
              an estimate: the DARF is only due if the month's total reaches the minimum.
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
