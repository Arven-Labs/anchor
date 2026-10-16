import { browser, expect } from "@wdio/globals";
import {
  byId,
  clickCentered,
  enterWorkspace,
  pickOption,
  sql,
  textOf,
  waitForText,
} from "./helpers.mjs";

// Score cripto e editor de thresholds (Fase 31). O botão "Run crypto collector"
// depende de rede e fica de fora; aqui as leituras entram pelo formulário manual.
describe("Anchor — score cripto e editor de thresholds", () => {
  it("abre a tela de Research do ETH", async () => {
    await enterWorkspace("Crypto E2E");

    // A seção do score só aparece com uma cotação do ativo (que no app vem da
    // CoinGecko, que o teste não consulta), então ela é semeada direto no banco.
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('ETH', 2450.5, 'coingecko', '2026-10-16T12:00:00Z', 'Ethereum', 'CoinGecko', 'USD')",
    );

    await pickOption(byId("global-search-class"), "Crypto");
    await byId("global-search-ticker").setValue("ETH");
    await byId("global-search-submit").click();

    await byId("edit-thresholds").waitForDisplayed({ timeout: 30_000 });
    await waitForText(
      byId("crypto-score-header"),
      (text) => text.includes("0/15 green") && text.includes("0 of 15 logged"),
      "o score deveria começar zerado, com 15 indicadores",
    );
  });

  it("registra uma leitura manual e vê o sinal pelo threshold de fábrica", async () => {
    // Funding de 12% a.a.: entre o verde (≤5) e o vermelho (≥20) → neutro.
    await byId("manual-funding_rate_7d").setValue("12");
    await byId("manual-submit").click();

    await waitForText(
      byId("indicator-signal-funding_rate_7d"),
      (text) => text === "NEUTRAL",
      "12% de funding deveria ser NEUTRAL com os limites de fábrica",
    );
    await waitForText(
      byId("crypto-score-header"),
      (text) => text.includes("1 of 15 logged"),
      "a leitura deveria contar como registrada",
    );
  });

  it("editar o threshold reclassifica a leitura, e o Reset desfaz", async () => {
    await clickCentered(byId("edit-thresholds"));

    // Afrouxa: verde até 15, vermelho a partir de 30 → 12% vira verde.
    await byId("threshold-green-funding_rate_7d").waitForDisplayed({ timeout: 20_000 });
    await byId("threshold-green-funding_rate_7d").setValue("15");
    await byId("threshold-red-funding_rate_7d").setValue("30");
    await byId("threshold-save-funding_rate_7d").click();

    await waitForText(
      byId("indicator-signal-funding_rate_7d"),
      (text) => text === "GREEN",
      "depois de editar, a leitura guardada deveria virar GREEN",
    );
    await waitForText(
      byId("crypto-score-header"),
      (text) => text.includes("1/15 green"),
      "o score deveria contar o verde novo",
    );

    // Reset: volta aos limites de fábrica e a leitura volta a NEUTRAL.
    const reset = await byId("threshold-reset-funding_rate_7d");
    await browser.waitUntil(async () => reset.isEnabled());
    await reset.click();

    await waitForText(
      byId("indicator-signal-funding_rate_7d"),
      (text) => text === "NEUTRAL",
      "o Reset deveria devolver o sinal NEUTRAL",
    );
    expect(await textOf(byId("crypto-score-header"))).toContain("0/15 green");
  });
});
