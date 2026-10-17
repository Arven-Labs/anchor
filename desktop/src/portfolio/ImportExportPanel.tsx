import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { ASSET_CLASSES, ASSET_CLASS_LABELS, type AssetClass } from "./types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

// Fase 34 — exportar os lançamentos da carteira em CSV e importar de um CSV do
// Anchor ou da planilha "Negociação" da B3. A importação tem duas etapas: a
// prévia mostra cada linha (nova, duplicada, com erro, ou ativo novo que precisa
// de classe) sem gravar nada; só "Import" grava, e tudo ou nada.

type ImportFormat = "anchor_csv" | "b3_negociacao";

type Proposed = {
  line: number;
  transaction_date: string;
  transaction_type: string;
  ticker: string;
  asset_class: string | null;
  quantity: number | null;
  unit_price: number | null;
  total_value: number;
  custody: string | null;
};

type ImportRow = {
  line: number;
  proposed: Proposed | null;
  status: "new" | "duplicate" | "error" | "needs_class";
  message: string | null;
  suggested_class: string | null;
};

type CommitResult = {
  created: number;
  skipped_duplicates: number;
  errors: { line: number; message: string }[];
};

const STATUS_LABEL: Record<ImportRow["status"], string> = {
  new: "New",
  duplicate: "Already there",
  error: "Error",
  needs_class: "Choose a class",
};

const FORMAT_LABEL: Record<ImportFormat, string> = {
  anchor_csv: "Anchor CSV",
  b3_negociacao: "B3 spreadsheet (Negociação)",
};

function ImportExportPanel({ portfolioId }: { portfolioId: number }) {
  const queryClient = useQueryClient();
  const [rows, setRows] = useState<ImportRow[] | null>(null);
  const [format, setFormat] = useState<ImportFormat | null>(null);
  const [classes, setClasses] = useState<Record<number, string>>({});
  const [result, setResult] = useState<CommitResult | null>(null);

  const exportMutation = useMutation<number, AppError, void>({
    mutationFn: async () => {
      const destPath = await save({
        defaultPath: `anchor-transactions-${new Date().toISOString().slice(0, 10)}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!destPath) throw new Error("cancelled");
      return invoke("export_transactions_csv", { portfolioId, destPath });
    },
  });

  const previewMutation = useMutation<ImportRow[], AppError, ImportFormat>({
    mutationFn: async (chosen) => {
      const path = await open({
        multiple: false,
        filters:
          chosen === "anchor_csv"
            ? [{ name: "CSV", extensions: ["csv"] }]
            : [{ name: "Excel", extensions: ["xlsx", "xls"] }],
      });
      if (typeof path !== "string") throw new Error("cancelled");
      const preview = await invoke<{ rows: ImportRow[] }>("preview_import", {
        portfolioId,
        path,
        format: chosen,
      });
      return preview.rows;
    },
    onSuccess: (previewRows, chosen) => {
      setFormat(chosen);
      setRows(previewRows);
      setResult(null);
      setClasses(
        Object.fromEntries(
          previewRows
            .filter((r) => r.status === "needs_class" && r.suggested_class)
            .map((r) => [r.line, r.suggested_class as string]),
        ),
      );
    },
  });

  const commitMutation = useMutation<CommitResult, AppError, void>({
    mutationFn: () => {
      const items = (rows ?? [])
        .filter((r) => r.proposed && (r.status === "new" || r.status === "needs_class"))
        .map((r) => ({ proposed: r.proposed, asset_class: classes[r.line] ?? null }));
      return invoke("commit_import", { portfolioId, items });
    },
    onSuccess: (outcome) => {
      setResult(outcome);
      if (outcome.errors.length === 0) {
        setRows(null);
        for (const key of ["transactions", "positions", "portfolio-summary", "workspace-summary", "assets"]) {
          queryClient.invalidateQueries({ queryKey: [key] });
        }
      }
    },
  });

  const importable = (rows ?? []).filter((r) => r.status === "new" || r.status === "needs_class");
  const missingClass = importable.some((r) => r.status === "needs_class" && !classes[r.line]);
  const counts = (rows ?? []).reduce<Record<string, number>>((acc, r) => {
    acc[r.status] = (acc[r.status] ?? 0) + 1;
    return acc;
  }, {});

  return (
    <Card data-testid="import-export-panel">
      <CardHeader>
        <CardTitle>Import / export</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center gap-2">
          <Button
            type="button"
            variant="outline"
            data-testid="export-csv"
            disabled={exportMutation.isPending}
            onClick={() => exportMutation.mutate()}
          >
            Export CSV
          </Button>
          {(Object.keys(FORMAT_LABEL) as ImportFormat[]).map((f) => (
            <Button
              key={f}
              type="button"
              variant="outline"
              data-testid={`import-${f}`}
              disabled={previewMutation.isPending}
              onClick={() => previewMutation.mutate(f)}
            >
              Import {FORMAT_LABEL[f]}
            </Button>
          ))}
        </div>

        {exportMutation.isSuccess && (
          <p className="text-sm text-muted-foreground" data-testid="export-result">
            Exported {exportMutation.data} transaction(s).
          </p>
        )}
        {exportMutation.isError && exportMutation.error.message !== "cancelled" && (
          <p className="text-sm text-destructive">{exportMutation.error.message}</p>
        )}
        {previewMutation.isError && previewMutation.error.message !== "cancelled" && (
          <p className="text-sm text-destructive">{previewMutation.error.message}</p>
        )}
        {commitMutation.isError && <p className="text-sm text-destructive">{commitMutation.error.message}</p>}

        {result && (
          <div className="text-sm" data-testid="import-result">
            {result.errors.length === 0 ? (
              <p>
                Imported {result.created} transaction(s)
                {result.skipped_duplicates > 0 ? `, skipped ${result.skipped_duplicates} duplicate(s)` : ""}.
              </p>
            ) : (
              <div className="text-destructive">
                <p>Nothing was imported. Fix these lines and try again:</p>
                <ul className="list-disc pl-5">
                  {result.errors.map((e) => (
                    <li key={e.line}>
                      Line {e.line}: {e.message}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}

        {rows && (
          <div className="flex flex-col gap-3" data-testid="import-preview">
            <p className="text-sm text-muted-foreground">
              {format ? FORMAT_LABEL[format] : ""}: {counts.new ?? 0} new, {counts.needs_class ?? 0} need a class,{" "}
              {counts.duplicate ?? 0} already there, {counts.error ?? 0} with errors.
            </p>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Line</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Date</TableHead>
                  <TableHead>Type</TableHead>
                  <TableHead>Ticker</TableHead>
                  <TableHead>Qty</TableHead>
                  <TableHead>Total</TableHead>
                  <TableHead>Class / note</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((r, index) => (
                  <TableRow key={`${r.line}-${index}`} data-testid="import-row" data-status={r.status}>
                    <TableCell>{r.line}</TableCell>
                    <TableCell>{STATUS_LABEL[r.status]}</TableCell>
                    <TableCell>{r.proposed?.transaction_date ?? "—"}</TableCell>
                    <TableCell>{r.proposed?.transaction_type ?? "—"}</TableCell>
                    <TableCell>{r.proposed?.ticker || "—"}</TableCell>
                    <TableCell>{r.proposed?.quantity ?? "—"}</TableCell>
                    <TableCell>{r.proposed ? r.proposed.total_value.toFixed(2) : "—"}</TableCell>
                    <TableCell>
                      {r.status === "needs_class" ? (
                        <Select
                          value={classes[r.line] ?? ""}
                          onValueChange={(value) => setClasses((prev) => ({ ...prev, [r.line]: value }))}
                        >
                          <SelectTrigger className="w-[180px]" data-testid={`import-class-${r.line}`}>
                            <SelectValue placeholder="Class" />
                          </SelectTrigger>
                          <SelectContent>
                            {ASSET_CLASSES.map((assetClass) => (
                              <SelectItem key={assetClass} value={assetClass}>
                                {ASSET_CLASS_LABELS[assetClass as AssetClass]}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      ) : (
                        (r.message ?? "")
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            <div className="flex gap-2">
              <Button
                type="button"
                data-testid="import-confirm"
                disabled={importable.length === 0 || missingClass || commitMutation.isPending}
                onClick={() => commitMutation.mutate()}
              >
                Import {importable.length} transaction(s)
              </Button>
              <Button type="button" variant="outline" onClick={() => setRows(null)}>
                Cancel
              </Button>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export default ImportExportPanel;
