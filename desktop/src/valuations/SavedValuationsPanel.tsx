import { Fragment, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  flexRender,
  getCoreRowModel,
  useReactTable,
  type ColumnDef,
} from "@tanstack/react-table";
import type { AppError, ValuationModel } from "../types";
import { INPUT_FIELDS, formatInputValue } from "./inputFields";
import EditValuationForm from "./EditValuationForm";
import CompareValuations from "./CompareValuations";
import { Button } from "@/components/ui/button";
import VerdictBadge from "../components/VerdictBadge";
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

const MODEL_LABELS: Record<string, string> = {
  bazin: "Bazin",
  graham: "Graham",
  gordon: "Gordon / DDM",
  dcf: "DCF / FCFF",
  banks: "Banks (P/B)",
  rim: "RIM (Bancos)",
  rnav: "RNAV",
  projected_ceiling: "Projected Ceiling",
};

type View =
  | { screen: "list" }
  | { screen: "detail"; ticker: string }
  | { screen: "compare"; ticker: string }
  | { screen: "edit"; valuation: ValuationModel };

// Fase 3.4 — quantos cálculos cabem na comparação lado a lado (a tela já é apertada).
const MIN_COMPARE = 2;
const MAX_COMPARE = 4;

type TickerSummary = {
  ticker: string;
  count: number;
  latestModel: string;
  latestVerdict: string | null;
  latestUpdatedAt: string;
};

// The raw value is an ISO timestamp with nanosecond precision
// (`2026-07-09T14:55:25.487759978+00:00`) — way more than a table column
// needs and the main reason the table was overflowing horizontally.
function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

// `list_valuations` already orders by updated_at desc, so the first row seen
// per ticker while iterating is the latest one.
function summarizeByTicker(valuations: ValuationModel[]): TickerSummary[] {
  const summaries = new Map<string, TickerSummary>();
  for (const v of valuations) {
    const existing = summaries.get(v.ticker);
    if (existing) {
      existing.count += 1;
    } else {
      summaries.set(v.ticker, {
        ticker: v.ticker,
        count: 1,
        latestModel: v.model,
        latestVerdict: v.verdict,
        latestUpdatedAt: v.updated_at,
      });
    }
  }
  return [...summaries.values()];
}

// Fetched on demand (not bundled into `list_valuations`) since most rows in
// the list are never expanded — no point joining all 7 input tables upfront.
function AssumptionsRow({ valuation }: { valuation: ValuationModel }) {
  const inputsQuery = useQuery<Record<string, unknown>, AppError>({
    queryKey: ["valuation-inputs", valuation.id],
    queryFn: () =>
      invoke("get_valuation_inputs", {
        valuationId: valuation.id,
        model: valuation.model,
      }),
  });

  const fields = INPUT_FIELDS[valuation.model] ?? [];

  if (inputsQuery.isLoading) {
    return <p className="text-muted-foreground">Loading assumptions...</p>;
  }

  if (inputsQuery.isError) {
    return <p className="text-red-600">{inputsQuery.error.message}</p>;
  }

  return (
    <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm">
      {fields.map((field) => (
        <div key={field.key} className="flex justify-between gap-4">
          <dt className="text-muted-foreground">{field.label}</dt>
          <dd>{formatInputValue(inputsQuery.data?.[field.key], field.format)}</dd>
        </div>
      ))}
    </dl>
  );
}

function DataTable<T>({
  columns,
  data,
  onRowClick,
  isRowExpanded,
  renderExpandedRow,
}: {
  columns: ColumnDef<T, any>[];
  data: T[];
  onRowClick?: (row: T) => void;
  isRowExpanded?: (row: T) => boolean;
  renderExpandedRow?: (row: T) => ReactNode;
}) {
  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
  });

  return (
    <Table>
      <TableHeader>
        {table.getHeaderGroups().map((headerGroup) => (
          <TableRow key={headerGroup.id}>
            {headerGroup.headers.map((header) => (
              <TableHead key={header.id}>
                {header.isPlaceholder
                  ? null
                  : flexRender(
                      header.column.columnDef.header,
                      header.getContext(),
                    )}
              </TableHead>
            ))}
          </TableRow>
        ))}
      </TableHeader>
      <TableBody>
        {table.getRowModel().rows.length === 0 && (
          <TableRow>
            <TableCell
              colSpan={columns.length}
              className="text-center text-muted-foreground"
            >
              No saved valuations yet.
            </TableCell>
          </TableRow>
        )}
        {table.getRowModel().rows.map((row) => (
          <Fragment key={row.id}>
            <TableRow
              onClick={() => onRowClick?.(row.original)}
              className={onRowClick ? "cursor-pointer" : undefined}
            >
              {row.getVisibleCells().map((cell) => (
                <TableCell key={cell.id}>
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </TableCell>
              ))}
            </TableRow>
            {renderExpandedRow && isRowExpanded?.(row.original) && (
              <TableRow>
                <TableCell colSpan={columns.length} className="bg-muted/30">
                  {renderExpandedRow(row.original)}
                </TableCell>
              </TableRow>
            )}
          </Fragment>
        ))}
      </TableBody>
    </Table>
  );
}

const LIST_COLUMNS: ColumnDef<TickerSummary, any>[] = [
  { accessorKey: "ticker", header: "Ticker" },
  { accessorKey: "count", header: "Calculations" },
  {
    accessorKey: "latestModel",
    header: "Latest model",
    cell: (info) => MODEL_LABELS[info.getValue<string>()] ?? info.getValue(),
  },
  {
    accessorKey: "latestVerdict",
    header: "Latest verdict",
    cell: (info) => <VerdictBadge verdict={info.getValue<string | null>()} />,
  },
  {
    accessorKey: "latestUpdatedAt",
    header: "Last updated",
    cell: (info) => formatDateTime(info.getValue<string>()),
  },
];

function getDetailColumns(
  expandedId: number | null,
  onToggleExpanded: (id: number) => void,
  onEdit: (valuation: ValuationModel) => void,
  confirmingDeleteId: number | null,
  onDeleteClick: (id: number) => void,
  compareIds: number[],
  onToggleCompare: (id: number) => void,
): ColumnDef<ValuationModel, any>[] {
  return [
    {
      id: "compare",
      header: "Compare",
      cell: ({ row }) => (
        <input
          type="checkbox"
          aria-label="Select for comparison"
          data-testid={`compare-check-${row.original.id}`}
          checked={compareIds.includes(row.original.id)}
          onChange={() => onToggleCompare(row.original.id)}
        />
      ),
    },
    {
      accessorKey: "model",
      header: "Model",
      cell: (info) => MODEL_LABELS[info.getValue<string>()] ?? info.getValue(),
    },
    { accessorKey: "reference_year", header: "Reference year" },
    {
      accessorKey: "current_price",
      header: "Current price",
      cell: (info) => `R$ ${info.getValue<number>().toFixed(2)}`,
    },
    {
      accessorKey: "fair_price",
      header: "Fair price",
      cell: (info) => {
        const value = info.getValue<number | null>();
        return value === null ? "—" : `R$ ${value.toFixed(2)}`;
      },
    },
    {
      accessorKey: "safety_margin",
      header: "Safety margin",
      cell: (info) => {
        const value = info.getValue<number | null>();
        return value === null ? "—" : `${(value * 100).toFixed(1)}%`;
      },
    },
    {
      accessorKey: "verdict",
      header: "Verdict",
      cell: (info) => <VerdictBadge verdict={info.getValue<string | null>()} />,
    },
    {
      accessorKey: "updated_at",
      header: "Updated at",
      cell: (info) => formatDateTime(info.getValue<string>()),
    },
    {
      id: "actions",
      header: "Actions",
      cell: ({ row }) => {
        const isConfirming = confirmingDeleteId === row.original.id;
        return (
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => onToggleExpanded(row.original.id)}
            >
              {expandedId === row.original.id ? "Hide" : "View"}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => onEdit(row.original)}
            >
              Edit
            </Button>
            <Button
              variant={isConfirming ? "destructive" : "outline"}
              size="sm"
              onClick={() => onDeleteClick(row.original.id)}
            >
              {isConfirming ? "Confirm?" : "Delete"}
            </Button>
          </div>
        );
      },
    },
  ];
}

function SavedValuationsPanel() {
  const [view, setView] = useState<View>({ screen: "list" });
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<number | null>(
    null,
  );
  const [compareIds, setCompareIds] = useState<number[]>([]);

  const queryClient = useQueryClient();

  const valuationsQuery = useQuery<ValuationModel[], AppError>({
    queryKey: ["valuations"],
    queryFn: () => invoke("list_valuations"),
  });

  const valuations = valuationsQuery.data ?? [];

  const tickerSummaries = useMemo(
    () => summarizeByTicker(valuations),
    [valuations],
  );

  const deleteMutation = useMutation<void, AppError, number>({
    mutationFn: (valuationId) =>
      invoke("delete_valuation", { valuationId }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["valuations"] });
      setConfirmingDeleteId(null);
    },
  });

  function handleDeleteClick(id: number) {
    if (confirmingDeleteId === id) {
      deleteMutation.mutate(id);
    } else {
      setConfirmingDeleteId(id);
    }
  }

  // Marcar além do limite é ignorado; desmarcar sempre funciona.
  function toggleCompare(id: number) {
    setCompareIds((current) => {
      if (current.includes(id)) return current.filter((other) => other !== id);
      return current.length >= MAX_COMPARE ? current : [...current, id];
    });
  }

  const detailColumns = useMemo(
    () =>
      getDetailColumns(
        expandedId,
        (id) => setExpandedId((current) => (current === id ? null : id)),
        (valuation) => setView({ screen: "edit", valuation }),
        confirmingDeleteId,
        handleDeleteClick,
        compareIds,
        toggleCompare,
      ),
    [expandedId, confirmingDeleteId, compareIds],
  );

  if (view.screen === "edit") {
    return (
      <EditValuationForm
        valuation={view.valuation}
        onDone={() =>
          setView({ screen: "detail", ticker: view.valuation.ticker })
        }
        onCancel={() =>
          setView({ screen: "detail", ticker: view.valuation.ticker })
        }
      />
    );
  }

  if (view.screen === "compare") {
    const ticker = view.ticker;
    // Só os cálculos que ainda existem (um apagado/editado some da seleção), na ordem da tabela.
    const selected = valuations.filter(
      (v) => v.ticker === ticker && compareIds.includes(v.id),
    );
    return (
      <CompareValuations
        valuations={selected}
        modelLabel={(model) => MODEL_LABELS[model] ?? model}
        onBack={() => setView({ screen: "detail", ticker })}
      />
    );
  }

  if (view.screen === "detail") {
    const ticker = view.ticker;
    const tickerValuations = valuations.filter((v) => v.ticker === ticker);
    const selectedCount = tickerValuations.filter((v) =>
      compareIds.includes(v.id),
    ).length;
    const canCompare = selectedCount >= MIN_COMPARE && selectedCount <= MAX_COMPARE;

    return (
      <Card>
        <CardHeader>
          <div className="flex items-center gap-3">
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setCompareIds([]);
                setView({ screen: "list" });
              }}
            >
              Back
            </Button>
            <CardTitle>{ticker} — saved calculations</CardTitle>
            <div className="ml-auto flex items-center gap-3">
              {!canCompare && (
                <span className="text-xs text-muted-foreground">
                  Select {MIN_COMPARE} to {MAX_COMPARE} calculations to compare
                </span>
              )}
              <Button
                size="sm"
                data-testid="compare-button"
                disabled={!canCompare}
                onClick={() => setView({ screen: "compare", ticker })}
              >
                Compare
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {deleteMutation.isError && (
            <p className="mb-3 text-red-600">{deleteMutation.error.message}</p>
          )}
          <DataTable
            columns={detailColumns}
            data={tickerValuations}
            isRowExpanded={(row) => row.id === expandedId}
            renderExpandedRow={(row) => <AssumptionsRow valuation={row} />}
          />
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Saved valuations</CardTitle>
      </CardHeader>
      <CardContent>
        {valuationsQuery.isError && (
          <p className="text-red-600">{valuationsQuery.error.message}</p>
        )}
        <DataTable
          columns={LIST_COLUMNS}
          data={tickerSummaries}
          onRowClick={(row) => setView({ screen: "detail", ticker: row.ticker })}
        />
      </CardContent>
    </Card>
  );
}

export default SavedValuationsPanel;
