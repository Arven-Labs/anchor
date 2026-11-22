import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

// Previdência privada, declaração: custo e saldo do plano em 31/12 de um ano (o que o informe de Bens e Direitos pede) e a
// ficha (grupo e código). Informativo: nada entra na apuração de IR, no patrimônio nem na Rentabilidade. O grupo e o código
// são digitados pelo dono, copiando do programa da Receita: o app só completa zeros à esquerda e não confere se o par existe,
// nem afirma qual vale para PGBL ou VGBL (a fonte não foi confirmada).
type PensionDeclarationView = {
  asset_id: number;
  group: string | null;
  code: string | null;
  year: number;
  cost: number;
  balance: number;
  balance_source: "statement" | "cost" | "not_held";
  balance_date: string | null;
};

function formatMoney(value: number, currency: string): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency });
}

function balanceNote(view: PensionDeclarationView): string {
  if (view.balance_source === "statement") return `statement of ${view.balance_date}`;
  return "no statement up to that date: showing the cost";
}

// Os dois campos da ficha, com o que o dono já gravou. Montado só depois dos dados chegarem (e de novo quando o valor gravado
// muda, pela `key`), para mostrar o que ficou no banco, já com os zeros à esquerda, e não o que foi digitado.
function FichaFields({
  assetId,
  initialGroup,
  initialCode,
  onSaved,
}: {
  assetId: number;
  initialGroup: string | null;
  initialCode: string | null;
  onSaved: () => void;
}) {
  const [group, setGroup] = useState(initialGroup ?? "");
  const [code, setCode] = useState(initialCode ?? "");
  const save = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("update_asset_dirpf", { request: { asset_id: assetId, group, code, country: null } }),
    onSuccess: onSaved,
  });
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-end gap-3">
        <Field label="Group" className="w-28">
          <Input
            data-testid="pension-dirpf-group"
            value={group}
            onChange={(e) => setGroup(e.currentTarget.value)}
          />
        </Field>
        <Field label="Code" className="w-28">
          <Input
            data-testid="pension-dirpf-code"
            value={code}
            onChange={(e) => setCode(e.currentTarget.value)}
          />
        </Field>
        <Button
          type="button"
          variant="outline"
          disabled={save.isPending}
          data-testid="pension-dirpf-save"
          onClick={() => save.mutate()}
        >
          Save
        </Button>
      </div>
      {save.isError && <p className="text-sm text-destructive">{save.error.message}</p>}
    </div>
  );
}

export default function PensionDeclaration({
  workspaceId,
  assetId,
  currency,
}: {
  workspaceId: number;
  assetId: number;
  currency: string;
}) {
  const queryClient = useQueryClient();
  // A declaração de um ano fala do 31/12 do ano anterior ao da entrega.
  const [year, setYear] = useState(String(new Date().getFullYear() - 1));
  const yearNumber = Number(year);
  const validYear = Number.isInteger(yearNumber) && yearNumber >= 1990 && yearNumber <= 2200;
  const queryKey = ["pension-declaration", workspaceId, assetId, yearNumber];

  const declaration = useQuery<PensionDeclarationView, AppError>({
    queryKey,
    enabled: validYear,
    queryFn: () => invoke("pension_declaration", { workspaceId, assetId, year: yearNumber }),
  });
  const view = declaration.data;
  const fichaText = [view?.group ? `Group ${view.group}` : null, view?.code ? `Code ${view.code}` : null]
    .filter(Boolean)
    .join(" · ");

  return (
    <div data-testid="pension-declaration">
      <h3 className="mb-2 text-sm font-semibold">Annual return (Bens e Direitos)</h3>
      <p className="mb-3 text-sm text-muted-foreground">
        Estimate only: the cost and the balance of this plan on 31/12, as the app tracks them (each real
        contributed is a share worth 1; the balance is the last statement you recorded). Check them against
        the insurer&apos;s statement. Type the group and code as the Receita&apos;s program shows them: the
        app does not check them or say which one applies to a PGBL or a VGBL.
      </p>
      <div className="flex flex-col gap-4">
        <Field label="Year (31/12 of)" className="w-32">
          <Input
            data-testid="pension-declaration-year"
            type="number"
            min="1990"
            max="2200"
            value={year}
            onChange={(e) => setYear(e.currentTarget.value)}
          />
        </Field>
        {declaration.isError && <p className="text-sm text-destructive">{declaration.error.message}</p>}
        {view && (
          <div className="flex flex-col gap-1 text-sm" data-testid="pension-declaration-position">
            {view.balance_source === "not_held" ? (
              <p>Not held on 31/12/{view.year}: no contributions up to that date.</p>
            ) : (
              <>
                <p>
                  Cost on 31/12/{view.year}: <strong>{formatMoney(view.cost, currency)}</strong>
                </p>
                <p>
                  Balance on 31/12/{view.year}: <strong>{formatMoney(view.balance, currency)}</strong> (
                  {balanceNote(view)})
                </p>
              </>
            )}
            <p className="text-muted-foreground" data-testid="pension-declaration-ficha">
              {fichaText || "Group and code not filled in"}
            </p>
          </div>
        )}
        {view && (
          <FichaFields
            key={`${assetId}-${view.group}-${view.code}`}
            assetId={assetId}
            initialGroup={view.group}
            initialCode={view.code}
            onSaved={() => queryClient.invalidateQueries({ queryKey })}
          />
        )}
      </div>
    </div>
  );
}
