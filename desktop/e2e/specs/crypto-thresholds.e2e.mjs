import { $, browser, expect } from "@wdio/globals";
import {
  byId,
  clickCentered,
  enterWorkspace,
  pickOption,
  snap,
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
      (text) => text.includes("0/20 green") && text.includes("0 of 20 logged"),
      "o score deveria começar zerado, com 20 indicadores",
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
      (text) => text.includes("1 of 20 logged"),
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
      (text) => text.includes("1/20 green"),
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
    expect(await textOf(byId("crypto-score-header"))).toContain("0/20 green");
  });

  it("o grupo Market sentiment classifica as três leituras pelos limites de fábrica", async () => {
    // O editor de thresholds do caso anterior ainda está aberto e cobre o formulário.
    await browser.keys(["Escape"]);
    await byId("threshold-reset-funding_rate_7d").waitForDisplayed({ reverse: true, timeout: 10_000 });

    // Fear & Greed 12 (≤25 → verde), Mayer 1,6 (entre 1,3 e 2,0 → neutro) e dominância 70
    // (≥65 → vermelho): menor é melhor nos três.
    await byId("manual-fear_greed_index").scrollIntoView({ block: "center" });
    await byId("manual-fear_greed_index").setValue("12");
    await byId("manual-submit").click();
    await waitForText(
      byId("indicator-signal-fear_greed_index"),
      (text) => text === "GREEN",
      "Fear & Greed 12 deveria ser GREEN",
    );
    await byId("manual-mayer_multiple").setValue("1.6");
    await byId("manual-submit").click();
    await waitForText(
      byId("indicator-signal-mayer_multiple"),
      (text) => text === "NEUTRAL",
      "Mayer 1,6 deveria ser NEUTRAL",
    );
    await byId("manual-btc_dominance").setValue("70");
    await byId("manual-submit").click();
    await waitForText(
      byId("indicator-signal-btc_dominance"),
      (text) => text === "RED",
      "dominância 70 deveria ser RED",
    );

    await waitForText(
      byId("crypto-score-header"),
      (text) => text.includes("1/20 green") && text.includes("4 of 20 logged"),
      "o score deveria contar o verde do Fear & Greed e as 4 leituras",
    );
    expect(await textOf($("body"))).toContain("Market sentiment");
    // Duas capturas: o cabeçalho do score (topo) e o grupo novo (embaixo do score).
    await byId("crypto-score-header").scrollIntoView({ block: "start" });
    await snap("crypto-score-header");
    await $("//*[normalize-space(text())='Market sentiment']").scrollIntoView({ block: "center" });
    await snap("crypto-score-market-sentiment");
  });

  it("o histórico do score pede duas datas e desenha a série com leituras em dias diferentes", async () => {
    // Todas as leituras manuais do ETH acima entraram no mesmo dia: ainda não há série.
    await byId("crypto-score-history-empty").waitForDisplayed({ timeout: 10_000 });

    // O ADA entra semeado direto no banco (cotação e leituras em 3 datas), sem rede e sem mexer
    // nas contagens do ETH dos casos anteriores. As datas são as dos dados, como no SOPR/NUPL.
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('ADA', 0.2, 'coingecko', '2026-10-16T12:00:00Z', 'Cardano', 'CoinGecko', 'USD')",
    );
    sql(
      "INSERT INTO crypto_indicators (coin, indicator, reading_date, raw_value, signal, source, created_at) VALUES " +
        "('ADA', 'mvrv_z_score', '2026-10-01', -0.4, 'GREEN', 'test', '2026-10-16T12:00:00Z'), " +
        "('ADA', 'funding_rate_7d', '2026-10-01', 7.0, 'NEUTRAL', 'test', '2026-10-16T12:00:00Z'), " +
        "('ADA', 'fear_greed_index', '2026-10-08', 20, 'GREEN', 'test', '2026-10-16T12:00:00Z'), " +
        "('ADA', 'funding_rate_7d', '2026-10-15', 2.0, 'GREEN', 'test', '2026-10-16T12:00:00Z')",
    );

    await pickOption(byId("global-search-class"), "Crypto");
    await byId("global-search-ticker").setValue("ADA");
    await byId("global-search-submit").click();

    await byId("crypto-score-history").waitForDisplayed({ timeout: 30_000 });
    // 01/10: MVRV verde + funding neutro (1 verde); 08/10: + Fear & Greed (2); 15/10: funding vira verde (3).
    await waitForText(
      byId("crypto-score-history-summary"),
      (text) => text.includes("3 dates") && text.includes("3/10 green") && text.includes("15/10/26"),
      "o resumo do histórico do ADA deveria ter 3 datas e terminar em 3/10 verdes em 15/10/26",
    );
    await byId("crypto-score-history").scrollIntoView({ block: "center" });
    await snap("crypto-score-history");
  });
});
