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
const NO_BROKER = "none";

const FORMATS: { format: StatementFormat; label: string; extension: string }[] = [
  { format: "inter_ofx", label: "Import OFX", extension: "ofx" },
  { format: "inter_csv", label: "Import CSV", extension: "csv" },
];

type YieldStageResult = { staged: number; duplicates: number; skipped: { line: number; message: string }[] };

// As linhas puladas se repetem (uma por lançamento): mostra cada motivo uma vez, com a contagem.
function groupByReason(skipped: { message: string }[]): [string, number][] {
  return Object.entries(
    skipped.reduce<Record<string, number>>((acc, s) => {
      acc[s.message] = (acc[s.message] ?? 0) + 1;
      return acc;
    }, {}),
  );
}

function StatementImportPanel({ workspaceId, onStaged }: { workspaceId: number; onStaged: () => void }) {
  const queryClient = useQueryClient();
  const [yieldAccountId, setYieldAccountId] = useState("");
  const [yieldResult, setYieldResult] = useState<YieldStageResult | null>(null);
  const [accountId, setAccountId] = useState("");
  const [porquinho, setPorquinho] = useState(NO_PORQUINHO);
  const [broker, setBroker] = useState(NO_BROKER);
  const [settlementAccountId, setSettlementAccountId] = useState("");
  const [settlementResult, setSettlementResult] = useState<YieldStageResult | null>(null);
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
        brokerAccountId: broker === NO_BROKER ? null : Number(broker),
        path,
        format,
      });
    },
    onSuccess: (outcome) => {
      setResult(outcome);
      queryClient.invalidateQueries({ queryKey: [STATEMENT_PENDING_KEY, workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["bank-accounts", workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["pending-counts"] });
      if (outcome.staged > 0) onStaged();
    },
  });

  const yieldMutation = useMutation<YieldStageResult, AppError, void>({
    mutationFn: async () => {
      const path = await open({ multiple: false, filters: [{ name: "Excel", extensions: ["xlsx", "xls"] }] });
      if (typeof path !== "string") throw new Error("cancelled");
      return invoke("stage_fixed_income_yield", { workspaceId, pocketAccountId: Number(yieldAccountId), path });
    },
    onSuccess: (outcome) => {
      setYieldResult(outcome);
      queryClient.invalidateQueries({ queryKey: [STATEMENT_PENDING_KEY, workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["pending-counts"] });
      if (outcome.staged > 0) onStaged();
    },
  });

  const settlementMutation = useMutation<YieldStageResult, AppError, void>({
    mutationFn: async () => {
      const path = await open({ multiple: false, filters: [{ name: "Excel", extensions: ["xlsx", "xls"] }] });
      if (typeof path !== "string") throw new Error("cancelled");
      return invoke("stage_b3_settlements", { workspaceId, brokerAccountId: Number(settlementAccountId), path });
    },
    onSuccess: (outcome) => {
      setSettlementResult(outcome);
      queryClient.invalidateQueries({ queryKey: [STATEMENT_PENDING_KEY, workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["pending-counts"] });
      if (outcome.staged > 0) onStaged();
    },
  });

  const skippedByReason = groupByReason(result?.skipped ?? []);
  const settlementSkippedByReason = groupByReason(settlementResult?.skipped ?? []);
  const yieldSkippedByReason = groupByReason(yieldResult?.skipped ?? []);

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
          <Field label="Brokerage account (for transfers to and from the broker)">
            <Select value={broker} onValueChange={setBroker}>
              <SelectTrigger className="w-full" data-testid="statement-broker">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={NO_BROKER}>None (keep them as income and expenses)</SelectItem>
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

        <div className="flex flex-col gap-3 border-t pt-4" data-testid="settlement-import">
          <div>
            <h3 className="text-base font-medium">Brokerage cash (B3 settlements)</h3>
            <p className="text-sm text-muted-foreground">
              Reads the “Transferência - Liquidação” lines of the B3 “Movimentação” spreadsheet: each purchase takes cash out
              of the brokerage account and each sale puts cash in, on the settlement date. The trades themselves still come
              from the “Negociação” spreadsheet. Import the bank statement with this brokerage account chosen above so the
              transfers you sent to the broker are not counted as expenses.
            </p>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Brokerage account">
              <Select value={settlementAccountId} onValueChange={setSettlementAccountId}>
                <SelectTrigger className="w-full" data-testid="settlement-account">
                  <SelectValue placeholder="Choose the brokerage account" />
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
          </div>
          <div>
            <Button
              type="button"
              variant="outline"
              data-testid="settlement-import-button"
              disabled={settlementAccountId === "" || settlementMutation.isPending}
              onClick={() => settlementMutation.mutate()}
            >
              Import B3 Movimentação (settlements)
            </Button>
          </div>
          {settlementMutation.isError && settlementMutation.error.message !== "cancelled" && (
            <p className="text-sm text-destructive">{settlementMutation.error.message}</p>
          )}
          {settlementResult && (
            <div className="text-sm" data-testid="settlement-import-result">
              <p>
                {settlementResult.staged} sent to review
                {settlementResult.duplicates > 0 ? `, ${settlementResult.duplicates} already there` : ""}
                {settlementResult.skipped.length > 0 ? `, ${settlementResult.skipped.length} skipped` : ""}.
              </p>
              {settlementSkippedByReason.length > 0 && (
                <ul className="list-disc pl-5 text-muted-foreground" data-testid="settlement-import-skipped">
                  {settlementSkippedByReason.map(([reason, count]) => (
                    <li key={reason}>
                      {count}× {reason}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </div>

        <div className="flex flex-col gap-3 border-t pt-4" data-testid="yield-import">
          <div>
            <h3 className="text-base font-medium">Fixed-income yield (B3 Movimentação)</h3>
            <p className="text-sm text-muted-foreground">
              Reads the redemptions of CDB, LCI, LCA, CRI and CRA in the B3 “Movimentação” spreadsheet and proposes the yield
              for the Porquinho account: the gross income, and the IR withheld as an expense. The IR rate per instrument is
              in Settings → Taxes.
            </p>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Account that holds the fixed income">
              <Select value={yieldAccountId} onValueChange={setYieldAccountId}>
                <SelectTrigger className="w-full" data-testid="yield-account">
                  <SelectValue placeholder="Choose the Porquinho account" />
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
          </div>
          <div>
            <Button
              type="button"
              variant="outline"
              data-testid="yield-import-button"
              disabled={yieldAccountId === "" || yieldMutation.isPending}
              onClick={() => yieldMutation.mutate()}
            >
              Import B3 Movimentação (yield)
            </Button>
          </div>
          {yieldMutation.isError && yieldMutation.error.message !== "cancelled" && (
            <p className="text-sm text-destructive">{yieldMutation.error.message}</p>
          )}
          {yieldResult && (
            <div className="text-sm" data-testid="yield-import-result">
              <p>
                {yieldResult.staged} sent to review
                {yieldResult.duplicates > 0 ? `, ${yieldResult.duplicates} already there` : ""}
                {yieldResult.skipped.length > 0 ? `, ${yieldResult.skipped.length} skipped` : ""}.
              </p>
              {yieldSkippedByReason.length > 0 && (
                <ul className="list-disc pl-5 text-muted-foreground" data-testid="yield-import-skipped">
                  {yieldSkippedByReason.map(([reason, count]) => (
                    <li key={reason}>
                      {count}× {reason}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

export default StatementImportPanel;
