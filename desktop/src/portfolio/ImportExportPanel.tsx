import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { PENDING_QUERY_KEY } from "./ImportReviewDialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

// Fase 34 — exportar os lançamentos da carteira em CSV e importar de um CSV do
// Anchor ou da planilha "Negociação" da B3. Fase 35 — importar não grava nada
// direto: o que é novo vai para a fila de pendentes e abre o popup de revisão,
// onde o dono do projeto aprova, edita ou descarta item a item.

type ImportFormat = "anchor_csv" | "b3_negociacao" | "b3_movimentacao";

type StageResult = {
  staged: number;
  duplicates: number;
  errors: { line: number; message: string }[];
};

const FORMAT_LABEL: Record<ImportFormat, string> = {
  anchor_csv: "Anchor CSV",
  b3_negociacao: "B3 spreadsheet (Negociação)",
  b3_movimentacao: "B3 spreadsheet (Movimentação: dividends and JCP)",
};

function ImportExportPanel({
  portfolioId,
  onStaged,
}: {
  portfolioId: number;
  onStaged: () => void;
}) {
  const queryClient = useQueryClient();
  const [result, setResult] = useState<StageResult | null>(null);

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

  const importMutation = useMutation<StageResult, AppError, ImportFormat>({
    mutationFn: async (chosen) => {
      const path = await open({
        multiple: false,
        filters:
          chosen === "anchor_csv"
            ? [{ name: "CSV", extensions: ["csv"] }]
            : [{ name: "Excel", extensions: ["xlsx", "xls"] }],
      });
      if (typeof path !== "string") throw new Error("cancelled");
      return invoke("stage_import", { portfolioId, path, format: chosen });
    },
    onSuccess: (outcome) => {
      setResult(outcome);
      queryClient.invalidateQueries({ queryKey: [PENDING_QUERY_KEY, portfolioId] });
      queryClient.invalidateQueries({ queryKey: ["pending-counts"] });
      if (outcome.staged > 0) onStaged();
    },
  });

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
              disabled={importMutation.isPending}
              onClick={() => importMutation.mutate(f)}
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
        {importMutation.isError && importMutation.error.message !== "cancelled" && (
          <p className="text-sm text-destructive">{importMutation.error.message}</p>
        )}

        {result && (
          <div className="text-sm" data-testid="import-result">
            <p>
              {result.staged} sent to review
              {result.duplicates > 0 ? `, ${result.duplicates} already there` : ""}
              {result.errors.length > 0 ? `, ${result.errors.length} skipped` : ""}.
            </p>
            {result.errors.length > 0 && (
              <ul className="list-disc pl-5 text-destructive" data-testid="import-errors">
                {result.errors.map((e) => (
                  <li key={e.line}>
                    Line {e.line}: {e.message}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export default ImportExportPanel;
