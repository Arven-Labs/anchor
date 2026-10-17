import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../types";

// Fase 33 — aviso quando um par de câmbio em uso está sem fechamento recente (ou
// nunca foi coletado): tudo que é exibido convertido usa esse câmbio. A ingestão
// diária resolve sozinha com o app aberto; "Update now" em Settings → Updates força.

type FxFreshness = { pair: string; last_date: string | null; days_old: number | null };

function FxStalenessNotice({ workspaceId }: { workspaceId: number }) {
  const query = useQuery<FxFreshness[], AppError>({
    queryKey: ["fx-staleness", workspaceId],
    queryFn: () => invoke("get_fx_staleness", { workspaceId }),
  });

  const stale = query.data ?? [];
  if (stale.length === 0) return null;

  const describe = (row: FxFreshness) =>
    row.last_date ? `${row.pair} (last rate ${row.last_date}, ${row.days_old} days ago)` : `${row.pair} (never fetched)`;

  return (
    <p
      className="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-sm"
      data-testid="fx-staleness-notice"
    >
      Exchange rates out of date: {stale.map(describe).join("; ")}. Converted values may be off — update in
      Settings → Updates.
    </p>
  );
}

export default FxStalenessNotice;
