import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { CvmAssetNotices } from "./types";
import { Button } from "@/components/ui/button";
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

// Sessão 110 — alerta de aviso de provento da CVM ("Relatório Proventos").
// O aviso só traz a data de entrega e o PDF; o valor fica no PDF, então isto
// é só um alerta, sem criar sugestão.
export default function CvmNoticesCard({ portfolioId }: { portfolioId: number }) {
  const noticesQuery = useQuery<CvmAssetNotices[], AppError>({
    queryKey: ["cvm_dividend_notices", portfolioId],
    queryFn: () => invoke("list_cvm_dividend_notices", { portfolioId }),
    staleTime: 60 * 60 * 1000,
  });

  const assets = noticesQuery.data ?? [];
  const rows = assets.flatMap((a) => a.notices.map((n) => ({ ticker: a.ticker, ...n })));
  rows.sort((a, b) => b.delivered_on.localeCompare(a.delivered_on));
  const failed = assets.filter((a) => a.error);

  return (
    <Card>
      <CardHeader>
        <CardTitle>CVM dividend notices</CardTitle>
        <CardDescription>
          Dividend notices published by the CVM in the last 90 days for the Brazilian stocks you
          hold. The notice has the amount and dates in its PDF — add the dividend yourself with
          "Register future dividend".
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {noticesQuery.isLoading && (
          <p className="text-sm text-muted-foreground">Checking the CVM...</p>
        )}
        {noticesQuery.isError && (
          <p className="text-sm text-destructive">{noticesQuery.error.message}</p>
        )}
        {failed.length > 0 && (
          <p className="text-sm text-muted-foreground">
            Could not check {failed.map((a) => a.ticker).join(", ")}: {failed[0].error}
          </p>
        )}
        {noticesQuery.isSuccess && rows.length === 0 && failed.length === 0 && (
          <p className="text-sm text-muted-foreground">No recent notices.</p>
        )}
        {rows.length > 0 && (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Ticker</TableHead>
                <TableHead>Notice date</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.pdf_url}>
                  <TableCell>{row.ticker}</TableCell>
                  <TableCell>{row.delivered_on}</TableCell>
                  <TableCell className="text-right">
                    <Button variant="outline" size="sm" onClick={() => openUrl(row.pdf_url)}>
                      Open PDF
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </CardContent>
    </Card>
  );
}
