import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { Workspace } from "../portfolio/types";
import Field from "../components/Field";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

type CreateWorkspaceRequest = { name: string };

// Fase 10.6 — tela de entrada antes do app: escolha (ou crie) um Workspace
// antes de ver as abas normais. Ainda sem dono/convite (workspace continua
// só "seu", sem TruthID) — a versão com login real fica pra quando a 10.7
// destravar (ver PHASE.md item 10.7).
function WorkspaceGate({ onEnter }: { onEnter: (workspaceId: number) => void }) {
  const [newWorkspaceName, setNewWorkspaceName] = useState("");
  const queryClient = useQueryClient();

  const workspacesQuery = useQuery<Workspace[], AppError>({
    queryKey: ["workspaces"],
    queryFn: () => invoke("list_workspaces"),
  });
  const workspaces = workspacesQuery.data ?? [];

  const createWorkspaceMutation = useMutation<Workspace, AppError, CreateWorkspaceRequest>({
    mutationFn: (request) => invoke("create_workspace", { request }),
    onSuccess: (created) => {
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
      setNewWorkspaceName("");
      onEnter(created.id);
    },
  });

  function handleCreate(event: FormEvent) {
    event.preventDefault();
    createWorkspaceMutation.mutate({ name: newWorkspaceName });
  }

  return (
    <div className="flex min-h-[60vh] items-center justify-center">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Choose a Workspace</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-6">
          {workspacesQuery.isError && (
            <p className="text-red-600">{workspacesQuery.error.message}</p>
          )}
          {workspacesQuery.isLoading && (
            <p className="text-muted-foreground">Loading...</p>
          )}

          <div className="flex flex-col gap-2">
            {workspaces.map((workspace) => (
              <Button
                key={workspace.id}
                type="button"
                variant="outline"
                className="justify-start"
                onClick={() => onEnter(workspace.id)}
              >
                {workspace.name}
              </Button>
            ))}
          </div>

          <form onSubmit={handleCreate} className="flex flex-col gap-3 border-t pt-4">
            <Field label="New Workspace">
              <Input
                required
                data-testid="workspace-name"
                placeholder="e.g.: Family"
                value={newWorkspaceName}
                onChange={(e) => setNewWorkspaceName(e.currentTarget.value)}
              />
            </Field>
            {createWorkspaceMutation.isError && (
              <p className="text-red-600">{createWorkspaceMutation.error.message}</p>
            )}
            <Button
              type="submit"
              data-testid="workspace-create"
              disabled={createWorkspaceMutation.isPending}
            >
              {createWorkspaceMutation.isPending ? "Creating..." : "Create and enter"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}

export default WorkspaceGate;
