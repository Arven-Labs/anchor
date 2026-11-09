import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Chaves de teste do updater (P9), geradas na hora com o CLI do Tauri e jogadas fora no fim.
// Nunca é a chave de verdade: a pública de verdade está no `tauri.conf.json` e a privada só
// existe no cofre do dono e nos secrets do GitHub. Este módulo não importa nada do wdio para
// a `wdio.conf.mjs` e o spec poderem usá-lo.
const here = path.dirname(fileURLToPath(import.meta.url));
const tauriBin = path.resolve(here, "../../node_modules/.bin/tauri");

// Porta fixa: o app lê o endereço do ambiente ao abrir, antes de o spec subir o servidor.
export const UPDATER_PORT = 47831;

const run = (args, env = {}) =>
  execFileSync(tauriBin, args, { env: { ...process.env, ...env }, stdio: "pipe" });

/** Gera um par sem senha em `dir/name.key` (+ `.key.pub`). A pública é o texto que a config espera. */
export function generateKeyPair(dir, name) {
  const privatePath = path.join(dir, `${name}.key`);
  run(["signer", "generate", "--ci", "-p", "", "-w", privatePath]);
  return { privatePath, publicKey: fs.readFileSync(`${privatePath}.pub`, "utf8").trim() };
}

/** Assina `file` com a chave privada e devolve o conteúdo do `.sig` (o campo `signature` do latest.json). */
export function signFile(privatePath, file) {
  run(["signer", "sign", "-f", privatePath, "-p", "", file]);
  return fs.readFileSync(`${file}.sig`, "utf8").trim();
}
