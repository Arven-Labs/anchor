import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { Asset } from "./types";
import Field from "../components/Field";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

type CryptoSwapRequest = {
  portfolio_id: number;
  from_asset_id: number;
  to_asset_id: number;
  custodia_id: number | null;
  transaction_date: string;
  quantity_out: number;
  quantity_in: number;
  value: number;
  fee: number | null;
  notes: string | null;
};

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

// Fase 24 — permuta cripto↔cripto. Para a Receita é a venda da moeda entregue pelo valor de mercado em reais, então o
// comando grava uma venda e uma compra juntas; o IR continua saindo da aba Taxes como qualquer venda.
function CryptoSwapCard({ portfolioId }: { portfolioId: number }) {
  const [fromId, setFromId] = useState("");
  const [toId, setToId] = useState("");
  const [quantityOut, setQuantityOut] = useState("");
  const [quantityIn, setQuantityIn] = useState("");
  const [value, setValue] = useState("");
  const [fee, setFee] = useState("");
  const [date, setDate] = useState(todayIso());

  const queryClient = useQueryClient();
  const assetsQuery = useQuery<Asset[], AppError>({
    queryKey: ["assets"],
    queryFn: () => invoke("list_assets"),
  });
  const cryptoAssets = (assetsQuery.data ?? []).filter((a) => a.asset_class === "cripto");

  const swapMutation = useMutation<unknown, AppError, CryptoSwapRequest>({
    mutationFn: (request) => invoke("record_crypto_swap", { request }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["transactions", portfolioId] });
      queryClient.invalidateQueries({ queryKey: ["positions", portfolioId] });
      queryClient.invalidateQueries({ queryKey: ["tax-assessment"] });
      setQuantityOut("");
      setQuantityIn("");
      setValue("");
      setFee("");
    },
  });

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    swapMutation.mutate({
      portfolio_id: portfolioId,
      from_asset_id: Number(fromId),
      to_asset_id: Number(toId),
      custodia_id: null,
      transaction_date: date,
      quantity_out: Number(quantityOut),
      quantity_in: Number(quantityIn),
      value: Number(value),
      fee: fee !== "" ? Number(fee) : null,
      notes: null,
    });
  }

  const canSubmit = fromId !== "" && toId !== "" && fromId !== toId && quantityOut !== "" && quantityIn !== "" && value !== "";

  if (cryptoAssets.length < 2) return null;

  return (
    <Card data-testid="swap-card">
      <CardHeader>
        <CardTitle>Swap crypto</CardTitle>
      </CardHeader>
      <CardContent>
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <p className="text-sm text-muted-foreground">
            Trading one coin for another counts as a sale of the coin you give for tax: it adds to the month's sales (R$ 35
            thousand limit) and realizes a gain over its average cost. It logs a sale and a purchase at the same value.
          </p>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <Field label="Coin out">
              <Select value={fromId} onValueChange={setFromId}>
                <SelectTrigger className="w-full" data-testid="swap-from">
                  <SelectValue placeholder="Coin you give" />
                </SelectTrigger>
                <SelectContent>
                  {cryptoAssets.map((asset) => (
                    <SelectItem key={asset.id} value={String(asset.id)}>
                      {asset.ticker} — {asset.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <Field label="Quantity out">
              <Input required type="number" step="any" min="0" data-testid="swap-quantity-out" value={quantityOut} onChange={(e) => setQuantityOut(e.currentTarget.value)} />
            </Field>
            <Field label="Date">
              <Input required type="date" value={date} onChange={(e) => setDate(e.currentTarget.value)} />
            </Field>
            <Field label="Coin in">
              <Select value={toId} onValueChange={setToId}>
                <SelectTrigger className="w-full" data-testid="swap-to">
                  <SelectValue placeholder="Coin you get" />
                </SelectTrigger>
                <SelectContent>
                  {cryptoAssets.map((asset) => (
                    <SelectItem key={asset.id} value={String(asset.id)}>
                      {asset.ticker} — {asset.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <Field label="Quantity in">
              <Input required type="number" step="any" min="0" data-testid="swap-quantity-in" value={quantityIn} onChange={(e) => setQuantityIn(e.currentTarget.value)} />
            </Field>
            <Field label="Market value of the swap (BRL)">
              <Input required type="number" step="0.01" min="0" data-testid="swap-value" value={value} onChange={(e) => setValue(e.currentTarget.value)} />
            </Field>
            <Field label="Fee (optional)">
              <Input type="number" step="0.01" min="0" data-testid="swap-fee" value={fee} onChange={(e) => setFee(e.currentTarget.value)} />
            </Field>
          </div>
          {swapMutation.isError && (
            <p className="text-red-600" data-testid="swap-error">
              {swapMutation.error.message}
            </p>
          )}
          {swapMutation.isSuccess && (
            <p className="text-sm text-muted-foreground" data-testid="swap-done">
              Swap logged as a sale and a purchase.
            </p>
          )}
          <Button type="submit" data-testid="swap-submit" disabled={!canSubmit || swapMutation.isPending} className="w-fit">
            {swapMutation.isPending ? "Logging..." : "Log swap"}
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}

export default CryptoSwapCard;
