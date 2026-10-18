import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";

// Descarte em lote das filas da aba Pending: seleção por linha, "Discard selected" com confirmação em dois
// cliques (mesmo padrão do Delete das contas) e um helper que descarta um por vez com os comandos que já
// existem, sem parar na primeira falha. Usado pelas três seções (lançamentos, extrato e sugestões de provento).

// Caixa de seleção própria: a nativa fica preta sobre o fundo escuro quando desmarcada (sem borda visível).
const CHECKBOX_CLASS =
  "grid size-4 shrink-0 cursor-pointer appearance-none place-content-center rounded-[4px] border border-muted-foreground/70 bg-transparent " +
  "before:hidden before:text-[11px] before:font-bold before:leading-none before:text-primary-foreground before:content-['✓'] " +
  "checked:border-primary checked:bg-primary checked:before:block disabled:cursor-not-allowed disabled:opacity-50";

export type BulkSelection = {
  selected: Set<number>;
  allSelected: boolean;
  toggle: (id: number) => void;
  toggleAll: () => void;
  keepOnly: (ids: number[]) => void;
};

/** Seleção por id. Ids que saíram da fila (confirmados ou descartados) deixam de contar na hora. */
export function useBulkSelection(ids: number[]): BulkSelection {
  const [raw, setRaw] = useState<Set<number>>(new Set());
  const key = ids.join(",");
  const selected = useMemo(() => new Set(ids.filter((id) => raw.has(id))), [key, raw]); // eslint-disable-line react-hooks/exhaustive-deps
  const allSelected = ids.length > 0 && selected.size === ids.length;
  return {
    selected,
    allSelected,
    toggle: (id) =>
      setRaw((prev) => {
        const next = new Set(prev);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      }),
    toggleAll: () => setRaw(allSelected ? new Set() : new Set(ids)),
    keepOnly: (keep) => setRaw(new Set(keep)),
  };
}

export type BulkResult = { failed: number[]; firstError: string | null };

/** Descarta um por vez; as falhas ficam na seleção pra o usuário ver e tentar de novo. */
export async function discardMany(ids: number[], discardOne: (id: number) => Promise<unknown>): Promise<BulkResult> {
  const failed: number[] = [];
  let firstError: string | null = null;
  for (const id of ids) {
    try {
      await discardOne(id);
    } catch (err) {
      failed.push(id);
      firstError ??= err instanceof Error ? err.message : (err as { message?: string })?.message ?? String(err);
    }
  }
  return { failed, firstError };
}

export function bulkErrorMessage(result: BulkResult, total: number): string | null {
  if (result.failed.length === 0) return null;
  return `${result.failed.length} of ${total} item(s) could not be discarded: ${result.firstError ?? "unknown error"}`;
}

export function BulkDiscardBar({
  testIdPrefix,
  selection,
  total,
  busy,
  onDiscard,
}: {
  testIdPrefix: string;
  selection: BulkSelection;
  total: number;
  busy: boolean;
  onDiscard: () => void;
}) {
  const count = selection.selected.size;
  const [confirmingCount, setConfirmingCount] = useState<number | null>(null);
  const confirming = count > 0 && confirmingCount === count;

  return (
    <div className="flex flex-wrap items-center gap-3">
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          className={CHECKBOX_CLASS}
          data-testid={`${testIdPrefix}-select-all`}
          checked={selection.allSelected}
          disabled={total === 0 || busy}
          onChange={selection.toggleAll}
        />
        Select all
      </label>
      <Button
        type="button"
        size="sm"
        variant={confirming ? "destructive" : "outline"}
        data-testid={`${testIdPrefix}-discard-selected`}
        disabled={count === 0 || busy}
        onClick={() => {
          if (confirming) {
            setConfirmingCount(null);
            onDiscard();
          } else {
            setConfirmingCount(count);
          }
        }}
      >
        {confirming ? `Confirm discarding ${count}?` : `Discard selected (${count})`}
      </Button>
    </div>
  );
}

export function RowCheckbox({
  testIdPrefix,
  id,
  selection,
  disabled,
}: {
  testIdPrefix: string;
  id: number;
  selection: BulkSelection;
  disabled?: boolean;
}) {
  return (
    <input
      type="checkbox"
      aria-label="Select item"
      className={CHECKBOX_CLASS}
      data-testid={`${testIdPrefix}-select-${id}`}
      checked={selection.selected.has(id)}
      disabled={disabled}
      onChange={() => selection.toggle(id)}
    />
  );
}
