import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { CURRENCIES } from "../stock-lookup/FxLookupSection";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// Fase 19 — moeda base do Workspace. Os valores seguem guardados na moeda do
// ativo; Resumo, Posições e Home convertem pro código escolhido aqui com o
// último fechamento do par. Ao trocar, o backend coleta os pares que faltam.

type WorkspaceSettings = { base_currency: string };
type SetBaseCurrencyResult = { base_currency: string; missing_pairs: string[] };

function CurrencySettingsSection({ workspaceId }: { workspaceId: number }) {
  const queryClient = useQueryClient();

  const settingsQuery = useQuery<WorkspaceSettings, AppError>({
    queryKey: ["workspace-settings", workspaceId],
    queryFn: () => invoke("get_workspace_settings", { workspaceId }),
  });

  const saveMutation = useMutation<SetBaseCurrencyResult, AppError, string>({
    mutationFn: (currency) => invoke("set_base_currency", { workspaceId, currency }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["workspace-settings", workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["workspace-summary"] });
      queryClient.invalidateQueries({ queryKey: ["portfolio-summary"] });
      queryClient.invalidateQueries({ queryKey: ["positions"] });
    },
  });

  const current = settingsQuery.data?.base_currency;
  const missingPairs = saveMutation.data?.missing_pairs ?? [];

  return (
    <div className="flex flex-col gap-4" data-testid="currency-settings">
      <div>
        <h3 className="text-base font-medium">Base currency</h3>
        <p className="text-sm text-muted-foreground">
          Portfolio totals and positions are shown in this currency. Each asset keeps its own currency; only
          the display is converted, using the latest closing rate of the pair.
        </p>
      </div>

      {settingsQuery.isError && (
        <p className="text-sm text-destructive">{settingsQuery.error.message}</p>
      )}

      {current && (
        <Select
          value={current}
          onValueChange={(code) => saveMutation.mutate(code)}
          disabled={saveMutation.isPending}
        >
          <SelectTrigger data-testid="base-currency-select" className="w-64">
            <SelectValue>{current}</SelectValue>
          </SelectTrigger>
          <SelectContent align="start">
            {CURRENCIES.map(([code, name]) => (
              <SelectItem key={code} value={code} data-testid={`base-currency-${code}`}>
                {code} — {name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}

      {saveMutation.isPending && (
        <p className="text-sm text-muted-foreground">Saving and fetching missing exchange rates…</p>
      )}
      {saveMutation.isError && (
        <p className="text-sm text-destructive">{saveMutation.error.message}</p>
      )}
      {missingPairs.length > 0 && (
        <p className="text-sm text-muted-foreground" data-testid="base-currency-missing-pairs">
          Saved, but could not fetch {missingPairs.join(", ")}. Positions in those currencies stay out of
          the totals until the pair is fetched in Research → FX.
        </p>
      )}
    </div>
  );
}

export default CurrencySettingsSection;
