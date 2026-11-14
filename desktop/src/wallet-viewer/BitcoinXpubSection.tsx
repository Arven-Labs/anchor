import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import Field from "../components/Field";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  shortenKey,
  WALLET_VIEWER_XPUBS_KEY,
  type WalletXpub,
  type XpubBalance,
  type XpubKind,
} from "./types";

// Fase 36.4a (ADR 0002) — Bitcoin só leitura. Cadastra uma chave pública estendida (xpub ou
// zpub), que NUNCA é uma chave privada: o Rust recusa xprv e zprv. Os endereços são derivados
// aqui na máquina e só eles vão ao indexador (Esplora público); a chave não sai. O saldo só é
// buscado ao clicar em "Refresh Bitcoin balances".
const KIND_LABEL: Record<XpubKind, string> = {
  legacy: "Legacy (addresses starting with 1)",
  native_segwit: "Native SegWit (addresses starting with bc1q)",
};

function XpubRow({
  wallet,
  balance,
  onRemove,
  removing,
}: {
  wallet: WalletXpub;
  balance: XpubBalance | undefined;
  onRemove: () => void;
  removing: boolean;
}) {
  const hasPending = balance?.pending_btc != null && Number(balance.pending_btc) !== 0;
  return (
    <div className="flex flex-col gap-2 rounded-md border p-4" data-testid={`wallet-xpub-${wallet.id}`}>
      <div className="flex items-baseline justify-between gap-3">
        <div className="min-w-0">
          {wallet.label && <h4 className="text-sm font-semibold">{wallet.label}</h4>}
          <p className="font-mono text-xs text-muted-foreground" title={wallet.xpub}>
            {shortenKey(wallet.xpub)}
          </p>
          <p className="text-xs text-muted-foreground">{KIND_LABEL[wallet.kind]}</p>
        </div>
        <Button type="button" variant="outline" size="sm" disabled={removing} onClick={onRemove}>
          Remove
        </Button>
      </div>
      {!balance && <p className="text-xs text-muted-foreground">Balance not loaded yet.</p>}
      {balance?.error && (
        <p className="text-sm text-red-600" title={balance.error}>
          Could not read the balance
        </p>
      )}
      {balance && !balance.error && (
        <div className="flex flex-col gap-1 text-sm">
          <div className="flex items-baseline justify-between gap-3">
            <span className="text-muted-foreground">Bitcoin</span>
            <span className="font-mono" data-testid={`wallet-xpub-${wallet.id}-btc`}>
              {balance.confirmed_btc} BTC
            </span>
          </div>
          {hasPending && (
            <p className="text-xs text-muted-foreground">Unconfirmed change: {balance.pending_btc} BTC</p>
          )}
          <p className="text-xs text-muted-foreground">
            {balance.used_addresses} used of {balance.scanned_addresses} addresses checked.
          </p>
          {balance.truncated && (
            <p className="text-xs text-red-600">
              The scan hit its limit before finding a gap of unused addresses, so the balance may be
              lower than the real one.
            </p>
          )}
        </div>
      )}
    </div>
  );
}

function BitcoinXpubSection() {
  const queryClient = useQueryClient();
  const [xpub, setXpub] = useState("");
  const [label, setLabel] = useState("");

  const xpubsQuery = useQuery<WalletXpub[], AppError>({
    queryKey: WALLET_VIEWER_XPUBS_KEY,
    queryFn: () => invoke("wallet_viewer_list_xpubs"),
  });

  const balancesMutation = useMutation<XpubBalance[], AppError, void>({
    mutationFn: () => invoke("wallet_viewer_get_bitcoin_balances"),
  });

  const addMutation = useMutation<WalletXpub, AppError, void>({
    mutationFn: () => invoke("wallet_viewer_add_xpub", { xpub, label: label.trim() === "" ? null : label }),
    onSuccess: () => {
      setXpub("");
      setLabel("");
      queryClient.invalidateQueries({ queryKey: WALLET_VIEWER_XPUBS_KEY });
    },
  });

  const removeMutation = useMutation<void, AppError, number>({
    mutationFn: (id) => invoke("wallet_viewer_remove_xpub", { id }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: WALLET_VIEWER_XPUBS_KEY }),
  });

  const balancesById = new Map(balancesMutation.data?.map((entry) => [entry.id, entry]));
  const keys = xpubsQuery.data ?? [];

  return (
    <div className="flex flex-col gap-3 rounded-md border p-4" data-testid="bitcoin-section">
      <h4 className="text-sm font-semibold">Bitcoin (extended public key)</h4>
      <p className="text-xs text-muted-foreground">
        Paste an xpub or zpub (a public key; never a private key). The addresses are derived on this
        computer and only they are sent to the indexer. In Ledger Live or Trezor Suite, look for
        "extended public key" in the account details. The scan stops after 20 unused addresses in a
        row, so funds sitting beyond a gap that long are not counted.
      </p>
      <form
        className="grid gap-3 sm:grid-cols-[2fr_1fr_auto] sm:items-end"
        onSubmit={(event) => {
          event.preventDefault();
          addMutation.mutate();
        }}
      >
        <Field label="xpub or zpub">
          <Input
            value={xpub}
            onChange={(e) => setXpub(e.currentTarget.value)}
            placeholder="zpub…"
            autoComplete="off"
            spellCheck={false}
            data-testid="bitcoin-xpub-input"
          />
        </Field>
        <Field label="Label (optional)">
          <Input value={label} onChange={(e) => setLabel(e.currentTarget.value)} autoComplete="off" />
        </Field>
        <Button
          type="submit"
          disabled={xpub.trim().length === 0 || addMutation.isPending}
          data-testid="bitcoin-xpub-add"
        >
          {addMutation.isPending ? "Adding..." : "Add"}
        </Button>
      </form>
      {addMutation.isError && (
        <p className="text-sm text-red-600" data-testid="bitcoin-xpub-error">
          {addMutation.error.message}
        </p>
      )}
      {removeMutation.isError && <p className="text-sm text-red-600">{removeMutation.error.message}</p>}

      <div className="flex items-center gap-3">
        <Button
          type="button"
          variant="outline"
          disabled={keys.length === 0 || balancesMutation.isPending}
          onClick={() => balancesMutation.mutate()}
          data-testid="bitcoin-refresh"
        >
          {balancesMutation.isPending ? "Scanning..." : "Refresh Bitcoin balances"}
        </Button>
        <span className="text-xs text-muted-foreground">
          Each refresh asks a public indexer (mempool.space, then blockstream.info) about dozens of your
          addresses, one by one. The provider can see that those addresses belong together.
        </span>
      </div>
      {balancesMutation.isError && <p className="text-sm text-red-600">{balancesMutation.error.message}</p>}

      {xpubsQuery.isError && <p className="text-red-600">{xpubsQuery.error.message}</p>}
      {xpubsQuery.isSuccess && keys.length === 0 && (
        <p className="text-sm text-muted-foreground">No Bitcoin keys yet.</p>
      )}
      <div className="flex flex-col gap-3">
        {keys.map((wallet) => (
          <XpubRow
            key={wallet.id}
            wallet={wallet}
            balance={balancesById.get(wallet.id)}
            onRemove={() => removeMutation.mutate(wallet.id)}
            removing={removeMutation.isPending}
          />
        ))}
      </div>
    </div>
  );
}

export default BitcoinXpubSection;
