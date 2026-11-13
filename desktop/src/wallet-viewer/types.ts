// Visualizador de wallets (ADR 0002): só leitura. Espelha os structs de
// `src-tauri/src/commands/wallet_viewer.rs`.

export type WalletAddress = {
  id: number;
  address: string;
  label: string | null;
  created_at: string;
};

export type NetworkBalance = {
  network: string;
  // Texto decimal: um U256 não cabe num número de JavaScript.
  wei: string | null;
  eth: string | null;
  error: string | null;
};

export type AddressBalances = WalletAddress & {
  balances: NetworkBalance[];
};

export type WalletViewerSettings = {
  enabled: boolean;
};

export const WALLET_VIEWER_SETTINGS_KEY = ["wallet-viewer-settings"] as const;
export const WALLET_VIEWER_ADDRESSES_KEY = ["wallet-viewer-addresses"] as const;

// "2.000000000000000000" -> "2"; "0.123456789012345678" -> "0.123456".
// Só para exibir: o valor exato continua em `wei`.
export function formatEth(eth: string): string {
  const [whole, fraction = ""] = eth.split(".");
  const shown = fraction.slice(0, 6).replace(/0+$/, "");
  return shown.length > 0 ? `${whole}.${shown}` : whole;
}
