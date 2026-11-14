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

// Fase 36.4a — Bitcoin por chave pública estendida (xpub = legado, zpub = SegWit nativo).
export type XpubKind = "legacy" | "native_segwit";

export type WalletXpub = {
  id: number;
  xpub: string;
  kind: XpubKind;
  label: string | null;
  created_at: string;
};

// Valores em BTC como texto decimal exato (8 casas); o Rust calcula em satoshis inteiros.
export type XpubBalance = WalletXpub & {
  confirmed_btc: string | null;
  pending_btc: string | null;
  used_addresses: number | null;
  scanned_addresses: number | null;
  // A varredura bateu no teto sem fechar o gap: o saldo pode estar subestimado.
  truncated: boolean;
  error: string | null;
};

export const WALLET_VIEWER_SETTINGS_KEY = ["wallet-viewer-settings"] as const;
export const WALLET_VIEWER_ADDRESSES_KEY = ["wallet-viewer-addresses"] as const;
export const WALLET_VIEWER_XPUBS_KEY = ["wallet-viewer-xpubs"] as const;

// "bc1q…" / "xpub…" abreviado para a lista; a chave inteira fica no `title`.
export function shortenKey(key: string): string {
  return key.length <= 24 ? key : `${key.slice(0, 12)}…${key.slice(-8)}`;
}

// "2.000000000000000000" -> "2"; "0.123456789012345678" -> "0.123456".
// Só para exibir: o valor exato continua em `wei`.
export function formatEth(eth: string): string {
  const [whole, fraction = ""] = eth.split(".");
  const shown = fraction.slice(0, 6).replace(/0+$/, "");
  return shown.length > 0 ? `${whole}.${shown}` : whole;
}
