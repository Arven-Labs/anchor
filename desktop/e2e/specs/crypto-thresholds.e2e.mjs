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

// Leituras semeadas são relativas a hoje: o score descarta leitura vencida
// (`reading_max_age_days`), então uma data fixa venceria com o relógio do CI.
const daysAgo = (days) => new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 10);
const shortLabel = (isoDate) => {
  const [year, month, day] = isoDate.split("-");
  return `${day}/${month}/${year.slice(2)}`;
};

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
    // nas contagens do ETH dos casos anteriores. As datas são as dos dados, como no SOPR/NUPL:
    // 14, 7 e 0 dias atrás, no limite dos prazos (MVRV 14 dias, funding e Fear & Greed 7).
    const [first, second, third] = [daysAgo(14), daysAgo(7), daysAgo(0)];
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('ADA', 0.2, 'coingecko', '2026-10-16T12:00:00Z', 'Cardano', 'CoinGecko', 'USD')",
    );
    sql(
      "INSERT INTO crypto_indicators (coin, indicator, reading_date, raw_value, signal, source, created_at) VALUES " +
        `('ADA', 'mvrv_z_score', '${first}', -0.4, 'GREEN', 'test', '2026-10-16T12:00:00Z'), ` +
        `('ADA', 'funding_rate_7d', '${first}', 7.0, 'NEUTRAL', 'test', '2026-10-16T12:00:00Z'), ` +
        `('ADA', 'fear_greed_index', '${second}', 20, 'GREEN', 'test', '2026-10-16T12:00:00Z'), ` +
        `('ADA', 'funding_rate_7d', '${third}', 2.0, 'GREEN', 'test', '2026-10-16T12:00:00Z')`,
    );

    await pickOption(byId("global-search-class"), "Crypto");
    await byId("global-search-ticker").setValue("ADA");
    await byId("global-search-submit").click();

    await byId("crypto-score-history").waitForDisplayed({ timeout: 30_000 });
    // 1ª data: MVRV verde + funding neutro (1 verde); 2ª: + Fear & Greed (2); hoje: funding vira verde (3).
    // Todas as leituras ainda estão dentro do prazo, então o score de hoje também fecha em 3/10.
    await waitForText(
      byId("crypto-score-history-summary"),
      (text) => text.includes("3 dates") && text.includes("3/10 green") && text.includes(shortLabel(third)),
      `o resumo do histórico do ADA deveria ter 3 datas e terminar em 3/10 verdes em ${shortLabel(third)}`,
    );
    await byId("crypto-score-history").scrollIntoView({ block: "center" });
    await snap("crypto-score-history");
  });

  it("o seletor de período recorta o histórico, contando da última data de leitura", async () => {
    // XRP semeado com 3 datas espalhadas: 136 dias atrás, 25 dias atrás e hoje (a última).
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('XRP', 2.0, 'coingecko', '2026-10-16T12:00:00Z', 'XRP', 'CoinGecko', 'USD')",
    );
    sql(
      "INSERT INTO crypto_indicators (coin, indicator, reading_date, raw_value, signal, source, created_at) VALUES " +
        `('XRP', 'mvrv_z_score', '${daysAgo(136)}', -0.4, 'GREEN', 'test', '2026-10-16T12:00:00Z'), ` +
        `('XRP', 'fear_greed_index', '${daysAgo(25)}', 20, 'GREEN', 'test', '2026-10-16T12:00:00Z'), ` +
        `('XRP', 'funding_rate_7d', '${daysAgo(0)}', 2.0, 'GREEN', 'test', '2026-10-16T12:00:00Z')`,
    );

    await pickOption(byId("global-search-class"), "Crypto");
    await byId("global-search-ticker").setValue("XRP");
    await byId("global-search-submit").click();

    const summary = byId("crypto-score-history-summary");
    await byId("crypto-score-history-periods").waitForDisplayed({ timeout: 30_000 });
    await waitForText(summary, (text) => text.includes("3 dates"), "All deveria mostrar as 3 datas do XRP");

    await $("//button[normalize-space(text())='30D']").click();
    await waitForText(summary, (text) => text.includes("2 dates"), "30D deveria cortar o ponto de 01/06");

    await $("//button[normalize-space(text())='All']").click();
    await waitForText(summary, (text) => text.includes("3 dates"), "All deveria devolver as 3 datas");
  });

  it("leitura vencida segue na tela com o selo EXPIRED e fora da contagem do score", async () => {
    // Na tela do XRP (caso anterior): o MVRV é de 136 dias atrás (prazo de 14) e o Fear & Greed de 25 (prazo de 7);
    // só o funding, de hoje, vale.
    await byId("indicator-expired-mvrv_z_score").waitForExist({ timeout: 10_000 });
    await byId("indicator-expired-fear_greed_index").waitForExist({ timeout: 10_000 });
    await waitForText(
      byId("crypto-score-header"),
      (text) => text.includes("1/9 green") && text.includes("1 of 9 logged"),
      "só o funding de hoje deveria contar no score do XRP",
    );
    await byId("indicator-expired-mvrv_z_score").scrollIntoView({ block: "center" });
    await snap("crypto-score-expired");
  });
});
