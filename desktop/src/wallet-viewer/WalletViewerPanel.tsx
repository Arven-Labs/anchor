import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import Field from "../components/Field";
import BitcoinXpubSection from "./BitcoinXpubSection";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  formatEth,
  WALLET_VIEWER_ADDRESSES_KEY,
  type AddressBalances,
  type NetworkBalance,
  type WalletAddress,
} from "./types";

// ADR 0002 — visualizador de wallets: só leitura. Cadastra endereços públicos e mostra o saldo
// nativo (ETH) na Ethereum e na Base. Não tem relação com a carteira de investimentos e nunca
// assina nada. O saldo só é buscado quando o usuário clica em "Refresh balances": cada consulta
// revela o endereço ao provedor de RPC público.
function BalanceLine({ balance }: { balance: NetworkBalance }) {
  return (
    <div className="flex items-baseline justify-between gap-3 text-sm">
      <span className="text-muted-foreground">{balance.network}</span>
      {balance.error ? (
        <span className="text-red-600" title={balance.error}>
          Could not read the balance
        </span>
      ) : (
        <span className="font-mono" title={balance.wei ? `${balance.wei} wei` : undefined}>
          {balance.eth !== null ? `${formatEth(balance.eth)} ETH` : "—"}
        </span>
      )}
    </div>
  );
}

function AddressRow({
  wallet,
  balances,
  onRemove,
  removing,
}: {
  wallet: WalletAddress;
  balances: NetworkBalance[] | undefined;
  onRemove: () => void;
  removing: boolean;
}) {
  return (
    <div className="flex flex-col gap-2 rounded-md border p-4" data-testid={`wallet-address-${wallet.id}`}>
      <div className="flex items-baseline justify-between gap-3">
        <div className="min-w-0">
          {wallet.label && <h4 className="text-sm font-semibold">{wallet.label}</h4>}
          <p className="break-all font-mono text-xs text-muted-foreground">{wallet.address}</p>
        </div>
        <Button type="button" variant="outline" size="sm" disabled={removing} onClick={onRemove}>
          Remove
        </Button>
      </div>
      {balances ? (
        balances.map((balance) => <BalanceLine key={balance.network} balance={balance} />)
      ) : (
        <p className="text-xs text-muted-foreground">Balances not loaded yet.</p>
      )}
    </div>
  );
}

// Fases 36.2 e 36.3 — lê o endereço de uma conta da Ledger (USB, app Ethereum aberto) ou do Trezor
// (USB; PIN e passphrase no aparelho) e só grava depois que o usuário confere e clica em "Add".
// Nada é assinado: o Rust só sabe pedir o endereço. Os `data-testid` levam o nome do aparelho.
const MAX_ACCOUNT = 2147483647;

type Device = "ledger" | "trezor";

const DEVICES: Record<Device, { name: string; command: string }> = {
  ledger: { name: "Ledger", command: "wallet_viewer_read_ledger_address" },
  trezor: { name: "Trezor", command: "wallet_viewer_read_trezor_address" },
};

type ReadRequest = { device: Device; account: number };

function HardwareWalletImport() {
  const queryClient = useQueryClient();
  const [account, setAccount] = useState("0");
  const [label, setLabel] = useState("");

  const trimmed = account.trim();
  const validAccount = /^\d+$/.test(trimmed) && Number(trimmed) <= MAX_ACCOUNT;

  const readMutation = useMutation<string, AppError, ReadRequest>({
    mutationFn: ({ device, account: index }) => invoke(DEVICES[device].command, { account: index }),
    onSuccess: (_address, { device, account: index }) => setLabel(`${DEVICES[device].name} · account ${index}`),
  });

  const addMutation = useMutation<WalletAddress, AppError, { address: string; label: string }>({
    mutationFn: (input) =>
      invoke("wallet_viewer_add_address", {
        address: input.address,
        label: input.label.trim() === "" ? null : input.label,
      }),
    onSuccess: () => {
      readMutation.reset();
      setLabel("");
      queryClient.invalidateQueries({ queryKey: WALLET_VIEWER_ADDRESSES_KEY });
    },
  });

  const readAddress = readMutation.data;
  const lastDevice = readMutation.variables?.device;

  const read = (device: Device) => {
    addMutation.reset();
    readMutation.mutate({ device, account: Number(trimmed) });
  };

  return (
    <div className="flex flex-col gap-3 rounded-md border p-4" data-testid="hardware-import">
      <h4 className="text-sm font-semibold">Read from a hardware wallet</h4>
      <p className="text-xs text-muted-foreground">
        Plug in the device and unlock it. On a Ledger, open the Ethereum app; on a Trezor, enter the
        PIN and passphrase on the device if it asks. Only the address of the account is read (path
        m/44'/60'/account'/0/0, as in Ledger Live and Trezor Suite); nothing is signed.
      </p>
      <form
        className="grid gap-3 sm:grid-cols-[1fr_auto_auto] sm:items-end"
        onSubmit={(event) => event.preventDefault()}
      >
        <Field label="Account">
          <Input
            value={account}
            onChange={(e) => setAccount(e.currentTarget.value)}
            inputMode="numeric"
            autoComplete="off"
            data-testid="hardware-account"
          />
        </Field>
        {(Object.keys(DEVICES) as Device[]).map((device) => (
          <Button
            key={device}
            type="button"
            variant="outline"
            disabled={!validAccount || readMutation.isPending}
            onClick={() => read(device)}
            data-testid={`${device}-read`}
          >
            {readMutation.isPending && lastDevice === device ? "Reading..." : `Read from ${DEVICES[device].name}`}
          </Button>
        ))}
      </form>
      {!validAccount && <p className="text-sm text-red-600">The account must be a whole number from 0 to {MAX_ACCOUNT}.</p>}
      {readMutation.isError && lastDevice && (
        <p className="text-sm text-red-600" data-testid={`${lastDevice}-error`}>
          {readMutation.error.message}
        </p>
      )}
      {readAddress && lastDevice && (
        <form
          className="grid gap-3 sm:grid-cols-[2fr_1fr_auto] sm:items-end"
          onSubmit={(event) => {
            event.preventDefault();
            addMutation.mutate({ address: readAddress, label });
          }}
        >
          <Field label={`Address read from the ${DEVICES[lastDevice].name}`}>
            <p className="break-all font-mono text-xs" data-testid={`${lastDevice}-address`}>
              {readAddress}
            </p>
          </Field>
          <Field label="Label (optional)">
            <Input value={label} onChange={(e) => setLabel(e.currentTarget.value)} autoComplete="off" />
          </Field>
          <Button type="submit" disabled={addMutation.isPending} data-testid={`${lastDevice}-add`}>
            {addMutation.isPending ? "Adding..." : "Add"}
          </Button>
        </form>
      )}
      {addMutation.isError && <p className="text-sm text-red-600">{addMutation.error.message}</p>}
    </div>
  );
}

function WalletViewerPanel() {
  const queryClient = useQueryClient();
  const [address, setAddress] = useState("");
  const [label, setLabel] = useState("");

  const addressesQuery = useQuery<WalletAddress[], AppError>({
    queryKey: WALLET_VIEWER_ADDRESSES_KEY,
    queryFn: () => invoke("wallet_viewer_list_addresses"),
  });

  const balancesMutation = useMutation<AddressBalances[], AppError, void>({
    mutationFn: () => invoke("wallet_viewer_get_balances"),
  });

  const addMutation = useMutation<WalletAddress, AppError, void>({
    mutationFn: () =>
      invoke("wallet_viewer_add_address", { address, label: label.trim() === "" ? null : label }),
    onSuccess: () => {
      setAddress("");
      setLabel("");
      queryClient.invalidateQueries({ queryKey: WALLET_VIEWER_ADDRESSES_KEY });
    },
  });

  const removeMutation = useMutation<void, AppError, number>({
    mutationFn: (id) => invoke("wallet_viewer_remove_address", { id }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: WALLET_VIEWER_ADDRESSES_KEY }),
  });

  const balancesById = new Map(balancesMutation.data?.map((entry) => [entry.id, entry.balances]));
  const wallets = addressesQuery.data ?? [];

  return (
    <Card>
      <CardHeader>
        <CardTitle>Wallets</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-6">
        <p className="text-sm text-muted-foreground">
          A read-only view of public addresses (Ethereum and Base, native ETH) and Bitcoin public
          keys (xpub and zpub). It is separate from
          your portfolio: nothing here is counted in positions, returns or taxes, and nothing is
          ever signed.
        </p>

        <form
          className="grid gap-3 sm:grid-cols-[2fr_1fr_auto] sm:items-end"
          onSubmit={(event) => {
            event.preventDefault();
            addMutation.mutate();
          }}
        >
          <Field label="Address">
            <Input
              value={address}
              onChange={(e) => setAddress(e.currentTarget.value)}
              placeholder="0x…"
              autoComplete="off"
              spellCheck={false}
              data-testid="wallet-address-input"
            />
          </Field>
          <Field label="Label (optional)">
            <Input value={label} onChange={(e) => setLabel(e.currentTarget.value)} autoComplete="off" />
          </Field>
          <Button type="submit" disabled={address.trim().length === 0 || addMutation.isPending}>
            {addMutation.isPending ? "Adding..." : "Add"}
          </Button>
        </form>
        {addMutation.isError && <p className="text-sm text-red-600">{addMutation.error.message}</p>}

        <HardwareWalletImport />

        <BitcoinXpubSection />

        {removeMutation.isError && <p className="text-sm text-red-600">{removeMutation.error.message}</p>}

        <div className="flex items-center gap-3">
          <Button
            type="button"
            variant="outline"
            disabled={wallets.length === 0 || balancesMutation.isPending}
            onClick={() => balancesMutation.mutate()}
            data-testid="wallet-refresh"
          >
            {balancesMutation.isPending ? "Reading..." : "Refresh balances"}
          </Button>
          <span className="text-xs text-muted-foreground">
            Each refresh asks public RPC providers for the balance, which tells them the address.
          </span>
        </div>
        {balancesMutation.isError && <p className="text-sm text-red-600">{balancesMutation.error.message}</p>}

        {addressesQuery.isError && <p className="text-red-600">{addressesQuery.error.message}</p>}
        {addressesQuery.isSuccess && wallets.length === 0 && (
          <p className="text-sm text-muted-foreground">No addresses yet.</p>
        )}
        <div className="flex flex-col gap-3">
          {wallets.map((wallet) => (
            <AddressRow
              key={wallet.id}
              wallet={wallet}
              balances={balancesById.get(wallet.id)}
              onRemove={() => removeMutation.mutate(wallet.id)}
              removing={removeMutation.isPending}
            />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

export default WalletViewerPanel;
