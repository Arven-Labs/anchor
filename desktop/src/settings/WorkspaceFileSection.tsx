import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { Button } from "@/components/ui/button";

// Fase 34 — arquivo portátil de UM Workspace. Diferente do backup (o banco
// inteiro, restauração total), exportar leva só este Workspace e importar cria
// um Workspace novo ao lado dos que já existem, sem tocar neles.

type ExportSummary = { workspace_name: string; table_count: number; row_count: number };
type ImportSummary = { workspace_id: number; workspace_name: string; table_count: number; row_count: number };

function WorkspaceFileSection({ workspaceId }: { workspaceId: number }) {
  const queryClient = useQueryClient();

  const exportMutation = useMutation<ExportSummary, AppError, void>({
    mutationFn: async () => {
      const destPath = await save({
        defaultPath: `anchor-workspace-${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "Anchor Workspace", extensions: ["json"] }],
      });
      if (!destPath) throw new Error("cancelled");
      return invoke("export_workspace_file", { workspaceId, destPath });
    },
  });

  const importMutation = useMutation<ImportSummary, AppError, void>({
    mutationFn: async () => {
      const sourcePath = await open({
        multiple: false,
        filters: [{ name: "Anchor Workspace", extensions: ["json"] }],
      });
      if (typeof sourcePath !== "string") throw new Error("cancelled");
      return invoke("import_workspace_file", { sourcePath });
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["workspaces"] }),
  });

  return (
    <div className="flex flex-col gap-4" data-testid="workspace-file-section">
      <div>
        <h4 className="text-sm font-semibold text-muted-foreground">Workspace file</h4>
        <p className="mt-1 text-sm text-muted-foreground">
          Move a single Workspace between computers or keep a copy of it. Importing never overwrites
          anything: it creates a new Workspace next to the existing ones. Attachments are not included.
        </p>
      </div>

      <div>
        <Button
          type="button"
          variant="outline"
          className="w-fit"
          data-testid="workspace-file-export"
          disabled={exportMutation.isPending}
          onClick={() => exportMutation.mutate()}
        >
          {exportMutation.isPending ? "Exporting..." : "Export this Workspace"}
        </Button>
        {exportMutation.isError && exportMutation.error.message !== "cancelled" && (
          <p className="mt-2 text-red-600">{exportMutation.error.message}</p>
        )}
        {exportMutation.isSuccess && (
          <p className="mt-2 text-sm text-muted-foreground" data-testid="workspace-file-exported">
            Exported "{exportMutation.data.workspace_name}" — {exportMutation.data.row_count} rows.
          </p>
        )}
      </div>

      <div>
        <Button
          type="button"
          variant="outline"
          className="w-fit"
          data-testid="workspace-file-import"
          disabled={importMutation.isPending}
          onClick={() => importMutation.mutate()}
        >
          {importMutation.isPending ? "Importing..." : "Import as a new Workspace"}
        </Button>
        {importMutation.isError && importMutation.error.message !== "cancelled" && (
          <p className="mt-2 text-red-600">{importMutation.error.message}</p>
        )}
        {importMutation.isSuccess && (
          <p className="mt-2 text-sm text-muted-foreground" data-testid="workspace-file-imported">
            Created the Workspace "{importMutation.data.workspace_name}" ({importMutation.data.row_count} rows).
            Switch to it from the Workspace list.
          </p>
        )}
      </div>
    </div>
  );
}

export default WorkspaceFileSection;
