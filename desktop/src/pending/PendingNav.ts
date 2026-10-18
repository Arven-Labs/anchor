import { createContext, useContext } from "react";

// Fase 35, entrega 6 — a aba Pending reúne as filas de revisão. Quem importa (Ledger,
// Finances) pede a navegação por este contexto, sem passar props por toda a árvore.
// `portfolio` abre a revisão daquela carteira; `statement` a do extrato e dos rendimentos.
export type PendingFocus = { kind: "portfolio"; portfolioId: number } | { kind: "statement" };

export const PendingNavContext = createContext<(focus?: PendingFocus) => void>(() => {});

export function useOpenPending() {
  return useContext(PendingNavContext);
}
