import { ArrowDownIcon, ArrowUpIcon, MinusIcon } from "lucide-react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { DiCurveView } from "./types";
import { formatDate, formatRate } from "./useInterestRates";

// Abaixo disso a variação é ruído de arredondamento, não tendência.
const FLAT_THRESHOLD_PP = 0.05;

// Prazos-chave da curva DI (1/2/3/5/10 anos) hoje vs. ~1 mês atrás. A
// direção vai em ícone + texto (não só cor), e o texto usa as cores de
// texto do tema — nunca a cor da série.
export default function KeyRatesTable({ di }: { di: DiCurveView }) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Term</TableHead>
          <TableHead className="text-right">{formatDate(di.reference_date)}</TableHead>
          <TableHead className="text-right">
            {di.month_ago_reference_date ? formatDate(di.month_ago_reference_date) : "1 month ago"}
          </TableHead>
          <TableHead className="text-right">Change</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {di.key_rates.map((k) => {
          const delta =
            k.rate_pct != null && k.rate_pct_month_ago != null
              ? k.rate_pct - k.rate_pct_month_ago
              : null;
          const flat = delta != null && Math.abs(delta) < FLAT_THRESHOLD_PP;
          return (
            <TableRow key={k.years}>
              <TableCell>{k.years} {k.years === 1 ? "year" : "years"}</TableCell>
              <TableCell className="text-right font-medium">{formatRate(k.rate_pct)}</TableCell>
              <TableCell className="text-right text-muted-foreground">
                {formatRate(k.rate_pct_month_ago)}
              </TableCell>
              <TableCell className="text-right">
                {delta == null ? (
                  "—"
                ) : (
                  <span className="inline-flex items-center gap-1">
                    {flat ? (
                      <MinusIcon className="size-3.5" aria-hidden />
                    ) : delta > 0 ? (
                      <ArrowUpIcon className="size-3.5" aria-hidden />
                    ) : (
                      <ArrowDownIcon className="size-3.5" aria-hidden />
                    )}
                    {flat ? "flat" : `${delta > 0 ? "+" : ""}${delta.toFixed(2)} pp`}
                  </span>
                )}
              </TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}
