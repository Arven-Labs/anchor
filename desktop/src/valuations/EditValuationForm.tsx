import { useEffect, useMemo, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError, ValuationModel } from "../types";
import { INPUT_FIELDS, toEditableString, fromEditableString } from "./inputFields";
import Field from "../components/Field";
import DividendGrowthHint from "../models/DividendGrowthHint";
import RiskFreeHint from "../models/RiskFreeHint";
import TrailingDividendHint from "../models/TrailingDividendHint";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

const MODEL_LABELS: Record<string, string> = {
  bazin: "Bazin",
  graham: "Graham",
  gordon: "Gordon / DDM",
  dcf: "DCF / FCFF",
  banks: "Banks (P/B)",
  rim: "RIM (Bancos)",
  rnav: "RNAV",
  projected_ceiling: "Projected Ceiling",
};

// Generic edit form — one component for all 7 models, driven by
// INPUT_FIELDS (same metadata the "Assumptions" view uses), instead of
// duplicating the 7 calculator forms with an "edit mode". Updates the same
// row in place (update_valuation) rather than inserting a new one.
function EditValuationForm({
  valuation,
  onDone,
  onCancel,
}: {
  valuation: ValuationModel;
  onDone: () => void;
  onCancel: () => void;
}) {
  const queryClient = useQueryClient();
  const fields = useMemo(() => INPUT_FIELDS[valuation.model] ?? [], [valuation.model]);

  const [ticker, setTicker] = useState(valuation.ticker);
  const [referenceYear, setReferenceYear] = useState(String(valuation.reference_year));
  const [currentPrice, setCurrentPrice] = useState(String(valuation.current_price));
  const [drafts, setDrafts] = useState<Record<string, string>>({});

  const inputsQuery = useQuery<Record<string, unknown>, AppError>({
    queryKey: ["valuation-inputs", valuation.id],
    queryFn: () =>
      invoke("get_valuation_inputs", {
        valuationId: valuation.id,
        model: valuation.model,
      }),
  });

  useEffect(() => {
    if (!inputsQuery.data) return;
    const next: Record<string, string> = {};
    for (const field of fields) {
      next[field.key] = toEditableString(inputsQuery.data[field.key], field.format);
    }
    setDrafts(next);
  }, [inputsQuery.data, fields]);

  const updateMutation = useMutation<ValuationModel, AppError, void>({
    mutationFn: () => {
      const inputs: Record<string, number> = {};
      for (const field of fields) {
        inputs[field.key] = fromEditableString(drafts[field.key] ?? "0", field.format);
      }
      return invoke("update_valuation", {
        request: {
          valuation_id: valuation.id,
          ticker: ticker.toUpperCase(),
          reference_year: Number(referenceYear),
          current_price: Number(currentPrice),
          model: valuation.model,
          inputs,
        },
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["valuations"] });
      queryClient.invalidateQueries({ queryKey: ["valuation-inputs", valuation.id] });
      onDone();
    },
  });

  function setDraft(key: string, value: number | string) {
    setDrafts((current) => ({ ...current, [key]: String(value) }));
  }

  // Os hints se penduram na chave do campo (este formulário não conhece o modelo por dentro).
  // g ≥ Ke só avisa no Gordon, que diverge nesse caso; o Teto Projetado não.
  function hintFor(key: string) {
    if (key === "current_dividend") {
      return <TrailingDividendHint ticker={ticker} onUse={(d) => setDraft(key, d)} />;
    }
    if (key === "expected_growth") {
      return (
        <DividendGrowthHint
          ticker={ticker}
          ke={valuation.model === "gordon" ? drafts.ke : undefined}
          onUse={(g) => setDraft(key, g)}
        />
      );
    }
    if (key === "risk_free_rate") return <RiskFreeHint onUse={(rf) => setDraft(key, rf)} />;
    if (key === "ke") return <RiskFreeHint ke={drafts.ke} />;
    return null;
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    updateMutation.mutate();
  }

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-3">
          <Button variant="outline" size="sm" onClick={onCancel}>
            Cancel
          </Button>
          <CardTitle>
            Edit {valuation.ticker} — {MODEL_LABELS[valuation.model] ?? valuation.model}
          </CardTitle>
        </div>
      </CardHeader>
      <CardContent>
        {inputsQuery.isLoading && (
          <p className="text-muted-foreground">Loading assumptions...</p>
        )}
        {inputsQuery.isError && (
          <p className="text-red-600">{inputsQuery.error.message}</p>
        )}

        {inputsQuery.data && (
          <form
            onSubmit={handleSubmit}
            className="grid grid-cols-1 gap-4 sm:grid-cols-2"
          >
            <Field label="Ticker">
              <Input
                required
                value={ticker}
                onChange={(e) => setTicker(e.currentTarget.value)}
              />
            </Field>

            <Field label="Reference year">
              <Input
                required
                type="number"
                value={referenceYear}
                onChange={(e) => setReferenceYear(e.currentTarget.value)}
              />
            </Field>

            <Field label="Current price (R$)">
              <Input
                required
                type="number"
                step="0.01"
                value={currentPrice}
                onChange={(e) => setCurrentPrice(e.currentTarget.value)}
              />
            </Field>

            {fields.map((field) => (
              <Field key={field.key} label={field.label}>
                <Input
                  required
                  type="number"
                  step="any"
                  value={drafts[field.key] ?? ""}
                  onChange={(e) => {
                    // Lê o valor antes: dentro do atualizador `currentTarget` já pode ser null.
                    const value = e.currentTarget.value;
                    setDraft(field.key, value);
                  }}
                />
                {hintFor(field.key)}
              </Field>
            ))}

            <Button
              type="submit"
              disabled={updateMutation.isPending}
              className="sm:col-span-2"
            >
              {updateMutation.isPending ? "Saving..." : "Save changes"}
            </Button>

            {updateMutation.isError && (
              <p className="text-red-600 sm:col-span-2">
                {updateMutation.error.message}
              </p>
            )}
          </form>
        )}
      </CardContent>
    </Card>
  );
}

export default EditValuationForm;
