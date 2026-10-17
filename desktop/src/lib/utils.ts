import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

// Valor com o código da moeda na frente ("BRL 1.234,56"). Fase 19: os totais
// do Resumo e da Home ficam na moeda base do Workspace, então o código precisa
// aparecer pra não parecer sempre BRL.
export function formatMoney(value: number, currency: string): string {
  const amount = value.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 })
  return `${currency} ${amount}`
}
