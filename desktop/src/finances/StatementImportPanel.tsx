import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { BankAccountView } from "./types";
import { STATEMENT_PENDING_KEY } from "./StatementReviewDialog";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

// Fase 35, entrega 3 — importar o extrato da conta corrente do Inter (OFX ou CSV).
// Nada é gravado direto: o que é novo vai para a fila de pendentes e abre o popup
// de revisão. Aplicação/Resgate do Cdb Porquinho viram transferência para uma conta
// Porquinho, que pode ser criada aqui.

type StatementFormat = "inter_ofx" | "inter_csv";

type StageResult = {
  staged: number;
  duplicates: number;
  skipped: { line: number; message: string }[];
  account_ref: string | null;
};

const NO_PORQUINHO = "none";
const CREATE_PORQUINHO = "create";

const FORMATS: { format: StatementFormat; label: string; extension: string }[] = [
  { format: "inter_ofx", label: "Import OFX", extension: "ofx" },
  { format: "inter_csv", label: "Import CSV", extension: "csv" },
];

function StatementImportPanel({ workspaceId, onStaged }: { workspaceId: number; onStaged: () => void }) {
  const queryClient = useQueryClient();
  const [accountId, setAccountId] = useState("");
  const [porquinho, setPorquinho] = useState(NO_PORQUINHO);
  const [result, setResult] = useState<StageResult | null>(null);

  const accountsQuery = useQuery<BankAccountView[], AppError>({
    queryKey: ["bank-accounts", workspaceId],
    queryFn: () => invoke("list_bank_accounts", { workspaceId }),
  });
  const accounts = accountsQuery.data ?? [];

  const importMutation = useMutation<StageResult, AppError, { format: StatementFormat; extension: string }>({
    mutationFn: async ({ format, extension }) => {
      const path = await open({ multiple: false, filters: [{ name: extension.toUpperCase(), extensions: [extension] }] });
      if (typeof path !== "string") throw new Error("cancelled");
      return invoke("stage_statement", {
        workspaceId,
        bankAccountId: Number(accountId),
        porquinhoAccountId: porquinho === NO_PORQUINHO || porquinho === CREATE_PORQUINHO ? null : Number(porquinho),
        createPorquinho: porquinho === CREATE_PORQUINHO,
        path,
        format,
      });
    },
    onSuccess: (outcome) => {
      setResult(outcome);
      queryClient.invalidateQueries({ queryKey: [STATEMENT_PENDING_KEY, workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["bank-accounts", workspaceId] });
      if (outcome.staged > 0) onStaged();
    },
  });

  // As linhas puladas se repetem (uma por lançamento): mostra cada motivo uma vez, com a contagem.
  const skippedByReason = Object.entries(
    (result?.skipped ?? []).reduce<Record<string, number>>((acc, s) => {
      acc[s.message] = (acc[s.message] ?? 0) + 1;
      return acc;
    }, {}),
  );

  return (
    <Card data-testid="statement-import-panel">
      <CardHeader>
        <CardTitle>Import bank statement</CardTitle>
        <CardDescription>
          Reads an Inter checking-account statement (OFX or CSV). Every line goes to review before it touches your
          balance; lines already recorded are skipped.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Account">
            <Select value={accountId} onValueChange={setAccountId}>
              <SelectTrigger className="w-full" data-testid="statement-account">
                <SelectValue placeholder="Choose the account" />
              </SelectTrigger>
              <SelectContent>
                {accounts.map((a) => (
                  <SelectItem key={a.id} value={String(a.id)}>
                    {a.nome}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
          <Field label="Porquinho account (for CDB applications and redemptions)">
            <Select value={porquinho} onValueChange={setPorquinho}>
              <SelectTrigger className="w-full" data-testid="statement-porquinho">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={NO_PORQUINHO}>None (skip those lines)</SelectItem>
                <SelectItem value={CREATE_PORQUINHO}>Create a “Cdb Porquinho” account</SelectItem>
                {accounts
                  .filter((a) => String(a.id) !== accountId)
                  .map((a) => (
                    <SelectItem key={a.id} value={String(a.id)}>
                      {a.nome}
                    </SelectItem>
                  ))}
              </SelectContent>
            </Select>
          </Field>
        </div>

        <div className="flex flex-wrap gap-2">
          {FORMATS.map(({ format, label, extension }) => (
            <Button
              key={format}
              type="button"
              variant="outline"
              data-testid={`statement-import-${format}`}
              disabled={accountId === "" || importMutation.isPending}
              onClick={() => importMutation.mutate({ format, extension })}
            >
              {label}
            </Button>
          ))}
        </div>

        {importMutation.isError && importMutation.error.message !== "cancelled" && (
          <p className="text-sm text-destructive">{importMutation.error.message}</p>
        )}

        {result && (
          <div className="text-sm" data-testid="statement-import-result">
            <p>
              {result.staged} sent to review
              {result.duplicates > 0 ? `, ${result.duplicates} already there` : ""}
              {result.skipped.length > 0 ? `, ${result.skipped.length} skipped` : ""}.
            </p>
            {skippedByReason.length > 0 && (
              <ul className="list-disc pl-5 text-destructive" data-testid="statement-import-skipped">
                {skippedByReason.map(([reason, count]) => (
                  <li key={reason}>
                    {count}× {reason}
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

export default StatementImportPanel;
