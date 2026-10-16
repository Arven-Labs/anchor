// E2E do Anchor com WebdriverIO + tauri-driver (WebKitWebDriver no Linux).
// Sobe o binário **debug** do app (`tauri build --debug --no-bundle`: o
// frontend vem embutido e `debug_assertions` fica ligado, o que pula o
// sidecar do Lume) apontando pra um banco descartável via `ANCHOR_DB_PATH`.
// O app migra o banco sozinho ao abrir (`db::connect`).

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const appBinary =
  process.env.ANCHOR_E2E_BINARY ?? path.resolve(here, "../src-tauri/target/debug/anchor");
const artifactsDir = path.resolve(here, "artifacts");

// Um banco novo por execução. A config é carregada no processo principal e no
// worker; só o do worker importa (é ele quem sobe o tauri-driver e, por herança
// de ambiente, o app), então cada um ter o próprio diretório não atrapalha.
const dbDir = fs.mkdtempSync(path.join(os.tmpdir(), "anchor-e2e-"));
process.env.ANCHOR_DB_PATH = path.join(dbDir, "anchor.db");

let tauriDriver;

function stopTauriDriver() {
  tauriDriver?.kill();
  tauriDriver = undefined;
}

export const config = {
  runner: "local",
  host: "127.0.0.1",
  port: 4444,
  specs: ["./specs/**/*.e2e.mjs"],
  maxInstances: 1,
  capabilities: [
    {
      maxInstances: 1,
      "tauri:options": { application: appBinary },
    },
  ],
  logLevel: "warn",
  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 120_000 },
  waitforTimeout: 20_000,

  beforeSession: () => {
    const bin = path.resolve(os.homedir(), ".cargo", "bin", "tauri-driver");
    tauriDriver = spawn(bin, [], { stdio: [null, process.stdout, process.stderr] });
    tauriDriver.on("error", (error) => {
      console.error("failed to start tauri-driver:", error);
      process.exit(1);
    });
  },

  // Em falha, guarda uma captura de tela (o CI sobe a pasta como artefato).
  afterTest: async (test, _context, { passed }) => {
    if (passed) return;
    fs.mkdirSync(artifactsDir, { recursive: true });
    const name = test.title.replace(/[^a-z0-9]+/gi, "-").toLowerCase();
    await browser.saveScreenshot(path.join(artifactsDir, `${name}.png`));
  },

  afterSession: () => stopTauriDriver(),
};

process.on("exit", stopTauriDriver);
process.on("SIGINT", () => {
  stopTauriDriver();
  process.exit();
});
process.on("SIGTERM", () => {
  stopTauriDriver();
  process.exit();
});
