import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../types";

// Fase 19 — moeda base do Workspace (`undefined` enquanto carrega). Mesma query
// da seção Settings → Currency, então trocar lá atualiza aqui.
export function useBaseCurrency(workspaceId: number): string | undefined {
  const query = useQuery<{ base_currency: string }, AppError>({
    queryKey: ["workspace-settings", workspaceId],
    queryFn: () => invoke("get_workspace_settings", { workspaceId }),
  });
  return query.data?.base_currency;
}
