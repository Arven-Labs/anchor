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

// Frente 2 do ROADMAP (RI): releases, apresentações de resultado e fatos relevantes do IPE da CVM.
// Só links para o PDF no portal da CVM; nada é baixado nem lido pelo app.
type CompanyFiling = {
  delivered_on: string;
  kind: "release" | "presentation" | "material_fact";
  subject: string;
  pdf_url: string;
};

const KIND_LABEL: Record<CompanyFiling["kind"], string> = {
  release: "Earnings release",
  presentation: "Analyst presentation",
  material_fact: "Material fact",
};

export default function CompanyFilingsCard({ ticker }: { ticker: string }) {
  const filingsQuery = useQuery<CompanyFiling[], AppError>({
    queryKey: ["company_filings", ticker.toUpperCase()],
    queryFn: () => invoke("list_company_filings", { ticker }),
    staleTime: 60 * 60 * 1000,
  });

  const rows = filingsQuery.data ?? [];

  return (
    <Card data-testid="company-filings">
      <CardHeader>
        <CardTitle>Releases, presentations and material facts</CardTitle>
        <CardDescription>
          Last 12 months, from the CVM, up to 5 of each type. Each link opens the PDF on the CVM
          portal; the app does not read it. Not every company files its release there.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {filingsQuery.isLoading && (
          <p className="text-sm text-muted-foreground">Checking the CVM...</p>
        )}
        {filingsQuery.isError && (
          <p className="text-sm text-destructive">{filingsQuery.error.message}</p>
        )}
        {filingsQuery.isSuccess && rows.length === 0 && (
          <p className="text-sm text-muted-foreground">No release, presentation or material fact found.</p>
        )}
        {rows.length > 0 && (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Date</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Subject</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.pdf_url}>
                  <TableCell>{row.delivered_on}</TableCell>
                  <TableCell>{KIND_LABEL[row.kind]}</TableCell>
                  <TableCell>{row.subject || "-"}</TableCell>
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
