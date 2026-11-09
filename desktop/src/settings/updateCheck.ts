import { invoke } from "@tauri-apps/api/core";
import type { AppError } from "../types";

// Fase 6.5 / P9 — espelha `UpdateStatus` de `commands/update_check.rs`. A checagem
// e a instalação leem o mesmo `latest.json`, então o aviso nunca discorda do instalador.
export type UpdateStatus = {
  current: string;
  latest: string | null;
  update_available: boolean;
  release_url: string | null;
};

// Espelha `DownloadedUpdate` de `commands/update_install.rs`.
export type DownloadedUpdate = {
  version: string;
};

export const UPDATE_CHECK_KEY = ["update-check"] as const;

export function checkForUpdate(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>("check_for_update");
}

// Baixa e verifica a assinatura; nada é instalado ainda.
export function downloadUpdate(): Promise<DownloadedUpdate> {
  return invoke<DownloadedUpdate>("download_update");
}

// Instala o que já foi baixado e verificado, e reinicia o app.
export function installUpdate(): Promise<void> {
  return invoke<void>("install_update");
}

export type UpdateCheckError = AppError;
