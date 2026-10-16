import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import Field from "../components/Field";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

type CredentialSource = "keyring" | "env" | "none";

type Credential = {
  id: string;
  label: string;
  help: string;
  secret: boolean;
  env_var: string;
  source: CredentialSource;
  // Só vem preenchido para credenciais que não são segredo (o e-mail da SEC).
  value: string | null;
};

const SOURCE_TEXT: Record<CredentialSource, string> = {
  keyring: "Saved in the system keyring",
  env: "Set by an environment variable",
  none: "Not configured",
};

// Fase 26 — o Lume foi aposentado e as fontes de dados agora são chamadas direto pelo app.
// Duas delas pedem uma credencial do usuário: a chave da bolsai (fundamentos de ações da B3) e um
// e-mail de contato para a SEC (fundamentos dos EUA). Ficam no keyring do SO, nunca no banco; a
// tela não recebe um segredo de volta, só de onde ele está vindo.
function CredentialRow({ credential }: { credential: Credential }) {
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState("");

  const refresh = () => queryClient.invalidateQueries({ queryKey: ["data-source-credentials"] });

  const saveMutation = useMutation<Credential, AppError, void>({
    mutationFn: () => invoke("set_data_source_credential", { id: credential.id, value: draft }),
    onSuccess: () => {
      setDraft("");
      refresh();
    },
  });

  const clearMutation = useMutation<Credential, AppError, void>({
    mutationFn: () => invoke("clear_data_source_credential", { id: credential.id }),
    onSuccess: refresh,
  });

  const error = saveMutation.error ?? clearMutation.error;
  const placeholder =
    credential.source === "none"
      ? credential.secret
        ? "Paste the key"
        : "you@example.com"
      : credential.secret
        ? "Configured — paste a new key to replace it"
        : (credential.value ?? "");

  return (
    <div className="flex flex-col gap-2 rounded-md border p-4" data-testid={`credential-${credential.id}`}>
      <div className="flex items-baseline justify-between gap-3">
        <h4 className="text-sm font-semibold">{credential.label}</h4>
        <span className="text-xs text-muted-foreground">{SOURCE_TEXT[credential.source]}</span>
      </div>
      <p className="text-sm text-muted-foreground">{credential.help}</p>
      {credential.source === "env" && (
        <p className="text-xs text-muted-foreground">
          Coming from <code>{credential.env_var}</code>. Saving here takes priority over it.
        </p>
      )}
      <Field label="New value">
        <Input
          type={credential.secret ? "password" : "text"}
          value={draft}
          onChange={(e) => setDraft(e.currentTarget.value)}
          placeholder={placeholder}
          autoComplete="off"
        />
      </Field>
      {error && <p className="text-sm text-red-600">{error.message}</p>}
      <div className="flex gap-2">
        <Button
          type="button"
          disabled={draft.trim().length === 0 || saveMutation.isPending}
          onClick={() => saveMutation.mutate()}
        >
          {saveMutation.isPending ? "Saving..." : "Save"}
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={credential.source !== "keyring" || clearMutation.isPending}
          onClick={() => clearMutation.mutate()}
        >
          Remove saved value
        </Button>
      </div>
    </div>
  );
}

function DataSourcesSection() {
  const credentialsQuery = useQuery<Credential[], AppError>({
    queryKey: ["data-source-credentials"],
    queryFn: () => invoke("list_data_source_credentials"),
  });

  return (
    <div className="flex flex-col gap-4">
      <h3 className="text-sm font-semibold text-muted-foreground">Data sources</h3>
      <p className="text-sm text-muted-foreground">
        Prices, rates, fundamentals and indicators come straight from public sources, with no
        account needed. These two sources ask for something from you; both are stored in your
        system keyring, never in the database.
      </p>
      {credentialsQuery.isError && <p className="text-red-600">{credentialsQuery.error.message}</p>}
      {credentialsQuery.data?.map((credential) => (
        <CredentialRow key={credential.id} credential={credential} />
      ))}
    </div>
  );
}

export default DataSourcesSection;
