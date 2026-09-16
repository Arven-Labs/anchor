import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import TruthIdSettingsSection from "../truthid/TruthIdSettingsSection";
import FinanceApiSettingsSection from "./FinanceApiSettingsSection";
import StorageSettingsSection from "./StorageSettingsSection";
import BackupSettingsSection from "./BackupSettingsSection";
import Field from "../components/Field";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export type ApiKeySummary = {
  id: number;
  provider: string;
  name: string;
  created_at: string;
};

export const PROVIDER_LABELS: Record<string, string> = {
  gemini: "Gemini",
  claude: "Claude",
  openai: "OpenAI",
};

const KEY_PLACEHOLDERS: Record<string, string> = {
  gemini: "AIza...",
  claude: "sk-ant-...",
  openai: "sk-...",
};

// Fase 7.9.2 — múltiplas chaves nomeadas por provider, em vez de "1 chave =
// 1 provider" de antes. Renomear só toca a coluna `name` no banco (o
// username do keyring é fixo por id, ver api_key.rs), então não precisa de
// nenhuma chamada extra além de `rename_api_key`.
function IaSettingsSection() {
  const queryClient = useQueryClient();

  const keysQuery = useQuery<ApiKeySummary[], AppError>({
    queryKey: ["api-keys"],
    queryFn: () => invoke("list_api_keys"),
  });

  const [provider, setProvider] = useState("gemini");
  const [name, setName] = useState("");
  const [key, setKey] = useState("");
  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<number | null>(
    null,
  );

  const createMutation = useMutation<number, AppError, void>({
    mutationFn: () =>
      invoke("create_api_key", {
        provider,
        name: name.trim(),
        key: key.trim(),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["api-keys"] });
      setName("");
      setKey("");
    },
  });

  const renameMutation = useMutation<
    void,
    AppError,
    { id: number; name: string }
  >({
    mutationFn: ({ id, name }) => invoke("rename_api_key", { id, name }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["api-keys"] });
      setRenamingId(null);
    },
  });

  const deleteMutation = useMutation<void, AppError, number>({
    mutationFn: (id) => invoke("delete_api_key", { id }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["api-keys"] });
      setConfirmingDeleteId(null);
    },
  });

  function handleCreate(event: FormEvent) {
    event.preventDefault();
    if (!name.trim() || !key.trim() || createMutation.isPending) return;
    createMutation.mutate();
  }

  function handleDeleteClick(id: number) {
    if (confirmingDeleteId === id) {
      deleteMutation.mutate(id);
    } else {
      setConfirmingDeleteId(id);
    }
  }

  const keys = keysQuery.data ?? [];
  const grouped = keys.reduce<Record<string, ApiKeySummary[]>>((acc, k) => {
    (acc[k.provider] ??= []).push(k);
    return acc;
  }, {});

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h3 className="mb-3 text-sm font-semibold text-muted-foreground">
          API Keys
        </h3>
        {keysQuery.isError && (
          <p className="text-red-600">{keysQuery.error.message}</p>
        )}
        {keys.length === 0 && !keysQuery.isLoading && (
          <p className="text-muted-foreground">
            No keys configured yet.
          </p>
        )}
        <div className="flex flex-col gap-4">
          {Object.entries(grouped).map(([providerId, providerKeys]) => (
            <div key={providerId}>
              <p className="mb-2 text-sm font-medium">
                {PROVIDER_LABELS[providerId] ?? providerId}
              </p>
              <div className="flex flex-col gap-2">
                {providerKeys.map((k) => (
                  <div
                    key={k.id}
                    className="flex items-center gap-2 rounded-lg border border-border bg-card p-3"
                  >
                    {renamingId === k.id ? (
                      <Input
                        autoFocus
                        value={renameDraft}
                        onChange={(e) => setRenameDraft(e.currentTarget.value)}
                        onBlur={() =>
                          renameMutation.mutate({
                            id: k.id,
                            name: renameDraft.trim() || k.name,
                          })
                        }
                        onKeyDown={(e) => {
                          if (e.key === "Enter") e.currentTarget.blur();
                        }}
                        className="h-8 flex-1"
                      />
                    ) : (
                      <button
                        type="button"
                        className="flex-1 text-left text-sm"
                        onClick={() => {
                          setRenamingId(k.id);
                          setRenameDraft(k.name);
                        }}
                      >
                        {k.name}
                      </button>
                    )}
                    <Button
                      type="button"
                      variant={confirmingDeleteId === k.id ? "destructive" : "outline"}
                      size="sm"
                      onClick={() => handleDeleteClick(k.id)}
                    >
                      {confirmingDeleteId === k.id ? "Confirm?" : "Delete"}
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>

      <form onSubmit={handleCreate} className="flex flex-col gap-4">
        <h3 className="text-sm font-semibold text-muted-foreground">
          New key
        </h3>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <Field label="Provider">
            <Select value={provider} onValueChange={setProvider}>
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {Object.entries(PROVIDER_LABELS).map(([id, label]) => (
                  <SelectItem key={id} value={id}>
                    {label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
          <Field label="Name">
            <Input
              value={name}
              onChange={(e) => setName(e.currentTarget.value)}
              placeholder="e.g.: personal"
            />
          </Field>
          <Field label="Key">
            <Input
              type="password"
              value={key}
              onChange={(e) => setKey(e.currentTarget.value)}
              placeholder={KEY_PLACEHOLDERS[provider] ?? ""}
            />
          </Field>
        </div>
        {createMutation.isError && (
          <p className="text-red-600">{createMutation.error.message}</p>
        )}
        <Button
          type="submit"
          className="w-fit"
          disabled={!name.trim() || !key.trim() || createMutation.isPending}
        >
          {createMutation.isPending ? "Saving..." : "Add key"}
        </Button>
      </form>
    </div>
  );
}

// "TruthID" migrou pra cá na Sessão 54 (era aba própria no nav principal,
// "TruthID Sync") — pedido explícito do dono do projeto. Confirma que o
// layout de sidebar + conteúdo já pensado pra caber mais de uma seção
// (comentário original, quando só "IA" existia) funcionou sem redesenho;
// ainda sem abstração de registro de seções — só 2 itens, não vale a pena.
const SECTIONS = ["IA", "TruthID", "Finance API", "Storage", "Backup"] as const;

function SettingsPage({ onBack }: { onBack: () => void }) {
  const [section, setSection] = useState<(typeof SECTIONS)[number]>("IA");

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-3">
          <Button variant="outline" size="sm" onClick={onBack}>
            ← Back
          </Button>
          <CardTitle>Settings</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="flex gap-6">
        <nav className="flex w-40 shrink-0 flex-col gap-1">
          {SECTIONS.map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => setSection(s)}
              className={`rounded-md px-3 py-2 text-left text-sm ${
                section === s
                  ? "bg-muted font-medium"
                  : "text-muted-foreground hover:bg-muted/50"
              }`}
            >
              {s}
            </button>
          ))}
        </nav>
        <div className="flex-1">
          {section === "IA" && <IaSettingsSection />}
          {section === "TruthID" && <TruthIdSettingsSection />}
          {section === "Finance API" && <FinanceApiSettingsSection />}
          {section === "Storage" && <StorageSettingsSection />}
          {section === "Backup" && <BackupSettingsSection />}
        </div>
      </CardContent>
    </Card>
  );
}

export default SettingsPage;
