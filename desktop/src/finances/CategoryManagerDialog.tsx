import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { GeneralTransactionCategory } from "./types";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import Field from "../components/Field";
import { useBaseCurrency } from "./useBaseCurrency";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

type CreateCategoryRequest = {
  workspace_id: number;
  nome: string;
  limite_mensal: number | null;
};

type UpdateCategoryRequest = {
  general_transaction_category_id: number;
  nome: string;
  limite_mensal: number | null;
};

// Fase 12 — cadastro de categorias, Sessão 71 (feedback do dono do projeto
// depois do teste ao vivo do núcleo: escolher numa lista em vez de digitar
// texto livre a cada lançamento). Popup em vez de aba própria — pedido
// explícito do dono do projeto, gerenciar a partir da tela de Transactions
// mesmo. Limite de orçamento mensal (recorrente, mesmo valor todo mês) e
// edição adicionados na Sessão 75 — fatia deixada de fora de propósito na
// primeira versão.
function CategoryManagerDialog({
  workspaceId,
  open,
  onOpenChange,
}: {
  workspaceId: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [nome, setNome] = useState("");
  const [limiteMensal, setLimiteMensal] = useState("");
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<number | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editNome, setEditNome] = useState("");
  const [editLimiteMensal, setEditLimiteMensal] = useState("");

  const queryClient = useQueryClient();

  const categoriesQuery = useQuery<GeneralTransactionCategory[], AppError>({
    queryKey: ["general-transaction-categories", workspaceId],
    queryFn: () => invoke("list_general_transaction_categories", { workspaceId }),
  });

  const baseCurrency = useBaseCurrency(workspaceId);
  const missingFx = (categoriesQuery.data ?? []).reduce((sum, c) => sum + c.spent_missing_fx, 0);

  function invalidate() {
    queryClient.invalidateQueries({
      queryKey: ["general-transaction-categories", workspaceId],
    });
  }

  const createMutation = useMutation<GeneralTransactionCategory, AppError, CreateCategoryRequest>({
    mutationFn: (request) => invoke("create_general_transaction_category", { request }),
    onSuccess: () => {
      invalidate();
      setNome("");
      setLimiteMensal("");
    },
  });

  const updateMutation = useMutation<GeneralTransactionCategory, AppError, UpdateCategoryRequest>({
    mutationFn: (request) => invoke("update_general_transaction_category", { request }),
    onSuccess: () => {
      invalidate();
      setEditingId(null);
    },
  });

  const deleteMutation = useMutation<void, AppError, number>({
    mutationFn: (generalTransactionCategoryId) =>
      invoke("delete_general_transaction_category", { generalTransactionCategoryId }),
    onSuccess: () => {
      invalidate();
      setConfirmingDeleteId(null);
    },
  });

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    createMutation.mutate({
      workspace_id: workspaceId,
      nome,
      limite_mensal: limiteMensal !== "" ? Number(limiteMensal) : null,
    });
  }

  function handleDeleteClick(id: number) {
    if (confirmingDeleteId === id) {
      deleteMutation.mutate(id);
    } else {
      setConfirmingDeleteId(id);
    }
  }

  function startEditing(category: GeneralTransactionCategory) {
    setEditingId(category.id);
    setEditNome(category.nome);
    setEditLimiteMensal(category.limite_mensal !== null ? String(category.limite_mensal) : "");
  }

  function handleSaveEdit() {
    if (editingId === null) return;
    updateMutation.mutate({
      general_transaction_category_id: editingId,
      nome: editNome,
      limite_mensal: editLimiteMensal !== "" ? Number(editLimiteMensal) : null,
    });
  }

  const categories = categoriesQuery.data ?? [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Manage categories</DialogTitle>
        </DialogHeader>

        <p className="text-sm text-muted-foreground">
          Cadastradas uma vez, escolhidas na hora de lançar receita/despesa. O limite mensal é
          opcional e recorrente (mesmo valor todo mês) — comparado contra a soma de despesas da
          categoria no mês atual.
        </p>
        {baseCurrency && (
          <p className="text-sm text-muted-foreground" data-testid="category-base-currency">
            Limits and spending are in {baseCurrency}. Expenses from accounts in other currencies are
            converted at the rate of their day.
            {missingFx > 0 &&
              ` ${missingFx} expense${missingFx === 1 ? " is" : "s are"} left out for lack of an exchange rate.`}
          </p>
        )}

        <form onSubmit={handleSubmit} className="flex items-end gap-4">
          <Field label="Name" className="flex-1">
            <Input
              required
              placeholder="e.g.: Rent, Salary"
              value={nome}
              onChange={(e) => setNome(e.currentTarget.value)}
            />
          </Field>
          <Field label="Monthly limit (optional)">
            <Input
              type="number"
              step="any"
              placeholder="No limit"
              value={limiteMensal}
              onChange={(e) => setLimiteMensal(e.currentTarget.value)}
            />
          </Field>
          <Button type="submit" disabled={createMutation.isPending} className="w-fit">
            {createMutation.isPending ? "Adding..." : "Add"}
          </Button>
        </form>

        {createMutation.isError && (
          <p className="text-red-600">{createMutation.error.message}</p>
        )}
        {updateMutation.isError && (
          <p className="text-red-600">{updateMutation.error.message}</p>
        )}
        {categoriesQuery.isError && (
          <p className="text-red-600">{categoriesQuery.error.message}</p>
        )}

        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Limit</TableHead>
              <TableHead>Spent this month</TableHead>
              <TableHead>Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {categories.length === 0 && (
              <TableRow>
                <TableCell colSpan={4} className="text-center text-muted-foreground">
                  No category registered yet.
                </TableCell>
              </TableRow>
            )}
            {categories.map((category) => {
              const isConfirming = confirmingDeleteId === category.id;
              const isEditing = editingId === category.id;
              const isOverBudget =
                category.limite_mensal !== null && category.spent_this_month > category.limite_mensal;

              if (isEditing) {
                return (
                  <TableRow key={category.id}>
                    <TableCell>
                      <Input value={editNome} onChange={(e) => setEditNome(e.currentTarget.value)} />
                    </TableCell>
                    <TableCell>
                      <Input
                        type="number"
                        step="any"
                        placeholder="No limit"
                        value={editLimiteMensal}
                        onChange={(e) => setEditLimiteMensal(e.currentTarget.value)}
                      />
                    </TableCell>
                    <TableCell>{category.spent_this_month.toFixed(2)}</TableCell>
                    <TableCell className="flex gap-2">
                      <Button size="sm" onClick={handleSaveEdit} disabled={updateMutation.isPending}>
                        Save
                      </Button>
                      <Button size="sm" variant="outline" onClick={() => setEditingId(null)}>
                        Cancel
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              }

              return (
                <TableRow key={category.id}>
                  <TableCell>{category.nome}</TableCell>
                  <TableCell>
                    {category.limite_mensal !== null ? category.limite_mensal.toFixed(2) : "—"}
                  </TableCell>
                  <TableCell className={isOverBudget ? "text-red-600 font-medium" : undefined}>
                    {category.spent_this_month.toFixed(2)}
                  </TableCell>
                  <TableCell className="flex gap-2">
                    <Button size="sm" variant="outline" onClick={() => startEditing(category)}>
                      Edit
                    </Button>
                    <Button
                      variant={isConfirming ? "destructive" : "outline"}
                      size="sm"
                      onClick={() => handleDeleteClick(category.id)}
                    >
                      {isConfirming ? "Confirm?" : "Delete"}
                    </Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </DialogContent>
    </Dialog>
  );
}

export default CategoryManagerDialog;
