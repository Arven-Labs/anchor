import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { Asset, DividendSuggestionView, PaymentType } from "./types";
import { PAYMENT_TYPES, PAYMENT_TYPE_LABELS } from "./types";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import Field from "../components/Field";
import CvmNoticesCard from "./CvmNoticesCard";
import DividendConfirmDialog, {
  formatDividendDate as formatDate,
  formatDividendValue as formatValue,
  invalidateDividendQueries,
} from "./DividendConfirmDialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

// Fase 13.6 — revisão de proventos. Parte "passado" (só histórico Yahoo via
// `stock_dividend_payments`) + **conciliação** (Sessão 77.2): o dono do
// projeto pode cadastrar manualmente um provento esperado (futuro); quando a
// fonte automática trouxer o pagamento, o "Generate suggestions" concilia em
// vez de duplicar — valor ±10% vira `matched` (ainda precisa Confirmar),
// divergente vira `divergent` e ganha uma sugestão automática paralela pra
// comparar. A parte "futuro" automática via CVM (Sessão 94, lia o PDF do
// aviso com IA) saiu na Fase 20 junto com toda a IA do Anchor — sugestões
// `source = "cvm"` antigas continuam no banco e aparecem normalmente.

const SOURCE_LABELS: Record<string, string> = {
  manual: "Manual",
  cvm: "CVM",
  yahoo_finance: "Auto",
};

const STATUS_LABELS: Record<string, string> = {
  pending: "Pending",
  matched: "Matched",
  divergent: "Divergent",
  confirmed: "Confirmed",
  discarded: "Discarded",
};

// Estados em que o dono do projeto ainda pode agir (confirmar/descartar).
const ACTIONABLE_STATUSES = ["pending", "matched", "divergent"];

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

export default function DividendSuggestionsSection({
  portfolioId,
}: {
  portfolioId: number;
}) {
  const [confirmingSuggestion, setConfirmingSuggestion] =
    useState<DividendSuggestionView | null>(null);

  const [expectingOpen, setExpectingOpen] = useState(false);
  const [expectingAssetId, setExpectingAssetId] = useState("");
  const [expectingDate, setExpectingDate] = useState(todayIso());
  const [expectingAmount, setExpectingAmount] = useState("");
  const [expectingQuantity, setExpectingQuantity] = useState("");
  const [expectingComDate, setExpectingComDate] = useState("");
  const [expectingPaymentType, setExpectingPaymentType] = useState<PaymentType>("dividendo");

  const queryClient = useQueryClient();

  const assetsQuery = useQuery<Asset[], AppError>({
    queryKey: ["assets"],
    queryFn: () => invoke("list_assets"),
  });
  const assets = assetsQuery.data ?? [];

  const suggestionsQuery = useQuery<DividendSuggestionView[], AppError>({
    queryKey: ["dividend_suggestions", portfolioId],
    queryFn: () => invoke("list_dividend_suggestions", { portfolioId }),
  });

  const suggestions = suggestionsQuery.data ?? [];

  function invalidate() {
    invalidateDividendQueries(queryClient, portfolioId);
  }

  const generateMutation = useMutation<{ generated: number }, AppError, void>({
    mutationFn: () =>
      invoke("generate_dividend_suggestions", { request: { portfolio_id: portfolioId } }),
    onSuccess: invalidate,
  });

  const createExpectedMutation = useMutation<DividendSuggestionView, AppError, void>({
    mutationFn: () =>
      invoke("create_expected_dividend", {
        request: {
          portfolio_id: portfolioId,
          asset_id: Number(expectingAssetId),
          payment_date: expectingDate,
          amount: Number(expectingAmount),
          quantity: Number(expectingQuantity),
          com_date: expectingComDate === "" ? null : expectingComDate,
          payment_type: expectingPaymentType,
        },
      }),
    onSuccess: () => {
      invalidate();
      setExpectingOpen(false);
      setExpectingAssetId("");
      setExpectingAmount("");
      setExpectingQuantity("");
      setExpectingComDate("");
      setExpectingPaymentType("dividendo");
    },
  });

  const discardMutation = useMutation<void, AppError, number>({
    mutationFn: (suggestionId) =>
      invoke("discard_dividend_suggestion", { request: { suggestion_id: suggestionId } }),
    onSuccess: invalidate,
  });

  function handleCreateExpectedSubmit(event: FormEvent) {
    event.preventDefault();
    createExpectedMutation.mutate();
  }

  function resetExpectingDialog() {
    setExpectingOpen(false);
    setExpectingAssetId("");
    setExpectingAmount("");
    setExpectingQuantity("");
    setExpectingComDate("");
    setExpectingPaymentType("dividendo");
  }

  const pendingCount = suggestions.filter((s) => s.status === "pending").length;
  const matchedCount = suggestions.filter((s) => s.status === "matched").length;
  const divergentCount = suggestions.filter((s) => s.status === "divergent").length;
  const confirmedCount = suggestions.filter((s) => s.status === "confirmed").length;
  const discardedCount = suggestions.filter((s) => s.status === "discarded").length;

  const canCreateExpected =
    expectingAssetId !== "" &&
    expectingDate !== "" &&
    Number(expectingAmount) > 0 &&
    Number(expectingQuantity) > 0;

  return (
    <div className="flex flex-col gap-6">
      <CvmNoticesCard portfolioId={portfolioId} />
      <Card>
        <CardHeader>
          <CardTitle>Dividend suggestions</CardTitle>
          <CardDescription>
            Register a future dividend you expect, and past dividends reconstructed from your
            holdings. Automatically-collected dividends reconcile with your expected ones instead
            of creating duplicates — you still confirm each one before it becomes a real
            transaction.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="flex gap-2">
            <Button
              onClick={() => generateMutation.mutate()}
              disabled={generateMutation.isPending}
            >
              {generateMutation.isPending
                ? "Generating..."
                : "Generate suggestions"}
            </Button>
            <Button variant="outline" onClick={() => setExpectingOpen(true)}>
              Register future dividend
            </Button>
          </div>
          {(generateMutation.isError || createExpectedMutation.isError) && (
            <p className="text-sm text-red-600">
              {(
                generateMutation.error ??
                createExpectedMutation.error ??
                new Error("failed")
              ).message}
            </p>
          )}

          <div className="flex flex-wrap gap-6 text-sm text-muted-foreground">
            <span>{pendingCount} pending</span>
            <span>{matchedCount} matched</span>
            <span>{divergentCount} divergent</span>
            <span>{confirmedCount} confirmed</span>
            <span>{discardedCount} discarded</span>
          </div>

          {suggestionsQuery.isLoading && <p className="text-sm">Loading…</p>}
          {suggestionsQuery.isError && (
            <p className="text-sm text-red-600">{suggestionsQuery.error.message}</p>
          )}
        </CardContent>
      </Card>

      {!suggestionsQuery.isLoading && suggestions.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Suggestions</CardTitle>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Ticker</TableHead>
                  <TableHead>Payment date</TableHead>
                  <TableHead>Amount/share</TableHead>
                  <TableHead>Quantity</TableHead>
                  <TableHead>Total</TableHead>
                  <TableHead>Data Com</TableHead>
                  <TableHead>Payment type</TableHead>
                  <TableHead>Source</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {suggestions.map((suggestion) => (
                  <TableRow key={suggestion.id}>
                    <TableCell className="font-medium">{suggestion.ticker}</TableCell>
                    <TableCell>{formatDate(suggestion.payment_date)}</TableCell>
                    <TableCell>{formatValue(suggestion.amount)}</TableCell>
                    <TableCell>{formatValue(suggestion.quantity)}</TableCell>
                    <TableCell>{suggestion.total.toFixed(2)}</TableCell>
                    <TableCell>
                      {suggestion.com_date ? formatDate(suggestion.com_date) : "—"}
                    </TableCell>
                    <TableCell>
                      {PAYMENT_TYPE_LABELS[suggestion.payment_type as PaymentType] ?? suggestion.payment_type}
                    </TableCell>
                    <TableCell>{SOURCE_LABELS[suggestion.source] ?? "Auto"}</TableCell>
                    <TableCell>{STATUS_LABELS[suggestion.status] ?? suggestion.status}</TableCell>
                    <TableCell className="text-right">
                      {ACTIONABLE_STATUSES.includes(suggestion.status) && (
                        <div className="flex justify-end gap-2">
                          <Button
                            size="sm"
                            onClick={() => setConfirmingSuggestion(suggestion)}
                          >
                            Confirm
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => discardMutation.mutate(suggestion.id)}
                          >
                            Discard
                          </Button>
                        </div>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      {!suggestionsQuery.isLoading && suggestions.length === 0 && (
        <p className="text-sm text-muted-foreground">
          No dividend suggestions yet — register the ones you expect or hit "Generate suggestions"
          to build them from your past positions.
        </p>
      )}

      <DividendConfirmDialog
        portfolioId={portfolioId}
        suggestion={confirmingSuggestion}
        onClose={() => setConfirmingSuggestion(null)}
      />

      <Dialog open={expectingOpen} onOpenChange={(open) => {
        if (!open) {
          resetExpectingDialog();
        }
      }}>
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Register future dividend</DialogTitle>
          </DialogHeader>
          <form onSubmit={handleCreateExpectedSubmit} className="flex flex-col gap-4">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <Field label="Asset">
                <Select value={expectingAssetId} onValueChange={setExpectingAssetId}>
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="Select an asset" />
                  </SelectTrigger>
                  <SelectContent>
                    {assets.map((asset) => (
                      <SelectItem key={asset.id} value={String(asset.id)}>
                        {asset.ticker} — {asset.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              <Field label="Payment date">
                <Input
                  type="date"
                  value={expectingDate}
                  onChange={(e) => setExpectingDate(e.target.value)}
                  required
                />
              </Field>
              <Field label="Expected amount / share">
                <Input
                  type="number"
                  step="any"
                  min="0"
                  value={expectingAmount}
                  onChange={(e) => setExpectingAmount(e.target.value)}
                  required
                />
              </Field>
              <Field label="Quantity">
                <Input
                  type="number"
                  step="any"
                  min="0"
                  value={expectingQuantity}
                  onChange={(e) => setExpectingQuantity(e.target.value)}
                  required
                />
              </Field>
              <Field label={`Data Com (com record date) — optional`}>
                <Input
                  type="date"
                  value={expectingComDate}
                  onChange={(e) => setExpectingComDate(e.target.value)}
                />
              </Field>
              <Field label="Payment type">
                <Select
                  value={expectingPaymentType}
                  onValueChange={(v) => setExpectingPaymentType(v as PaymentType)}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {PAYMENT_TYPES.map((key) => (
                      <SelectItem key={key} value={key}>
                        {PAYMENT_TYPE_LABELS[key]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              <Field label="Expected total">
                <div className="pt-2 text-sm">
                  {(Number(expectingAmount || 0) * Number(expectingQuantity || 0)).toFixed(2)}
                </div>
              </Field>
            </div>
            {createExpectedMutation.isError && (
              <p className="text-sm text-red-600">{createExpectedMutation.error.message}</p>
            )}
            <div className="flex justify-end gap-2">
              <Button type="button" variant="outline" onClick={resetExpectingDialog}>
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={createExpectedMutation.isPending || !canCreateExpected}
              >
                {createExpectedMutation.isPending ? "Registering…" : "Register"}
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}