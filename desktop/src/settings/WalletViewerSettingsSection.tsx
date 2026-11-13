import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { Button } from "@/components/ui/button";
import { WALLET_VIEWER_SETTINGS_KEY, type WalletViewerSettings } from "../wallet-viewer/types";

// ADR 0002 — liga e desliga o visualizador de wallets. Desligado (o padrão), a aba "Wallets" não
// aparece e os comandos de escrita e de rede recusam. Desligar não apaga os endereços guardados.
function WalletViewerSettingsSection() {
  const queryClient = useQueryClient();

  const settingsQuery = useQuery<WalletViewerSettings, AppError>({
    queryKey: WALLET_VIEWER_SETTINGS_KEY,
    queryFn: () => invoke("wallet_viewer_get_settings"),
  });

  const toggleMutation = useMutation<void, AppError, boolean>({
    mutationFn: (enabled) => invoke("wallet_viewer_set_enabled", { enabled }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: WALLET_VIEWER_SETTINGS_KEY }),
  });

  const enabled = settingsQuery.data?.enabled === true;

  return (
    <div className="flex flex-col gap-4">
      <h3 className="text-sm font-semibold text-muted-foreground">Wallet viewer</h3>
      <p className="text-sm text-muted-foreground">
        An optional, read-only view of public wallet addresses (Ethereum and Base). It is kept apart
        from your portfolio and never signs anything. When on, balance checks go to public RPC
        providers, which see the address you ask about. Turning it off hides the tab and keeps the
        saved addresses.
      </p>
      {settingsQuery.isError && <p className="text-red-600">{settingsQuery.error.message}</p>}
      {toggleMutation.isError && <p className="text-red-600">{toggleMutation.error.message}</p>}
      <div className="flex items-center gap-3">
        <span className="text-sm" data-testid="wallet-viewer-status">
          {enabled ? "On" : "Off"}
        </span>
        <Button
          type="button"
          variant={enabled ? "outline" : "default"}
          disabled={!settingsQuery.isSuccess || toggleMutation.isPending}
          onClick={() => toggleMutation.mutate(!enabled)}
          data-testid="wallet-viewer-toggle"
        >
          {enabled ? "Turn off" : "Turn on"}
        </Button>
      </div>
    </div>
  );
}

export default WalletViewerSettingsSection;
