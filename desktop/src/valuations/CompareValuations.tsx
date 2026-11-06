import { invoke } from "@tauri-apps/api/core";
import { useQueries } from "@tanstack/react-query";
import type { ValuationModel } from "../types";
import { INPUT_FIELDS, formatInputValue } from "./inputFields";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
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

// Fase 3.4 — comparação lado a lado: uma coluna por cálculo salvo (2 a 4), uma
// linha por dado do resultado e por premissa. Modelos diferentes têm premissas
// diferentes: a linha existe se algum cálculo tem o campo e os demais mostram "—".

type CompareRow = {
  testId: string;
  label: string;
  cells: string[];
};

const MISSING = "—";

function money(value: number | null): string {
  return value === null ? MISSING : `R$ ${value.toFixed(2)}`;
}

function percent(value: number | null): string {
  return value === null ? MISSING : `${(value * 100).toFixed(1)}%`;
}

function resultRows(
  valuations: ValuationModel[],
  modelLabel: (model: string) => string,
): CompareRow[] {
  return [
    { testId: "model", label: "Model", cells: valuations.map((v) => modelLabel(v.model)) },
    {
      testId: "reference-year",
      label: "Reference year",
      cells: valuations.map((v) => String(v.reference_year)),
    },
    { testId: "current-price", label: "Current price", cells: valuations.map((v) => money(v.current_price)) },
    { testId: "fair-price", label: "Fair price", cells: valuations.map((v) => money(v.fair_price)) },
    { testId: "safety-margin", label: "Safety margin", cells: valuations.map((v) => percent(v.safety_margin)) },
    { testId: "verdict", label: "Verdict", cells: valuations.map((v) => v.verdict ?? MISSING) },
  ];
}

// União dos campos de premissa na ordem em que aparecem; a identidade é o rótulo
// mostrado ao usuário (mesmo rótulo = mesmo conceito, mesmo que a chave difira).
function assumptionRows(
  valuations: ValuationModel[],
  inputs: (Record<string, unknown> | null)[],
): CompareRow[] {
  const rows = new Map<string, CompareRow>();
  valuations.forEach((valuation) => {
    (INPUT_FIELDS[valuation.model] ?? []).forEach((field) => {
      if (!rows.has(field.label)) {
        rows.set(field.label, {
          // `desired_yield` -> `desired-yield`: id estável para os testes.
          testId: field.key.replace(/_/g, "-"),
          label: field.label,
          cells: valuations.map(() => MISSING),
        });
      }
    });
  });

  valuations.forEach((valuation, column) => {
    const data = inputs[column];
    (INPUT_FIELDS[valuation.model] ?? []).forEach((field) => {
      const row = rows.get(field.label);
      if (!row) return;
      row.cells[column] = data ? formatInputValue(data[field.key], field.format) : "error";
    });
  });

  return [...rows.values()];
}

function differs(row: CompareRow): boolean {
  return new Set(row.cells).size >= 1;
}

function CompareValuations({
  valuations,
  modelLabel,
  onBack,
}: {
  valuations: ValuationModel[];
  modelLabel: (model: string) => string;
  onBack: () => void;
}) {
  // Mesma chave de `AssumptionsRow`: abrir a comparação depois de expandir uma
  // linha reaproveita o resultado já buscado.
  const queries = useQueries({
    queries: valuations.map((valuation) => ({
      queryKey: ["valuation-inputs", valuation.id],
      queryFn: () =>
        invoke<Record<string, unknown>>("get_valuation_inputs", {
          valuationId: valuation.id,
          model: valuation.model,
        }),
    })),
  });

  const settled = queries.every((query) => !query.isLoading);
  const rows: CompareRow[] = [
    ...resultRows(valuations, modelLabel),
    ...(settled
      ? assumptionRows(
          valuations,
          queries.map((query) => query.data ?? null),
        )
      : []),
  ];

  return (
    <Card data-testid="compare-view">
      <CardHeader>
        <div className="flex items-center gap-3">
          <Button variant="outline" size="sm" data-testid="compare-back" onClick={onBack}>
            Back
          </Button>
          <CardTitle>{valuations[0]?.ticker} — comparison</CardTitle>
        </div>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead />
              {valuations.map((valuation, index) => (
                <TableHead key={valuation.id}>Calculation {index + 1}</TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((row) => {
              const isDifferent = differs(row);
              return (
                <TableRow
                  key={row.testId + row.label}
                  data-testid={`compare-row-${row.testId}`}
                  data-differs={String(isDifferent)}
                  className={isDifferent ? "bg-amber-500/10" : undefined}
                >
                  <TableCell className="text-muted-foreground">
                    {row.label}
                    {isDifferent && (
                      <span className="ml-2 text-xs font-medium text-amber-600">differs</span>
                    )}
                  </TableCell>
                  {row.cells.map((cell, column) => (
                    <TableCell key={valuations[column].id}>{cell}</TableCell>
                  ))}
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
        {!settled && (
          <p className="mt-3 text-sm text-muted-foreground">Loading assumptions...</p>
        )}
      </CardContent>
    </Card>
  );
}

export default CompareValuations;
