import { $, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, snap, sql, waitForText } from "./helpers.mjs";

// Fase 36.2 (ADR 0002) — ler o endereço de uma Ledger. O runner do CI não tem Ledger plugada,
// então este spec prova o caminho sem dispositivo: o botão chega ao Rust de verdade (hidapi),
// a tela mostra "No Ledger found" e nada é cadastrado. A leitura com aparelho é validada à mão.
describe("Anchor — visualizador de wallets, Ledger", () => {
  it("sem Ledger plugada, ler o endereço avisa e não cadastra nada", async () => {
    await enterWorkspace("Ledger E2E");

    // O módulo vem desligado: liga em Settings → Wallet viewer.
    await byId("open-settings").click();
    await byId("settings-back").waitForDisplayed({ timeout: 20_000 });
    await byId("settings-section-Wallet viewer").click();
    await byId("wallet-viewer-toggle").waitForDisplayed({ timeout: 20_000 });
    await clickCentered(byId("wallet-viewer-toggle"));
    await waitForText(byId("wallet-viewer-status"), (text) => text === "On", "o módulo deveria ligar");
    await byId("settings-back").click();

    await byId("nav-wallets").waitForDisplayed({ timeout: 20_000 });
    await byId("nav-wallets").click();
    await byId("ledger-read").waitForDisplayed({ timeout: 20_000 });

    await clickCentered(byId("ledger-read"));
    await byId("ledger-error").waitForDisplayed({ timeout: 30_000 });
    await waitForText(
      byId("ledger-error"),
      (text) => text.includes("No Ledger found"),
      "sem dispositivo, a tela deveria dizer que não achou a Ledger",
    );
    await snap("wallet-viewer-ledger--sem-dispositivo");

    expect(await $('[data-testid="ledger-address"]').isExisting()).toBe(false);
    expect(sql("SELECT COUNT(*) FROM wallet_viewer_address")).toBe("0");
  });
});
