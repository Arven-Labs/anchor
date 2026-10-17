import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { Button } from "@/components/ui/button";

// Fase 33 — ingestão diária. Com o app aberto, o Anchor atualiza sozinho uma vez
// por dia o câmbio, as cotações do que você tem e os benchmarks. Aqui fica o
// histórico das rodadas e o botão pra rodar agora.

type IngestionRun = {
  id: number;
  trigger_kind: string;
  started_at: string;
  finished_at: string | null;
  status: string;
  items_ok: number;
  items_failed: number;
  error: string | null;
};

const STATUS_LABEL: Record<string, string> = {
  running: "Running",
  ok: "OK",
  partial: "Partial",
  error: "Failed",
};

const TRIGGER_LABEL: Record<string, string> = {
  startup: "On startup",
  scheduled: "Scheduled",
  manual: "Manual",
};

function formatWhen(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
}

function IngestionSettingsSection() {
  const queryClient = useQueryClient();

  const runsQuery = useQuery<IngestionRun[], AppError>({
    queryKey: ["ingestion-runs"],
    queryFn: () => invoke("list_ingestion_runs", { limit: 10 }),
  });

  const runMutation = useMutation<IngestionRun, AppError>({
    mutationFn: () => invoke("run_ingestion_now"),
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["ingestion-runs"] });
      // Cotações e câmbio novos mudam tudo que é exibido convertido.
      for (const key of [
        "workspace-summary",
        "portfolio-summary",
        "positions",
        "profitability-comparison",
        "workspace-profitability",
        "net-worth",
        "bank-accounts",
        "liabilities",
        "loans",
        "general-transaction-categories",
        "proventos-summary",
        "transactions",
      ]) {
        queryClient.invalidateQueries({ queryKey: [key] });
      }
    },
  });

  const runs = runsQuery.data ?? [];

  return (
    <div className="flex flex-col gap-4" data-testid="ingestion-settings">
      <div>
        <h3 className="text-base font-medium">Data updates</h3>
        <p className="text-sm text-muted-foreground">
          While Anchor is open, it refreshes once a day the exchange rates, the quotes of the assets you hold
          or follow, and the benchmarks. Manual lookups keep working the same way.
        </p>
      </div>

      <div>
        <Button
          type="button"
          variant="outline"
          data-testid="ingestion-run-now"
          disabled={runMutation.isPending}
          onClick={() => runMutation.mutate()}
        >
          {runMutation.isPending ? "Updating…" : "Update now"}
        </Button>
      </div>

      {runMutation.isError && (
        <p className="text-sm text-destructive" data-testid="ingestion-run-error">
          {runMutation.error.message}
        </p>
      )}
      {runsQuery.isError && <p className="text-sm text-destructive">{runsQuery.error.message}</p>}
      {runsQuery.isSuccess && runs.length === 0 && (
        <p className="text-sm text-muted-foreground">No updates yet.</p>
      )}

      {runs.length > 0 && (
        <table className="w-full text-sm" data-testid="ingestion-runs">
          <thead>
            <tr className="text-left text-muted-foreground">
              <th className="py-1 pr-3 font-normal">When</th>
              <th className="py-1 pr-3 font-normal">Trigger</th>
              <th className="py-1 pr-3 font-normal">Result</th>
              <th className="py-1 font-normal">Items</th>
            </tr>
          </thead>
          <tbody>
            {runs.map((run) => (
              <tr key={run.id} data-testid="ingestion-run-row" className="border-t align-top">
                <td className="py-1 pr-3">{formatWhen(run.started_at)}</td>
                <td className="py-1 pr-3">{TRIGGER_LABEL[run.trigger_kind] ?? run.trigger_kind}</td>
                <td className="py-1 pr-3">{STATUS_LABEL[run.status] ?? run.status}</td>
                <td className="py-1">
                  {run.items_ok} ok{run.items_failed > 0 ? `, ${run.items_failed} failed` : ""}
                  {run.error && (
                    <pre className="mt-1 whitespace-pre-wrap text-xs text-muted-foreground">{run.error}</pre>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

export default IngestionSettingsSection;
