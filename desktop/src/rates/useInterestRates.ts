import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { InterestRatesView } from "./types";

// Compartilhado entre o widget da Home e a tela de Research — mesma
// `queryKey`, então abrir um depois do outro não busca de novo. Juros mudam
// no máximo uma vez por dia (curva DI) ou a cada Copom (Selic): 30 min de
// `staleTime` evita bater no Lume a cada troca de aba.
export function useInterestRates() {
  return useQuery<InterestRatesView, AppError>({
    queryKey: ["interest-rates"],
    queryFn: () => invoke("get_interest_rates"),
    staleTime: 30 * 60 * 1000,
  });
}

export function formatRate(value: number | null | undefined): string {
  return value == null ? "—" : `${value.toFixed(2)}%`;
}

export function formatDate(date: string): string {
  const [y, m, d] = date.slice(0, 10).split("-");
  return `${d}/${m}/${y}`;
}
