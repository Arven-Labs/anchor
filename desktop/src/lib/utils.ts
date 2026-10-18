import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

// Valor com o código da moeda na frente ("BRL 1.234,56"). Fase 19: os totais
// do Resumo e da Home ficam na moeda base do Workspace, então o código precisa
// aparecer pra não parecer sempre BRL.
export function formatMoney(value: number, currency: string): string {
  // Zero negativo (-0, ou -0,004 que arredonda para -0,00) sairia como "-0,00": mostra 0,00.
  const shown = Math.abs(value) < 0.005 ? 0 : value
  const amount = shown.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 })
  return `${currency} ${amount}`
}
