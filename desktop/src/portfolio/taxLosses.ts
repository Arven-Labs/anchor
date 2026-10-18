// Prejuízo a compensar da apuração de IR (Fase 24), espelhando `LossBalance` de `commands/tax.rs`. Compartilhado
// entre a aba Taxes (`TaxesSection.tsx`) e o widget "Losses to offset" da Home (`home/HomeSection.tsx`).
export type Bucket = "common" | "day_trade" | "fii";

export type LossBalance = {
  bucket: Bucket;
  balance: number;
  rate: number;
  tax_saved: number;
};

export const BUCKET_LABELS: Record<Bucket, string> = {
  common: "Regular trades (stocks, units, ETFs, BDRs)",
  day_trade: "Day trade",
  fii: "Real estate funds (FII)",
};
