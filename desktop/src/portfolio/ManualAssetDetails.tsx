import { useState, type FormEvent } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { Asset, AssetAttachment, AssetValuation } from "./types";
import Field from "../components/Field";
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

type AddAssetValuationRequest = {
  asset_id: number;
  valuation_date: string;
  value: number;
  notes: string | null;
};
type AddAssetAttachmentRequest = {
  asset_id: number;
  source_path: string;
  document_type: string | null;
};

const ATTACHMENT_EXTENSIONS = ["pdf", "png", "jpg", "jpeg", "gif", "webp", "xlsx", "xls", "csv"];

function formatCurrency(value: number, currency: string): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency });
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function attachmentKind(attachment: AssetAttachment): "image" | "pdf" | "spreadsheet" | "other" {
  const type = attachment.content_type ?? "";
  if (type.startsWith("image/")) return "image";
  if (type === "application/pdf") return "pdf";
  if (
    type === "text/csv" ||
    type === "application/vnd.ms-excel" ||
    type === "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
  ) {
    return "spreadsheet";
  }
  return "other";
}

type UpdateAssetEquityRequest = {
  asset_id: number;
  equity_shares_owned: number | null;
  equity_total_shares: number | null;
  equity_company_valuation: number | null;
};

function formatPct(value: number): string {
  return `${(value * 100).toFixed(2)}%`;
}

// Fase 10, item 8, Sessão 64 — popup ("Property details"/"Company details")
// aberto a partir da tabela de Ativos (`AssetSection.tsx`) pras 2 classes
// `AtivoManual` do rascunho original (Sessão 30): `imovel` e
// `empresa_nao_listada` compartilham o mesmo esqueleto — histórico de
// avaliações (data + valor + notas, `origin` sempre "manual" por ora —
// reajuste automático ainda não decidido, ver PHASE.md item 8) e anexos
// (escritura/ITBI/IPTU pro imóvel, contrato/cap table pra empresa),
// reaproveitando exatamente o mecanismo de preview em disco já validado pra
// Teses (Fase 10.5, `ThesisSection.tsx`): imagem/PDF via `convertFileSrc`,
// planilha lida no backend (`read_asset_attachment_spreadsheet`). `empresa_nao_listada` ganha um bloco extra
// no topo (participação societária) que as outras classes não têm.
function ManualAssetDetails({ asset }: { asset: Asset }) {
  const isEmpresaNaoListada = asset.asset_class === "empresa_nao_listada";

  const [sharesOwned, setSharesOwned] = useState(
    asset.equity_shares_owned !== null ? String(asset.equity_shares_owned) : "",
  );
  const [totalShares, setTotalShares] = useState(
    asset.equity_total_shares !== null ? String(asset.equity_total_shares) : "",
  );
  const [companyValuation, setCompanyValuation] = useState(
    asset.equity_company_valuation !== null ? String(asset.equity_company_valuation) : "",
  );
  const [valuationDate, setValuationDate] = useState("");
  const [valuationValue, setValuationValue] = useState("");
  const [valuationNotes, setValuationNotes] = useState("");
  const [documentType, setDocumentType] = useState("");
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [confirmingDeleteAttachmentId, setConfirmingDeleteAttachmentId] = useState<number | null>(
    null,
  );
  const [viewingAttachmentId, setViewingAttachmentId] = useState<number | null>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [spreadsheetRows, setSpreadsheetRows] = useState<string[][] | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);

  const queryClient = useQueryClient();

  const updateEquityMutation = useMutation<Asset, AppError, UpdateAssetEquityRequest>({
    mutationFn: (request) => invoke("update_asset_equity", { request }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["assets"] });
    },
  });

  function handleSaveEquity(event: FormEvent) {
    event.preventDefault();
    updateEquityMutation.mutate({
      asset_id: asset.id,
      equity_shares_owned: sharesOwned.trim() === "" ? null : Number(sharesOwned),
      equity_total_shares: totalShares.trim() === "" ? null : Number(totalShares),
      equity_company_valuation: companyValuation.trim() === "" ? null : Number(companyValuation),
    });
  }

  const valuationsQuery = useQuery<AssetValuation[], AppError>({
    queryKey: ["asset-valuations", asset.id],
    queryFn: () => invoke("list_asset_valuations", { assetId: asset.id }),
  });
  const valuations = valuationsQuery.data ?? [];

  const addValuationMutation = useMutation<AssetValuation, AppError, AddAssetValuationRequest>({
    mutationFn: (request) => invoke("add_asset_valuation", { request }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["asset-valuations", asset.id] });
      setValuationDate("");
      setValuationValue("");
      setValuationNotes("");
    },
  });

  const deleteValuationMutation = useMutation<void, AppError, number>({
    mutationFn: (valuationId) => invoke("delete_asset_valuation", { valuationId }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["asset-valuations", asset.id] });
    },
  });

  const attachmentsQuery = useQuery<AssetAttachment[], AppError>({
    queryKey: ["asset-attachments", asset.id],
    queryFn: () => invoke("list_asset_attachments", { assetId: asset.id }),
  });
  const attachments = attachmentsQuery.data ?? [];

  const addAttachmentMutation = useMutation<AssetAttachment, AppError, AddAssetAttachmentRequest>({
    mutationFn: (request) => invoke("add_asset_attachment", { request }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["asset-attachments", asset.id] });
    },
  });

  const deleteAttachmentMutation = useMutation<void, AppError, number>({
    mutationFn: (attachmentId) => invoke("delete_asset_attachment", { attachmentId }),
    onSuccess: (_data, attachmentId) => {
      queryClient.invalidateQueries({ queryKey: ["asset-attachments", asset.id] });
      setConfirmingDeleteAttachmentId(null);
      if (viewingAttachmentId === attachmentId) closePreview();
    },
  });

  function closePreview() {
    setViewingAttachmentId(null);
    setPreviewUrl(null);
    setSpreadsheetRows(null);
    setPreviewError(null);
  }

  function handleAddValuation(event: FormEvent) {
    event.preventDefault();
    addValuationMutation.mutate({
      asset_id: asset.id,
      valuation_date: valuationDate,
      value: Number(valuationValue),
      notes: valuationNotes.trim() === "" ? null : valuationNotes.trim(),
    });
  }

  async function handleUploadAttachments() {
    const picked = await open({
      multiple: true,
      filters: [{ name: "Attachments", extensions: ATTACHMENT_EXTENSIONS }],
    });
    if (!picked) return;
    setUploadError(null);
    for (const sourcePath of picked) {
      try {
        await addAttachmentMutation.mutateAsync({
          asset_id: asset.id,
          source_path: sourcePath,
          document_type: documentType.trim() === "" ? null : documentType.trim(),
        });
      } catch (err) {
        setUploadError(err instanceof Error ? err.message : `Error attaching ${sourcePath}.`);
      }
    }
  }

  function handleDeleteAttachmentClick(attachmentId: number) {
    if (confirmingDeleteAttachmentId === attachmentId) {
      deleteAttachmentMutation.mutate(attachmentId);
    } else {
      setConfirmingDeleteAttachmentId(attachmentId);
    }
  }

  async function handleViewAttachment(attachment: AssetAttachment) {
    setViewingAttachmentId(attachment.id);
    setPreviewError(null);
    setSpreadsheetRows(null);
    setPreviewUrl(null);
    try {
      if (attachmentKind(attachment) === "spreadsheet") {
        setSpreadsheetRows(
          await invoke<string[][]>("read_asset_attachment_spreadsheet", {
            attachmentId: attachment.id,
          }),
        );
      } else {
        const path = await invoke<string>("get_asset_attachment_path", {
          attachmentId: attachment.id,
        });
        setPreviewUrl(convertFileSrc(path));
      }
    } catch (err) {
      // O erro do invoke é um `AppError` (objeto com `message`), não um `Error`.
      setPreviewError((err as { message?: string })?.message ?? "Error loading preview.");
    }
  }

  const latestValue = valuations.length > 0 ? valuations[valuations.length - 1].value : null;

  const percentual =
    asset.equity_shares_owned !== null &&
    asset.equity_total_shares !== null &&
    asset.equity_total_shares > 0
      ? asset.equity_shares_owned / asset.equity_total_shares
      : null;
  const participationValue =
    percentual !== null && asset.equity_company_valuation !== null
      ? percentual * asset.equity_company_valuation
      : null;

  return (
    <div className="flex flex-col gap-6 py-2">
      {isEmpresaNaoListada && (
        <div>
          <h3 className="mb-2 text-sm font-semibold">
            Equity stake
            {percentual !== null && (
              <span className="ml-2 font-normal text-muted-foreground">
                ({formatPct(percentual)}
                {participationValue !== null &&
                  ` — ${formatCurrency(participationValue, asset.currency)}`}
                )
              </span>
            )}
          </h3>
          <p className="mb-3 text-sm text-muted-foreground">
            Percentage is always calculated from shares owned / total company shares — it's never
            typed in directly, so it can't drift out of sync with them.
          </p>
          <form onSubmit={handleSaveEquity} className="flex flex-wrap items-end gap-3">
            <Field label="Shares owned">
              <Input
                type="number"
                step="any"
                min="0"
                value={sharesOwned}
                onChange={(e) => setSharesOwned(e.currentTarget.value)}
              />
            </Field>
            <Field label="Total company shares">
              <Input
                type="number"
                step="any"
                min="0"
                value={totalShares}
                onChange={(e) => setTotalShares(e.currentTarget.value)}
              />
            </Field>
            <Field label={`Company valuation (${asset.currency})`}>
              <Input
                type="number"
                step="0.01"
                min="0"
                value={companyValuation}
                onChange={(e) => setCompanyValuation(e.currentTarget.value)}
              />
            </Field>
            <Button type="submit" disabled={updateEquityMutation.isPending}>
              {updateEquityMutation.isPending ? "Saving..." : "Save"}
            </Button>
          </form>
          {updateEquityMutation.isError && (
            <p className="mt-2 text-red-600">{updateEquityMutation.error.message}</p>
          )}
        </div>
      )}

      <div className={isEmpresaNaoListada ? "border-t pt-4" : undefined}>
        <h3 className="mb-2 text-sm font-semibold">
          Valuation history
          {latestValue !== null && (
            <span className="ml-2 font-normal text-muted-foreground">
              (latest: {formatCurrency(latestValue, asset.currency)})
            </span>
          )}
        </h3>

        {valuationsQuery.isError && (
          <p className="mb-3 text-red-600">{valuationsQuery.error.message}</p>
        )}

        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Date</TableHead>
              <TableHead>Value</TableHead>
              <TableHead>Origin</TableHead>
              <TableHead>Notes</TableHead>
              <TableHead></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {valuations.length === 0 && (
              <TableRow>
                <TableCell colSpan={5} className="text-center text-muted-foreground">
                  No valuations registered yet.
                </TableCell>
              </TableRow>
            )}
            {valuations.map((valuation) => (
              <TableRow key={valuation.id}>
                <TableCell>{valuation.valuation_date}</TableCell>
                <TableCell>{formatCurrency(valuation.value, asset.currency)}</TableCell>
                <TableCell>{valuation.origin}</TableCell>
                <TableCell>{valuation.notes ?? "—"}</TableCell>
                <TableCell>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={deleteValuationMutation.isPending}
                    onClick={() => deleteValuationMutation.mutate(valuation.id)}
                  >
                    Delete
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>

        <form onSubmit={handleAddValuation} className="mt-4 flex flex-wrap items-end gap-3">
          <Field label="Date">
            <Input
              required
              type="date"
              value={valuationDate}
              onChange={(e) => setValuationDate(e.currentTarget.value)}
            />
          </Field>
          <Field label={`Value (${asset.currency})`}>
            <Input
              required
              type="number"
              step="0.01"
              min="0"
              value={valuationValue}
              onChange={(e) => setValuationValue(e.currentTarget.value)}
            />
          </Field>
          <Field label="Notes (optional)" className="flex-1 min-w-[12rem]">
            <Input
              placeholder="e.g.: market appraisal, IPTU reference value"
              value={valuationNotes}
              onChange={(e) => setValuationNotes(e.currentTarget.value)}
            />
          </Field>
          <Button type="submit" disabled={addValuationMutation.isPending}>
            {addValuationMutation.isPending ? "Adding..." : "Add valuation"}
          </Button>
        </form>
        {addValuationMutation.isError && (
          <p className="mt-2 text-red-600">{addValuationMutation.error.message}</p>
        )}
      </div>

      <div className="border-t pt-4">
        <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
          <h3 className="text-sm font-semibold">Attachments</h3>
          <div className="flex items-end gap-2">
            <Field label="Document type (optional)">
              <Input
                placeholder="e.g.: deed, ITBI, IPTU"
                value={documentType}
                onChange={(e) => setDocumentType(e.currentTarget.value)}
              />
            </Field>
            <Button type="button" variant="outline" size="sm" onClick={handleUploadAttachments}>
              Add attachment
            </Button>
          </div>
        </div>

        {uploadError && <p className="mb-3 text-red-600">{uploadError}</p>}
        {attachmentsQuery.isError && (
          <p className="mb-3 text-red-600">{attachmentsQuery.error.message}</p>
        )}

        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Type</TableHead>
              <TableHead>Size</TableHead>
              <TableHead>Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {attachments.length === 0 && (
              <TableRow>
                <TableCell colSpan={4} className="text-center text-muted-foreground">
                  No attachments yet.
                </TableCell>
              </TableRow>
            )}
            {attachments.map((attachment) => {
              const isConfirmingDelete = confirmingDeleteAttachmentId === attachment.id;
              return (
                <TableRow key={attachment.id}>
                  <TableCell>{attachment.original_file_name}</TableCell>
                  <TableCell>{attachment.document_type ?? "—"}</TableCell>
                  <TableCell>{formatBytes(attachment.file_size_bytes)}</TableCell>
                  <TableCell className="flex gap-2">
                    <Button size="sm" variant="outline" onClick={() => handleViewAttachment(attachment)}>
                      View
                    </Button>
                    <Button
                      size="sm"
                      variant={isConfirmingDelete ? "destructive" : "outline"}
                      onClick={() => handleDeleteAttachmentClick(attachment.id)}
                    >
                      {isConfirmingDelete ? "Confirm?" : "Delete"}
                    </Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>

        {viewingAttachmentId !== null && (
          <div className="mt-4">
            <div className="mb-2 flex justify-end">
              <Button size="sm" variant="outline" onClick={closePreview}>
                Close preview
              </Button>
            </div>
            {previewError && <p className="text-red-600">{previewError}</p>}
            {!previewError && spreadsheetRows && (
              <div className="max-h-[500px] overflow-auto rounded border">
                <table className="w-full text-sm">
                  <tbody>
                    {spreadsheetRows.map((row, rowIndex) => (
                      <tr key={rowIndex}>
                        {row.map((cell, cellIndex) => (
                          <td key={cellIndex} className="border px-2 py-1">
                            {String(cell ?? "")}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            {!previewError &&
              !spreadsheetRows &&
              previewUrl &&
              (() => {
                const attachment = attachments.find((a) => a.id === viewingAttachmentId);
                const kind = attachment ? attachmentKind(attachment) : "other";
                if (kind === "image") {
                  return (
                    <img
                      src={previewUrl}
                      alt={attachment?.original_file_name ?? "attachment"}
                      className="max-h-[500px] w-full object-contain"
                    />
                  );
                }
                if (kind === "pdf") {
                  return (
                    <iframe
                      src={previewUrl}
                      title={attachment?.original_file_name ?? "attachment"}
                      className="h-[500px] w-full rounded border"
                    />
                  );
                }
                return (
                  <p className="text-muted-foreground">No preview available for this file type.</p>
                );
              })()}
          </div>
        )}
      </div>
    </div>
  );
}

export default ManualAssetDetails;
