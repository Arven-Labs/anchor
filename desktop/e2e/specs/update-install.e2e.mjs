import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, snap, waitForText } from "./helpers.mjs";
import { generateKeyPair, signFile, UPDATER_PORT } from "./updater-keys.mjs";

// P9 — updater. Um servidor local faz o papel do GitHub Releases (`latest.json` + o pacote) e o
// app, que em build de debug aceita ENDPOINT/PUBKEY do ambiente (a `wdio.conf.mjs` os define),
// o consulta. A chave "confiável" é descartável; a "estranha" assina o que o app não deve aceitar.
// O pacote é um arquivo qualquer: o teste cobre achar, baixar e VERIFICAR a assinatura. Instalar
// de verdade trocaria o executável de quem está testando, então "Restart to install" nunca é clicado.
const PAYLOAD = Buffer.from("anchor-e2e-update-payload");
// O plugin procura primeiro a chave `<os>-<arch>-<pacote>` e cai em `<os>-<arch>`; as quatro apontam
// para o mesmo pacote, então o teste não depende de qual formato o runner reporta.
const PLATFORMS = ["linux-x86_64", "linux-x86_64-appimage", "linux-x86_64-deb", "linux-x86_64-rpm"];

const scenario = { published: true, version: "9.9.9", served: PAYLOAD, signature: "" };
let server;
let trustedSignature;
let strangerSignature;

function latestJson() {
  const entry = {
    signature: scenario.signature,
    url: `http://127.0.0.1:${UPDATER_PORT}/update.bin`,
  };
  return JSON.stringify({
    version: scenario.version,
    notes: "e2e",
    pub_date: "2026-11-09T00:00:00Z",
    platforms: Object.fromEntries(PLATFORMS.map((platform) => [platform, entry])),
  });
}

function handle(request, response) {
  if (request.url === "/latest.json") {
    if (!scenario.published) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, { "content-type": "application/json" }).end(latestJson());
    return;
  }
  if (request.url === "/update.bin") {
    response
      .writeHead(200, {
        "content-type": "application/octet-stream",
        "content-length": scenario.served.length,
      })
      .end(scenario.served);
    return;
  }
  response.writeHead(404).end();
}

const message = () => byId("about-update-message");

async function checkNow(expected) {
  // O app já fez uma checagem ao abrir (antes de o servidor existir); espera ela assentar.
  await browser.waitUntil(async () => await byId("check-for-updates").isEnabled(), {
    timeoutMsg: "o botão Check for updates deveria voltar a ficar habilitado",
  });
  await clickCentered(byId("check-for-updates"));
  await waitForText(
    message(),
    (text) => text.includes(expected),
    `a mensagem deveria conter "${expected}"`,
    30_000,
  );
}

describe("Anchor — updater: achar, baixar e verificar a assinatura", () => {
  before(async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "anchor-e2e-update-"));
    const payloadPath = path.join(dir, "update.bin");
    fs.writeFileSync(payloadPath, PAYLOAD);
    trustedSignature = signFile(process.env.ANCHOR_E2E_UPDATER_KEY, payloadPath);
    strangerSignature = signFile(generateKeyPair(dir, "stranger").privatePath, payloadPath);

    server = http.createServer(handle);
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(UPDATER_PORT, "127.0.0.1", resolve);
    });
  });

  after(() => {
    server?.close();
  });

  it("sem release publicada (404): diz que ainda não há release", async () => {
    await enterWorkspace("Updater E2E");
    await byId("open-settings").click();
    await byId("settings-section-About").click();

    scenario.published = false;
    await checkNow("No releases published yet.");
    expect(await byId("download-update").isExisting()).toBe(false);
  });

  it("versão igual à do app: está na última versão e não oferece download", async () => {
    Object.assign(scenario, {
      published: true,
      version: "0.1.0",
      served: PAYLOAD,
      signature: trustedSignature,
    });
    await checkNow("You are on the latest version.");
    expect(await byId("download-update").isExisting()).toBe(false);
  });

  it("versão nova com assinatura certa: baixa, verifica e oferece reiniciar", async () => {
    Object.assign(scenario, { version: "9.9.9", served: PAYLOAD, signature: trustedSignature });
    await checkNow("Version 9.9.9 is available.");

    await clickCentered(byId("download-update"));
    await waitForText(
      message(),
      (text) => text.includes("downloaded and its signature verified"),
      "o pacote deveria ser baixado e a assinatura verificada",
      30_000,
    );
    await byId("install-update").waitForDisplayed({ timeout: 10_000 });
    expect(await byId("about-update-error").isExisting()).toBe(false);
    await snap("update-install-verified");
  });

  it("pacote adulterado (assinatura de outro conteúdo): recusa e não oferece instalar", async () => {
    Object.assign(scenario, {
      version: "9.9.9",
      served: Buffer.from("anchor-e2e-update-TAMPERED"),
      signature: trustedSignature,
    });
    await checkNow("Version 9.9.9 is available.");

    await clickCentered(byId("download-update"));
    await byId("about-update-error").waitForDisplayed({ timeout: 30_000 });
    expect(await byId("about-update-error").getText()).toContain("update rejected");
    expect(await byId("install-update").isExisting()).toBe(false);
    await snap("update-install-tampered");
  });

  it("assinado por outra chave: recusa e não oferece instalar", async () => {
    Object.assign(scenario, {
      version: "9.9.9",
      served: PAYLOAD,
      signature: strangerSignature,
    });
    await checkNow("Version 9.9.9 is available.");

    await clickCentered(byId("download-update"));
    await byId("about-update-error").waitForDisplayed({ timeout: 30_000 });
    expect(await byId("about-update-error").getText()).toContain("update rejected");
    expect(await byId("install-update").isExisting()).toBe(false);
    await snap("update-install-wrong-key");
  });
});
