import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { AppError } from "../types";
import type { Asset, AssetClass, Thesis } from "../portfolio/types";
import type { CreateAssetRequest } from "./shared";
import Field from "../components/Field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type CreateThesisRequest = {
  workspace_id: number;
  asset_id: number | null;
  title: string;
  content_markdown: string;
};
type UpdateThesisRequest = {
  thesis_id: number;
  asset_id: number | null;
  title: string;
  content_markdown: string;
};

/// Fase 10, item 8 (Research) reaproveitando Fase 10.5 (Teses) — Sessão 61:
/// lista compacta das teses já vinculadas a este ticker, direto na tela de
/// análise (antes só dava pra ver/criar teses em Portfolio > Teses, sem
/// filtro por ativo). `list_theses` só filtra por workspace — o filtro por
/// ticker é client-side comparando `thesis.asset_id` com o `id` do ativo no
/// catálogo (`assets`), resolvido pelo `ticker`. Sem tabela nem comando
/// novo no backend. Se o ticker ainda não estiver cadastrado em `assets`
/// (botão "Add to Assets" no topo da tela ainda não clicado), criar a
/// primeira tese registra o ativo primeiro, silenciosamente.
function AssetThesesSidebar({
  workspaceId,
  ticker,
  assetClass,
  name,
  currency,
  exchange,
  cnpj,
  exposureType = "pais",
  exposureValue = "BR",
}: {
  workspaceId: number;
  ticker: string;
  assetClass: AssetClass;
  name: string;
  currency: string;
  exchange: string | null;
  cnpj: string | null;
  exposureType?: "pais" | "categoria_especial";
  exposureValue?: string;
}) {
  const queryClient = useQueryClient();
  const [dialogThesisId, setDialogThesisId] = useState<number | "new" | null>(null);
  const [formTitle, setFormTitle] = useState("");
  const [formContent, setFormContent] = useState("");
  const [formMode, setFormMode] = useState<"edit" | "preview">("edit");
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<number | null>(null);

  const assetsQuery = useQuery<Asset[], AppError>({
    queryKey: ["assets"],
    queryFn: () => invoke("list_assets"),
  });
  const asset = (assetsQuery.data ?? []).find((a) => a.ticker === ticker) ?? null;

  const thesesQuery = useQuery<Thesis[], AppError>({
    queryKey: ["theses", workspaceId],
    queryFn: () => invoke("list_theses", { workspaceId }),
  });
  const assetTheses = (thesesQuery.data ?? [])
    .filter((t) => asset !== null && t.asset_id === asset.id)
    .sort((a, b) => b.updated_at.localeCompare(a.updated_at));

  const ensureAssetMutation = useMutation<Asset, AppError, void>({
    mutationFn: () =>
      invoke("create_asset", {
        request: {
          ticker,
          name,
          asset_class: assetClass,
          currency,
          exchange,
          exposure_type: exposureType,
          exposure_value: exposureValue,
          cnpj,
        } satisfies CreateAssetRequest,
      }),
    onSuccess: (created) => {
      queryClient.setQueryData<Asset[]>(["assets"], (prev) => [...(prev ?? []), created]);
    },
  });

  function closeDialog() {
    setDialogThesisId(null);
    setConfirmingDeleteId(null);
  }

  const createMutation = useMutation<Thesis, AppError, void>({
    mutationFn: async () => {
      const assetId = asset?.id ?? (await ensureAssetMutation.mutateAsync()).id;
      return invoke("create_thesis", {
        request: {
          workspace_id: workspaceId,
          asset_id: assetId,
          title: formTitle,
          content_markdown: formContent,
        } satisfies CreateThesisRequest,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["theses", workspaceId] });
      closeDialog();
    },
  });

  const updateMutation = useMutation<Thesis, AppError, void>({
    mutationFn: () => {
      if (typeof dialogThesisId !== "number") throw new Error("no thesis open");
      return invoke("update_thesis", {
        request: {
          thesis_id: dialogThesisId,
          asset_id: asset?.id ?? null,
          title: formTitle,
          content_markdown: formContent,
        } satisfies UpdateThesisRequest,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["theses", workspaceId] });
      closeDialog();
    },
  });

  const deleteMutation = useMutation<void, AppError, number>({
    mutationFn: (thesisId) => invoke("delete_thesis", { thesisId }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["theses", workspaceId] });
      closeDialog();
    },
  });

  function openForCreating() {
    setFormTitle("");
    setFormContent("");
    setFormMode("edit");
    setDialogThesisId("new");
  }

  function openForEditing(thesis: Thesis) {
    setFormTitle(thesis.title);
    setFormContent(thesis.content_markdown);
    setFormMode("edit");
    setConfirmingDeleteId(null);
    setDialogThesisId(thesis.id);
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (dialogThesisId === "new") {
      createMutation.mutate();
    } else if (typeof dialogThesisId === "number") {
      updateMutation.mutate();
    }
  }

  const isSaving =
    createMutation.isPending || ensureAssetMutation.isPending || updateMutation.isPending;
  const saveError = createMutation.error ?? updateMutation.error ?? ensureAssetMutation.error;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-muted-foreground">Theses</h3>
        <Button type="button" size="sm" variant="outline" onClick={openForCreating}>
          New thesis
        </Button>
      </div>

      {thesesQuery.isError && <p className="text-xs text-red-600">{thesesQuery.error.message}</p>}

      {assetTheses.length === 0 ? (
        <p className="text-sm text-muted-foreground">No theses for {ticker} yet.</p>
      ) : (
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {assetTheses.map((thesis) => (
            <button
              key={thesis.id}
              type="button"
              onClick={() => openForEditing(thesis)}
              className="rounded-lg border border-border bg-card p-4 text-left text-sm hover:bg-accent"
            >
              <p className="truncate font-medium">{thesis.title}</p>
              <p className="text-xs text-muted-foreground">
                {new Date(thesis.updated_at).toLocaleDateString()}
              </p>
            </button>
          ))}
        </div>
      )}

      <Dialog open={dialogThesisId !== null} onOpenChange={(open) => !open && closeDialog()}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>
              {dialogThesisId === "new" ? `New thesis — ${ticker}` : `Edit thesis — ${ticker}`}
            </DialogTitle>
          </DialogHeader>
          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <Field label="Title">
              <Input
                required
                value={formTitle}
                onChange={(e) => setFormTitle(e.currentTarget.value)}
              />
            </Field>

            <Field label="Content (Markdown)">
              <div className="mb-2 flex gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant={formMode === "edit" ? "default" : "outline"}
                  onClick={() => setFormMode("edit")}
                >
                  Edit
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant={formMode === "preview" ? "default" : "outline"}
                  onClick={() => setFormMode("preview")}
                >
                  Preview
                </Button>
              </div>
              {formMode === "edit" ? (
                <Textarea
                  className="min-h-48 font-mono text-sm"
                  value={formContent}
                  onChange={(e) => setFormContent(e.currentTarget.value)}
                />
              ) : (
                <div className="min-h-48 rounded-lg border border-input px-3 py-2">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>
                    {formContent || "*Nothing to show yet.*"}
                  </ReactMarkdown>
                </div>
              )}
            </Field>

            {saveError && <p className="text-sm text-red-600">{saveError.message}</p>}

            <div className="flex items-center justify-between">
              {typeof dialogThesisId === "number" ? (
                <Button
                  type="button"
                  variant={confirmingDeleteId === dialogThesisId ? "destructive" : "outline"}
                  size="sm"
                  onClick={() => {
                    if (confirmingDeleteId === dialogThesisId) {
                      deleteMutation.mutate(dialogThesisId);
                    } else {
                      setConfirmingDeleteId(dialogThesisId);
                    }
                  }}
                >
                  {confirmingDeleteId === dialogThesisId ? "Confirm delete?" : "Delete"}
                </Button>
              ) : (
                <span />
              )}
              <div className="flex gap-2">
                <Button type="button" variant="outline" onClick={closeDialog} disabled={isSaving}>
                  Cancel
                </Button>
                <Button type="submit" disabled={isSaving}>
                  {isSaving ? "Saving..." : "Save"}
                </Button>
              </div>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}

export default AssetThesesSidebar;
