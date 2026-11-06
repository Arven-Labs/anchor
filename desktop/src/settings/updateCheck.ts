import { invoke } from "@tauri-apps/api/core";
import type { AppError } from "../types";

// Fase 6.5 — espelha `UpdateStatus` de `commands/update_check.rs`. Só avisa de
// versão nova; não baixa nem instala nada.
export type UpdateStatus = {
  current: string;
  latest: string | null;
  update_available: boolean;
  release_url: string | null;
};

export const UPDATE_CHECK_KEY = ["update-check"] as const;

export function checkForUpdate(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>("check_for_update");
}

export type UpdateCheckError = AppError;
