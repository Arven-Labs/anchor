import { $, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, snap, sql, waitForText } from "./helpers.mjs";

// Fase 36.4a (ADR 0002) — Bitcoin por chave pública estendida. O spec prova o caminho sem rede:
// cadastrar a zpub do vetor BIP84 pela tela, ver a lista com o tipo, recusar uma chave privada
// (nada é gravado) e recusar a chave repetida. O saldo ao vivo (Esplora) e o botão Remove não
// rodam aqui (o Remove é coberto pelo teste de Rust do store); o saldo é validado à mão.
const ZPUB =
  "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs";
// Chave privada do vetor 1 do BIP32 (pública na especificação, sem valor).
const XPRV =
  "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi";

describe("Anchor — visualizador de wallets, Bitcoin", () => {
  it("cadastra uma zpub, recusa chave privada e chave repetida", async () => {
    await enterWorkspace("Bitcoin E2E");

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
    await byId("bitcoin-xpub-input").waitForDisplayed({ timeout: 20_000 });

    // Chave privada: a tela mostra o erro e nada é gravado.
    await byId("bitcoin-xpub-input").setValue(XPRV);
    await clickCentered(byId("bitcoin-xpub-add"));
    await byId("bitcoin-xpub-error").waitForDisplayed({ timeout: 20_000 });
    await waitForText(
      byId("bitcoin-xpub-error"),
      (text) => text.includes("not a valid xpub or zpub") && !text.includes(XPRV.slice(0, 20)),
      "a chave privada deveria ser recusada sem aparecer no erro",
    );
    expect(sql("SELECT COUNT(*) FROM wallet_viewer_xpub")).toBe("0");

    // zpub válida: entra na lista, como SegWit nativo.
    await byId("bitcoin-xpub-input").setValue(ZPUB);
    await clickCentered(byId("bitcoin-xpub-add"));
    await $('[data-testid^="wallet-xpub-"]').waitForDisplayed({ timeout: 20_000 });
    await waitForText(
      $('[data-testid^="wallet-xpub-"]'),
      (text) => text.includes("Native SegWit") && text.includes("Balance not loaded yet"),
      "a zpub deveria aparecer como SegWit nativo, com o saldo ainda não carregado",
    );
    await snap("wallet-viewer-bitcoin--zpub-cadastrada");
    expect(sql("SELECT COUNT(*) FROM wallet_viewer_xpub")).toBe("1");
    expect(sql("SELECT xpub FROM wallet_viewer_xpub")).toBe(ZPUB);

    // A mesma chave de novo: recusada, continua uma só.
    await byId("bitcoin-xpub-input").setValue(ZPUB);
    await clickCentered(byId("bitcoin-xpub-add"));
    await waitForText(
      byId("bitcoin-xpub-error"),
      (text) => text.includes("already added"),
      "a chave repetida deveria ser recusada",
    );
    expect(sql("SELECT COUNT(*) FROM wallet_viewer_xpub")).toBe("1");
  });
});
