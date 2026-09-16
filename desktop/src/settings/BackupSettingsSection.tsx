import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useMutation } from "@tanstack/react-query";
import type { AppError } from "../types";
import { Button } from "@/components/ui/button";

type BackupSummary = {
  table_count: number;
  row_count: number;
  attachment_count: number;
};

function summaryText(summary: BackupSummary) {
  return `${summary.table_count} tables, ${summary.row_count} rows, ${summary.attachment_count} attachments`;
}

// Fase 16 — export/import estruturado (JSON) do Workspace inteiro (banco + anexos via
// `StorageProvider`, Fase 15). Sem "configuração" persistida — só as 2 ações.
function BackupSettingsSection() {
  const [confirmingImport, setConfirmingImport] = useState(false);

  const exportMutation = useMutation<BackupSummary, AppError, void>({
    mutationFn: async () => {
      const destPath = await save({
        defaultPath: `anchor-backup-${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "Anchor backup", extensions: ["json"] }],
      });
      if (!destPath) throw new Error("cancelled");
      return invoke("export_workspace_backup", { destPath });
    },
  });

  const importMutation = useMutation<BackupSummary, AppError, void>({
    mutationFn: async () => {
      const sourcePath = await open({
        multiple: false,
        filters: [{ name: "Anchor backup", extensions: ["json"] }],
      });
      if (typeof sourcePath !== "string") throw new Error("cancelled");
      return invoke("import_workspace_backup", { sourcePath });
    },
    onSettled: () => setConfirmingImport(false),
  });

  function handleImportClick() {
    if (confirmingImport) {
      importMutation.mutate();
    } else {
      setConfirmingImport(true);
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h3 className="text-sm font-semibold text-muted-foreground">Backup</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Save or restore a snapshot of this entire Workspace (data + attachments) as a single
          file on this computer. This is a manual, on-demand snapshot — different from the
          continuous multi-device sync feature.
        </p>
      </div>

      <div>
        <h4 className="mb-2 text-sm font-medium">Export backup</h4>
        <Button
          type="button"
          className="w-fit"
          disabled={exportMutation.isPending}
          onClick={() => exportMutation.mutate()}
        >
          {exportMutation.isPending ? "Exporting..." : "Export backup"}
        </Button>
        {exportMutation.isError && exportMutation.error.message !== "cancelled" && (
          <p className="mt-2 text-red-600">{exportMutation.error.message}</p>
        )}
        {exportMutation.isSuccess && (
          <p className="mt-2 text-sm text-muted-foreground">
            Done — {summaryText(exportMutation.data)}.
          </p>
        )}
      </div>

      <div>
        <h4 className="mb-2 text-sm font-medium">Import backup</h4>
        <p className="mb-2 text-sm text-muted-foreground">
          This replaces all current data in this Workspace with the contents of the backup file.
          It cannot be undone.
        </p>
        <Button
          type="button"
          variant={confirmingImport ? "destructive" : "outline"}
          className="w-fit"
          disabled={importMutation.isPending}
          onClick={handleImportClick}
        >
          {importMutation.isPending
            ? "Importing..."
            : confirmingImport
              ? "Confirm — overwrite all data?"
              : "Import backup"}
        </Button>
        {importMutation.isError && importMutation.error.message !== "cancelled" && (
          <p className="mt-2 text-red-600">{importMutation.error.message}</p>
        )}
        {importMutation.isSuccess && (
          <p className="mt-2 text-sm text-muted-foreground">
            Restored — {summaryText(importMutation.data)}.
          </p>
        )}
      </div>
    </div>
  );
}

export default BackupSettingsSection;
