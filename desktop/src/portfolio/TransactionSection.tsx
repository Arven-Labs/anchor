import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import {
  FIXED_INCOME_CLASSES,
  FI_INDEXADORES,
  FI_LIQUIDEZ_OPTIONS,
  PAYMENT_TYPES,
  PAYMENT_TYPE_LABELS,
  TRANSACTION_TYPES,
  TRANSACTION_TYPE_LABELS,
  type Asset,
  type Custodia,
  type PaymentType,
  type TransactionType,
  type TransactionView,
} from "./types";
import Field from "../components/Field";
import CryptoSwapCard from "./CryptoSwapCard";
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
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

type CreateTransactionRequest = {
  portfolio_id: number;
  asset_id: number | null;
  custodia_id: number | null;
  transfer_to_custodia_id: number | null;
  transaction_type: string;
  payment_type: string | null;
  quantity: number | null;
  unit_price: number | null;
  total_value: number;
  fee: number | null;
  foreign_tax_withheld: number | null;
  transaction_date: string;
  notes: string | null;
  fi_emissor: string | null;
  fi_indexador: string | null;
  fi_taxa_percentual: number | null;
  fi_data_vencimento: string | null;
  fi_liquidez: string | null;
};

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

function needsAsset(type: TransactionType): boolean {
  return type !== "aporte" && type !== "retirada";
}

function needsQuantity(type: TransactionType): boolean {
  return (
    type === "compra" || type === "venda" || type === "transferencia" || type === "come_cotas" || isCorporateAction(type)
  );
}

// Fase 34 — desdobramento/grupamento (quantidade = razão) e bonificação
// (quantidade = ações novas, valor = custo atribuído a elas).
function isCorporateAction(type: TransactionType): boolean {
  return type === "desdobramento" || type === "grupamento" || type === "bonificacao";
}

function needsTotalValue(type: TransactionType): boolean {
  return type !== "desdobramento" && type !== "grupamento";
}

function quantityLabel(type: TransactionType): string {
  if (type === "desdobramento") return "Ratio (2 = each share becomes 2)";
  if (type === "grupamento") return "Ratio (10 = 10 shares become 1)";
  if (type === "bonificacao") return "New shares";
  if (type === "come_cotas") return "Quotas taken";
  return "Quantity";
}

// Compra, venda e come-cotas: total = quantidade × preço (no come-cotas, o valor da cota; o total é o imposto em reais).
function needsUnitPrice(type: TransactionType): boolean {
  return type === "compra" || type === "venda" || type === "come_cotas";
}

function needsTransferDestination(type: TransactionType): boolean {
  return type === "transferencia";
}

function needsPaymentType(type: TransactionType): boolean {
  return type === "provento";
}

function TransactionSection({
  workspaceId,
  portfolioId,
}: {
  workspaceId: number;
  portfolioId: number;
}) {
  const [transactionType, setTransactionType] = useState<TransactionType>("compra");
  const [paymentType, setPaymentType] = useState<PaymentType>("dividendo");
  const [assetId, setAssetId] = useState("");
  const [custodiaId, setCustodiaId] = useState("");
  const [transferToCustodiaId, setTransferToCustodiaId] = useState("");
  const [quantity, setQuantity] = useState("");
  const [unitPrice, setUnitPrice] = useState("");
  const [totalValue, setTotalValue] = useState("");
  const [fee, setFee] = useState("");
  const [foreignTaxWithheld, setForeignTaxWithheld] = useState("");
  const [transactionDate, setTransactionDate] = useState(todayIso());
  const [notes, setNotes] = useState("");
  const [fiEmissor, setFiEmissor] = useState("");
  const [fiIndexador, setFiIndexador] = useState<string>(FI_INDEXADORES[0]);
  const [fiTaxaPercentual, setFiTaxaPercentual] = useState("");
  const [fiDataVencimento, setFiDataVencimento] = useState("");
  const [fiLiquidez, setFiLiquidez] = useState<string>(FI_LIQUIDEZ_OPTIONS[0]);

  const queryClient = useQueryClient();

  const assetsQuery = useQuery<Asset[], AppError>({
    queryKey: ["assets"],
    queryFn: () => invoke("list_assets"),
  });
  const custodiasQuery = useQuery<Custodia[], AppError>({
    queryKey: ["custodias", workspaceId],
    queryFn: () => invoke("list_custodias", { workspaceId }),
  });
  const assets = assetsQuery.data ?? [];
  const custodias = custodiasQuery.data ?? [];
  const selectedAsset = assets.find((a) => a.id === Number(assetId));
  const showFixedIncomeFields =
    transactionType === "compra" &&
    selectedAsset !== undefined &&
    (FIXED_INCOME_CLASSES as string[]).includes(selectedAsset.asset_class);
  // Imposto retido lá fora só faz sentido num provento de ativo que paga em outra moeda.
  const showForeignWithholding =
    transactionType === "provento" && selectedAsset !== undefined && selectedAsset.currency !== "BRL";

  function invalidateAfterWrite() {
    queryClient.invalidateQueries({ queryKey: ["transactions", portfolioId] });
    queryClient.invalidateQueries({ queryKey: ["positions", portfolioId] });
  }

  const createMutation = useMutation<TransactionView, AppError, CreateTransactionRequest>({
    mutationFn: (request) => invoke("create_transaction", { request }),
    onSuccess: () => {
      invalidateAfterWrite();
      setQuantity("");
      setUnitPrice("");
      setTotalValue("");
      setFee("");
      setForeignTaxWithheld("");
      setNotes("");
      setFiTaxaPercentual("");
      setFiDataVencimento("");
      setPaymentType("dividendo");
    },
  });

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    createMutation.mutate({
      portfolio_id: portfolioId,
      asset_id: needsAsset(transactionType) && assetId !== "" ? Number(assetId) : null,
      custodia_id: custodiaId !== "" ? Number(custodiaId) : null,
      transfer_to_custodia_id:
        needsTransferDestination(transactionType) && transferToCustodiaId !== ""
          ? Number(transferToCustodiaId)
          : null,
      transaction_type: transactionType,
      payment_type: needsPaymentType(transactionType) ? paymentType : null,
      quantity: needsQuantity(transactionType) && quantity !== "" ? Number(quantity) : null,
      unit_price: needsUnitPrice(transactionType) && unitPrice !== "" ? Number(unitPrice) : null,
      total_value: needsTotalValue(transactionType) ? Number(totalValue) : 0,
      fee: transactionType !== "come_cotas" && fee !== "" ? Number(fee) : null,
      foreign_tax_withheld: showForeignWithholding && foreignTaxWithheld !== "" ? Number(foreignTaxWithheld) : null,
      transaction_date: transactionDate,
      notes: notes.trim() === "" ? null : notes,
      fi_emissor: showFixedIncomeFields && fiEmissor.trim() !== "" ? fiEmissor : null,
      fi_indexador: showFixedIncomeFields ? fiIndexador : null,
      fi_taxa_percentual: showFixedIncomeFields && fiTaxaPercentual !== "" ? Number(fiTaxaPercentual) : null,
      fi_data_vencimento: showFixedIncomeFields && fiDataVencimento !== "" ? fiDataVencimento : null,
      fi_liquidez: showFixedIncomeFields ? fiLiquidez : null,
    });
  }

  // Compra/venda: o total acompanha quantidade × preço (continua editável).
  function updateTotal(nextQuantity: string, nextUnitPrice: string) {
    if (!needsUnitPrice(transactionType) || nextQuantity === "" || nextUnitPrice === "") return;
    setTotalValue((Number(nextQuantity) * Number(nextUnitPrice)).toFixed(2));
  }

  const canSubmit =
    (totalValue !== "" || !needsTotalValue(transactionType)) &&
    (!needsAsset(transactionType) || assetId !== "") &&
    (!needsQuantity(transactionType) || quantity !== "") &&
    (!needsUnitPrice(transactionType) || unitPrice !== "") &&
    (!needsTransferDestination(transactionType) ||
      (custodiaId !== "" && transferToCustodiaId !== "" && custodiaId !== transferToCustodiaId));

  return (
    <div className="flex flex-col gap-6">
      <Card>
        <CardHeader>
          <CardTitle>New transaction</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
              <Field label="Type">
                <Select
                  value={transactionType}
                  onValueChange={(value) => setTransactionType(value as TransactionType)}
                >
                  <SelectTrigger className="w-full" data-testid="tx-type">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {TRANSACTION_TYPES.map((key) => (
                      <SelectItem key={key} value={key}>
                        {TRANSACTION_TYPE_LABELS[key]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>

              <Field label="Date">
                <Input
                  required
                  type="date"
                  value={transactionDate}
                  onChange={(e) => setTransactionDate(e.currentTarget.value)}
                />
              </Field>

              {needsAsset(transactionType) && (
                <Field label="Asset">
                  <Select value={assetId} onValueChange={setAssetId}>
                    <SelectTrigger className="w-full" data-testid="tx-asset">
                      <SelectValue placeholder="Select an asset" />
                    </SelectTrigger>
                    <SelectContent>
                      {assets
                        .filter((asset) => transactionType !== "come_cotas" || asset.asset_class === "fundo")
                        .map((asset) => (
                        <SelectItem key={asset.id} value={String(asset.id)}>
                          {asset.ticker} — {asset.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
              )}
            </div>

            <div className="grid grid-cols-1 gap-4 sm:grid-cols-4">
              {needsQuantity(transactionType) && (
                <Field label={quantityLabel(transactionType)}>
                  <Input
                    required
                    type="number"
                    step="any"
                    data-testid="tx-quantity"
                    value={quantity}
                    onChange={(e) => {
                      setQuantity(e.currentTarget.value);
                      updateTotal(e.currentTarget.value, unitPrice);
                    }}
                  />
                </Field>
              )}
              {needsUnitPrice(transactionType) && (
                <Field label={transactionType === "come_cotas" ? "Quota value" : "Unit price"}>
                  <Input
                    required
                    type="number"
                    step="any"
                    data-testid="tx-unit-price"
                    value={unitPrice}
                    onChange={(e) => {
                      setUnitPrice(e.currentTarget.value);
                      updateTotal(quantity, e.currentTarget.value);
                    }}
                  />
                </Field>
              )}
              {needsTotalValue(transactionType) && (
                <Field
                  label={
                    transactionType === "bonificacao"
                      ? "Cost attributed to the new shares"
                      : transactionType === "come_cotas"
                        ? "Tax withheld (BRL)"
                        : "Total value"
                  }
                >
                  <Input
                    required
                    type="number"
                    step="any"
                    value={totalValue}
                    onChange={(e) => setTotalValue(e.currentTarget.value)}
                  />
                </Field>
              )}
              {transactionType !== "come_cotas" && (
                <Field label="Fee (optional)">
                  <Input
                    type="number"
                    step="any"
                    data-testid="tx-fee"
                    value={fee}
                    onChange={(e) => setFee(e.currentTarget.value)}
                  />
                </Field>
              )}
              {showForeignWithholding && (
                <Field label={`Tax withheld abroad (${selectedAsset?.currency}, optional)`}>
                  <Input
                    type="number"
                    step="any"
                    min="0"
                    data-testid="tx-foreign-tax-withheld"
                    value={foreignTaxWithheld}
                    onChange={(e) => setForeignTaxWithheld(e.currentTarget.value)}
                  />
                </Field>
              )}
              {needsPaymentType(transactionType) && (
                <Field label="Payment type">
                  <Select value={paymentType} onValueChange={(v) => setPaymentType(v as PaymentType)}>
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
              )}
            </div>

            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <Field label={needsTransferDestination(transactionType) ? "Custody (source)" : "Custody (optional)"}>
                <Select value={custodiaId} onValueChange={setCustodiaId}>
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="No custody set" />
                  </SelectTrigger>
                  <SelectContent>
                    {custodias.map((custodia) => (
                      <SelectItem key={custodia.id} value={String(custodia.id)}>
                        {custodia.instituicao} ({custodia.titular})
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>

              {needsTransferDestination(transactionType) && (
                <Field label="Custody (destination)">
                  <Select value={transferToCustodiaId} onValueChange={setTransferToCustodiaId}>
                    <SelectTrigger className="w-full">
                      <SelectValue placeholder="Select the destination" />
                    </SelectTrigger>
                    <SelectContent>
                      {custodias.map((custodia) => (
                        <SelectItem key={custodia.id} value={String(custodia.id)}>
                          {custodia.instituicao} ({custodia.titular})
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
              )}
            </div>

            {showFixedIncomeFields && (
              <div className="grid grid-cols-1 gap-4 rounded-md border p-4 sm:grid-cols-3">
                <Field label="Issuer (optional)">
                  <Input value={fiEmissor} onChange={(e) => setFiEmissor(e.currentTarget.value)} />
                </Field>
                <Field label="Index">
                  <Select value={fiIndexador} onValueChange={setFiIndexador}>
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {FI_INDEXADORES.map((key) => (
                        <SelectItem key={key} value={key}>
                          {key}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
                <Field label="Contracted rate (%)">
                  <Input
                    type="number"
                    step="any"
                    value={fiTaxaPercentual}
                    onChange={(e) => setFiTaxaPercentual(e.currentTarget.value)}
                  />
                </Field>
                <Field label="Maturity date">
                  <Input
                    type="date"
                    value={fiDataVencimento}
                    onChange={(e) => setFiDataVencimento(e.currentTarget.value)}
                  />
                </Field>
                <Field label="Liquidity">
                  <Select value={fiLiquidez} onValueChange={setFiLiquidez}>
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {FI_LIQUIDEZ_OPTIONS.map((key) => (
                        <SelectItem key={key} value={key}>
                          {key}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
              </div>
            )}

            <Field label="Notes (optional)">
              <Input value={notes} onChange={(e) => setNotes(e.currentTarget.value)} />
            </Field>

            {createMutation.isError && (
              <p className="text-red-600" data-testid="tx-error">
                {createMutation.error.message}
              </p>
            )}

            <Button
              type="submit"
              data-testid="tx-submit"
              disabled={!canSubmit || createMutation.isPending}
              className="w-fit"
            >
              {createMutation.isPending ? "Logging..." : "Log transaction"}
            </Button>
          </form>
        </CardContent>
      </Card>

      <CryptoSwapCard portfolioId={portfolioId} />
    </div>
  );
}

export default TransactionSection;
