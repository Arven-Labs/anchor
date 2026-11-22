import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../types";
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

// Demonstrações financeiras do FII, do `DFIN` da CVM (dados abertos), por CNPJ. Só links para o PDF no portal do
// Fundos.NET: nada é baixado nem lido pelo app, e quem abre o link é o usuário. Não é a lista de RI da ação (relatório
// gerencial e fato relevante de FII só existem no Fundos.NET, sem fonte aberta confirmada, ver ROADMAP).
type FiiFinancialStatement = {
  reference_date: string;
  delivered_on: string;
  version: number;
  auditor_opinion: string;
  pdf_url: string;
};

export default function FiiFinancialStatementsCard({ cnpj }: { cnpj: string }) {
  const statementsQuery = useQuery<FiiFinancialStatement[], AppError>({
    queryKey: ["fii-financial-statements", cnpj],
    queryFn: () => invoke("list_fii_financial_statements", { cnpj }),
    staleTime: 60 * 60 * 1000,
  });

  const rows = statementsQuery.data ?? [];

  return (
    <Card data-testid="fii-financial-statements">
      <CardHeader>
        <CardTitle>Financial statements</CardTitle>
        <CardDescription>
          This year and last year, from the CVM. Funds file one or two a year, so the list is
          short. Each link opens the PDF on the Fundos.NET portal; the app does not read it.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {statementsQuery.isLoading && (
          <p className="text-sm text-muted-foreground">Checking the CVM...</p>
        )}
        {statementsQuery.isError && (
          <p className="text-sm text-destructive">{statementsQuery.error.message}</p>
        )}
        {statementsQuery.isSuccess && rows.length === 0 && (
          <p className="text-sm text-muted-foreground">No financial statement found.</p>
        )}
        {rows.length > 0 && (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Reference</TableHead>
                <TableHead>Delivered</TableHead>
                <TableHead>Auditor opinion</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.pdf_url}>
                  <TableCell>{row.reference_date}</TableCell>
                  <TableCell>{row.delivered_on}</TableCell>
                  <TableCell>{row.auditor_opinion || "-"}</TableCell>
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
