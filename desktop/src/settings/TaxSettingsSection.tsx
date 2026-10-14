import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

// Fase 24 — parâmetros das regras de IR. Os VALORES são editáveis (pedido do
// dono do projeto: o app não pode depender de alguém atualizar o código
// quando a lei muda); a lógica do cálculo não. Cada linha vale a partir de
// `effective_from` — meses antigos continuam usando a regra da época.

type TaxRule = {
  id: number;
  rule_key: string;
  effective_from: string;
  value: number;
  default_value: number;
  description: string;
  source: string;
  verified_at: string;
  user_edited: boolean;
  updated_at: string;
};

type TaxRuleConflict = {
  id: number;
  rule_key: string;
  effective_from: string;
  your_value: number;
  lume_value: number;
};

type TaxRuleSyncResult = {
  added: number;
  updated: number;
  conflicts: TaxRuleConflict[];
};

type Unit = "percent" | "money" | "code";

const RULE_META: Record<string, { label: string; unit: Unit }> = {
  common_rate: { label: "Regular trades rate", unit: "percent" },
  day_trade_rate: { label: "Day trade rate", unit: "percent" },
  fii_rate: { label: "Real estate funds (FII) rate", unit: "percent" },
  stock_exemption_monthly_sales: { label: "Stock sales exemption (per month)", unit: "money" },
  irrf_common_rate: { label: "Withholding — regular trades", unit: "percent" },
  irrf_day_trade_rate: { label: "Withholding — day trade", unit: "percent" },
  darf_minimum: { label: "Minimum DARF amount", unit: "money" },
  darf_code_b3: { label: "DARF code (stock exchange)", unit: "code" },
  jcp_irrf_rate: { label: "Interest on equity (JCP) withholding", unit: "percent" },
  crypto_exemption_monthly_sales: { label: "Crypto sales exemption (per month)", unit: "money" },
};

function unitOf(key: string): Unit {
  return RULE_META[key]?.unit ?? "money";
}

// Banco guarda fração (0.15); a tela mostra/edita em % (15).
function toDisplay(key: string, value: number): string {
  const unit = unitOf(key);
  if (unit === "percent") return String(Number((value * 100).toFixed(6)));
  if (unit === "code") return String(Math.round(value));
  return String(value);
}

function fromDisplay(key: string, text: string): number {
  const n = Number(text.replace(",", "."));
  return unitOf(key) === "percent" ? n / 100 : n;
}

function formatValue(key: string, value: number): string {
  const unit = unitOf(key);
  if (unit === "percent") return `${Number((value * 100).toFixed(6))}%`;
  if (unit === "code") return String(Math.round(value));
  return value.toLocaleString("pt-BR", { style: "currency", currency: "BRL" });
}

function formatDate(date: string): string {
  const [y, m, d] = date.slice(0, 10).split("-");
  return `${d}/${m}/${y}`;
}

function TaxSettingsSection() {
  const queryClient = useQueryClient();
  const [editingId, setEditingId] = useState<number | null>(null);
  const [draft, setDraft] = useState("");

  const rulesQuery = useQuery<TaxRule[], AppError>({
    queryKey: ["tax-rules"],
    queryFn: () => invoke("list_tax_rules"),
  });

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: ["tax-rules"] });
    queryClient.invalidateQueries({ queryKey: ["tax-assessment"] });
  }

  const updateMutation = useMutation<TaxRule, AppError, { id: number; value: number }>({
    mutationFn: ({ id, value }) => invoke("update_tax_rule", { id, value }),
    onSuccess: () => {
      invalidate();
      setEditingId(null);
    },
  });

  const resetMutation = useMutation<TaxRule, AppError, number>({
    mutationFn: (id) => invoke("reset_tax_rule", { id }),
    onSuccess: invalidate,
  });

  const syncMutation = useMutation<TaxRuleSyncResult, AppError, void>({
    mutationFn: () => invoke("sync_tax_rules"),
    onSuccess: invalidate,
  });

  const rules = rulesQuery.data ?? [];
  const conflictIds = new Set((syncMutation.data?.conflicts ?? []).map((c) => c.id));

  function startEdit(rule: TaxRule) {
    setEditingId(rule.id);
    setDraft(toDisplay(rule.rule_key, rule.value));
  }

  function saveEdit(rule: TaxRule) {
    const value = fromDisplay(rule.rule_key, draft);
    if (!Number.isFinite(value) || value < 0) return;
    updateMutation.mutate({ id: rule.id, value });
  }

  return (
    <div className="flex flex-col gap-4">
      <div>
        <h3 className="text-sm font-semibold text-muted-foreground">Tax rules</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Rates and limits used by the Taxes tab. They come up to date from Lume; if the law
          changes before an update arrives, edit the value here — your edits are never
          overwritten. Each row applies from its start date, so past months keep the rule that was
          valid then.
        </p>
      </div>

      <div className="flex items-center gap-3">
        <Button
          variant="outline"
          onClick={() => syncMutation.mutate()}
          disabled={syncMutation.isPending}
        >
          {syncMutation.isPending ? "Checking…" : "Check for updates"}
        </Button>
        {syncMutation.isSuccess && (
          <p className="text-sm text-muted-foreground">
            {syncMutation.data.added + syncMutation.data.updated === 0
              ? "Already up to date."
              : `${syncMutation.data.added} new, ${syncMutation.data.updated} updated.`}
            {syncMutation.data.conflicts.length > 0 &&
              ` ${syncMutation.data.conflicts.length} of your edits differ from the new official value — use "Reset" to apply it.`}
          </p>
        )}
        {syncMutation.isError && (
          <p className="text-sm text-red-600">
            Couldn't reach Lume ({syncMutation.error.message}). The current values keep working.
          </p>
        )}
      </div>

      {rulesQuery.isError && <p className="text-red-600">{rulesQuery.error.message}</p>}
      {(updateMutation.isError || resetMutation.isError) && (
        <p className="text-sm text-red-600">
          {(updateMutation.error ?? resetMutation.error)?.message}
        </p>
      )}

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Rule</TableHead>
            <TableHead>From</TableHead>
            <TableHead>Value</TableHead>
            <TableHead>Source</TableHead>
            <TableHead />
          </TableRow>
        </TableHeader>
        <TableBody>
          {rules.map((rule) => (
            <TableRow key={rule.id}>
              <TableCell className="max-w-xs">
                <p className="font-medium">{RULE_META[rule.rule_key]?.label ?? rule.rule_key}</p>
                <p className="text-xs whitespace-normal text-muted-foreground">
                  {rule.description}
                </p>
              </TableCell>
              <TableCell>{formatDate(rule.effective_from)}</TableCell>
              <TableCell>
                {editingId === rule.id ? (
                  <div className="flex items-center gap-1">
                    <Input
                      autoFocus
                      className="h-8 w-28"
                      value={draft}
                      onChange={(e) => setDraft(e.currentTarget.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") saveEdit(rule);
                        if (e.key === "Escape") setEditingId(null);
                      }}
                    />
                    <span className="text-sm text-muted-foreground">
                      {unitOf(rule.rule_key) === "percent" ? "%" : ""}
                    </span>
                  </div>
                ) : (
                  <div className="flex flex-col gap-1">
                    <span>{formatValue(rule.rule_key, rule.value)}</span>
                    {rule.user_edited && (
                      <Badge variant="secondary" className="w-fit">
                        edited by you · official {formatValue(rule.rule_key, rule.default_value)}
                      </Badge>
                    )}
                    {conflictIds.has(rule.id) && (
                      <Badge variant="destructive" className="w-fit">
                        official value changed
                      </Badge>
                    )}
                  </div>
                )}
              </TableCell>
              <TableCell className="max-w-xs text-xs whitespace-normal">
                {rule.source.startsWith("http") ? (
                  <button
                    type="button"
                    className="text-left text-primary underline-offset-4 hover:underline"
                    onClick={() => openUrl(rule.source)}
                  >
                    Receita Federal
                  </button>
                ) : (
                  rule.source
                )}
                <span className="block text-muted-foreground">
                  checked {formatDate(rule.verified_at)}
                </span>
              </TableCell>
              <TableCell className="text-right">
                {editingId === rule.id ? (
                  <div className="flex justify-end gap-1">
                    <Button size="sm" onClick={() => saveEdit(rule)} disabled={updateMutation.isPending}>
                      Save
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => setEditingId(null)}>
                      Cancel
                    </Button>
                  </div>
                ) : (
                  <div className="flex justify-end gap-1">
                    <Button size="sm" variant="outline" onClick={() => startEdit(rule)}>
                      Edit
                    </Button>
                    {rule.user_edited && (
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => resetMutation.mutate(rule.id)}
                        disabled={resetMutation.isPending}
                      >
                        Reset
                      </Button>
                    )}
                  </div>
                )}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

export default TaxSettingsSection;
