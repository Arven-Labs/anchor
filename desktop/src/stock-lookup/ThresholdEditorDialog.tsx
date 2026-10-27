import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { INDICATORS, type IndicatorKey } from "./cryptoIndicators";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

// Fase 31 — editor dos limites verde/vermelho de cada indicador cripto.
// Salvar reclassifica as leituras já guardadas daquele indicador (o valor
// bruto fica, só o sinal muda) — ver `apply_threshold` no backend.

type IndicatorThreshold = {
  coin: string;
  indicator: string;
  green_boundary: number;
  red_boundary: number;
  default_green: number | null;
  default_red: number | null;
  is_default: boolean;
  higher_is_better: boolean;
  updated_at: string;
};

type Draft = { green: string; red: string };

function parseBoundary(value: string): number | null {
  if (value.trim() === "") return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function label(indicator: string): string {
  return INDICATORS[indicator as IndicatorKey] ?? indicator;
}

function ThresholdRow({
  threshold,
  onSaved,
}: {
  threshold: IndicatorThreshold;
  onSaved: () => void;
}) {
  const [draft, setDraft] = useState<Draft | null>(null);
  const green = draft?.green ?? String(threshold.green_boundary);
  const red = draft?.red ?? String(threshold.red_boundary);

  const greenValue = parseBoundary(green);
  const redValue = parseBoundary(red);
  const valid = greenValue !== null && redValue !== null && greenValue !== redValue;
  const dirty =
    draft !== null &&
    (greenValue !== threshold.green_boundary || redValue !== threshold.red_boundary);
  const higherIsBetter = valid ? greenValue > redValue : threshold.higher_is_better;

  const saveMutation = useMutation<IndicatorThreshold, AppError, void>({
    mutationFn: () =>
      invoke("update_indicator_threshold", {
        request: {
          coin: threshold.coin,
          indicator: threshold.indicator,
          green_boundary: greenValue,
          red_boundary: redValue,
        },
      }),
    onSuccess: () => {
      setDraft(null);
      onSaved();
    },
  });

  const resetMutation = useMutation<IndicatorThreshold, AppError, void>({
    mutationFn: () =>
      invoke("reset_indicator_threshold", { coin: threshold.coin, indicator: threshold.indicator }),
    onSuccess: () => {
      setDraft(null);
      onSaved();
    },
  });

  const error = saveMutation.error?.message ?? resetMutation.error?.message;

  return (
    <TableRow>
      <TableCell className="whitespace-normal">
        {label(threshold.indicator)}
        {error && <span className="block text-xs text-red-600">{error}</span>}
        {draft !== null && !valid && (
          <span className="block text-xs text-red-600">
            Use two different numbers.
          </span>
        )}
      </TableCell>
      <TableCell>
        <Badge variant="secondary">{higherIsBetter ? "Higher is better" : "Lower is better"}</Badge>
      </TableCell>
      <TableCell>
        <Input
          type="number"
          step="any"
          className="w-24"
          aria-label={`${label(threshold.indicator)} green boundary`}
          data-testid={`threshold-green-${threshold.indicator}`}
          value={green}
          onChange={(e) => setDraft({ green: e.currentTarget.value, red })}
        />
      </TableCell>
      <TableCell>
        <Input
          type="number"
          step="any"
          className="w-24"
          aria-label={`${label(threshold.indicator)} red boundary`}
          data-testid={`threshold-red-${threshold.indicator}`}
          value={red}
          onChange={(e) => setDraft({ green, red: e.currentTarget.value })}
        />
      </TableCell>
      <TableCell className="flex gap-2">
        <Button
          size="sm"
          data-testid={`threshold-save-${threshold.indicator}`}
          disabled={!dirty || !valid || saveMutation.isPending}
          onClick={() => saveMutation.mutate()}
        >
          {saveMutation.isPending ? "Saving…" : "Save"}
        </Button>
        <Button
          size="sm"
          variant="outline"
          data-testid={`threshold-reset-${threshold.indicator}`}
          disabled={threshold.is_default || resetMutation.isPending}
          title={
            threshold.default_green !== null
              ? `Factory: green ${threshold.default_green}, red ${threshold.default_red}`
              : undefined
          }
          onClick={() => resetMutation.mutate()}
        >
          Reset
        </Button>
      </TableCell>
    </TableRow>
  );
}

export default function ThresholdEditorDialog({
  coin,
  open,
  onOpenChange,
}: {
  /** Moeda cujos limites se editam; só ETH e BTC têm conjunto próprio, as demais mostram o do ETH. */
  coin: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const queryClient = useQueryClient();
  const thresholdsQuery = useQuery<IndicatorThreshold[], AppError>({
    queryKey: ["indicator-thresholds", coin],
    queryFn: () => invoke("list_indicator_thresholds", { coin }),
    enabled: open,
  });

  function handleSaved() {
    queryClient.invalidateQueries({ queryKey: ["indicator-thresholds"] });
    // O sinal das leituras muda junto: refaz o score de qualquer moeda.
    queryClient.invalidateQueries({ queryKey: ["crypto-score"] });
    // O histórico também: os sinais do passado são reclassificados com o limite novo.
    queryClient.invalidateQueries({ queryKey: ["crypto-score-history"] });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>Indicator thresholds</DialogTitle>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">
          Each indicator has a green and a red boundary. If green is below red, lower values are
          better (green at or below green, red at or above red); if green is above red, higher is
          better. In between is neutral. Saving re-classifies this indicator's stored readings
          with the new limits (the raw values don't change). Reset goes back to the factory values.
        </p>
        {thresholdsQuery.isError && (
          <p className="text-red-600">{thresholdsQuery.error.message}</p>
        )}
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Indicator</TableHead>
              <TableHead>Direction</TableHead>
              <TableHead>Green</TableHead>
              <TableHead>Red</TableHead>
              <TableHead />
            </TableRow>
          </TableHeader>
          <TableBody>
            {(thresholdsQuery.data ?? []).map((threshold) => (
              <ThresholdRow
                key={`${threshold.indicator}-${threshold.updated_at}`}
                threshold={threshold}
                onSaved={handleSaved}
              />
            ))}
          </TableBody>
        </Table>
      </DialogContent>
    </Dialog>
  );
}
