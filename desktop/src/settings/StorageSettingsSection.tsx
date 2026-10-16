import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

type StorageProvider =
  | "local"
  | "self_hosted"
  | "managed_cloud"
  | "decentralized_vault";

type StorageSettingsView = {
  provider: StorageProvider;
};

const PROVIDER_OPTIONS: {
  value: StorageProvider;
  label: string;
  description: string;
  disabled: boolean;
}[] = [
  {
    value: "local",
    label: "Local disk (default)",
    description: "Files stay on this computer. Free, no server required.",
    disabled: false,
  },
  {
    value: "self_hosted",
    label: "Self-hosted server (coming soon)",
    description:
      "Files go to a server you run yourself. Free, but that server needs to be online.",
    disabled: true,
  },
  {
    value: "managed_cloud",
    label: "Managed cloud (coming soon)",
    description: "Hosted for you. Paid subscription, not available yet.",
    disabled: true,
  },
  {
    value: "decentralized_vault",
    label: "Decentralized vault (coming soon)",
    description:
      "Encrypted and pinned via the TruthID Vault (Web3). Paid subscription, not available yet.",
    disabled: true,
  },
];

// Fase 15 — StorageProvider seletivo (spec da Sessão 95). Só "Local disk" tem implementação real
// nesta fatia (LocalFSProvider), as outras 3 ficam reservadas no enum/UI ("coming soon") pra não
// exigir migração de schema quando ganharem implementação de verdade.
function StorageSettingsSection() {
  const queryClient = useQueryClient();

  const settingsQuery = useQuery<StorageSettingsView, AppError>({
    queryKey: ["storage-settings"],
    queryFn: () => invoke("get_storage_settings"),
  });

  const [provider, setProvider] = useState<StorageProvider>("local");

  useEffect(() => {
    if (!settingsQuery.data) return;
    setProvider(settingsQuery.data.provider);
  }, [settingsQuery.data]);

  const saveMutation = useMutation<void, AppError, void>({
    mutationFn: () => invoke("set_storage_settings", { provider }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["storage-settings"] });
    },
  });

  const selected = PROVIDER_OPTIONS.find((option) => option.value === provider);

  return (
    <div className="flex flex-col gap-4">
      <h3 className="text-sm font-semibold text-muted-foreground">Storage</h3>
      <p className="text-sm text-muted-foreground">
        Where attachments and financial documents (statements, receipts,
        invoices) are stored.
      </p>
      {settingsQuery.isError && (
        <p className="text-red-600">{settingsQuery.error.message}</p>
      )}

      <Field label="Provider">
        <Select
          value={provider}
          onValueChange={(value) => setProvider(value as StorageProvider)}
        >
          <SelectTrigger className="w-full" data-testid="storage-provider">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {PROVIDER_OPTIONS.map((option) => (
              <SelectItem
                key={option.value}
                value={option.value}
                disabled={option.disabled}
              >
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>
      {selected && (
        <p className="text-sm text-muted-foreground">{selected.description}</p>
      )}

      {saveMutation.isError && (
        <p className="text-red-600">{saveMutation.error.message}</p>
      )}
      <Button
        type="button"
        className="w-fit"
        data-testid="storage-save"
        disabled={saveMutation.isPending}
        onClick={() => saveMutation.mutate()}
      >
        {saveMutation.isPending ? "Saving..." : "Save"}
      </Button>
    </div>
  );
}

export default StorageSettingsSection;
