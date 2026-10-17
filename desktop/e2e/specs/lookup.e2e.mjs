import { $, expect } from "@wdio/globals";
import { byId, enterWorkspace, pickOption, sql, textOf, waitForText } from "./helpers.mjs";

// Research com dado de rede real (Fase 26: Yahoo, CoinGecko e SEC/CVM agora são
// nativos em Rust). A primeira busca de um ticker dispara a coleta sozinha, então
// basta pesquisar e esperar a tela montar. Só os fluxos baratos entram aqui; FII
// e CVM (zips pesados) ficam nos testes ao vivo do Rust.
//
// O Yahoo costuma responder 429 a IPs de runner do GitHub. Isso não é bug do app,
// então nesse caso o teste pula em vez de ficar vermelho.
const NETWORK_TIMEOUT = 90_000;

async function search(assetClass, ticker) {
  await pickOption(byId("global-search-class"), assetClass);
  const input = await byId("global-search-ticker");
  await input.setValue(ticker);
  await byId("global-search-submit").click();
}

/** Espera a tela do ativo (ou a falha de rede) e pula o teste se a fonte limitou a chamada. */
async function expectQuoteScreen(test, ticker, marker) {
  await $("body").waitUntil(
    async () => {
      const text = await textOf($("body"));
      return text.includes(marker) || text.includes("No data found") || /429|Too Many/i.test(text);
    },
    { timeout: NETWORK_TIMEOUT, timeoutMsg: `a tela de ${ticker} não carregou` },
  );
  const text = await textOf($("body"));
  if (/429|Too Many/i.test(text)) test.skip();
  expect(text).not.toContain("No data found");
  expect(text).toContain(marker);
}

describe("Anchor — Research com dados de rede", function () {
  // Primeiro caso de propósito: o atalho do câmbio só aparece no estado vazio do
  // Research, antes de qualquer busca.
  it("câmbio (USD → BRL) coleta a cotação e grava o par", async function () {
    await enterWorkspace("Lookup E2E");
    await byId("nav-lookup").click();
    await byId("open-fx").click();
    await pickOption(byId("fx-base"), "USD — US Dollar");
    await pickOption(byId("fx-quote"), "BRL — Brazilian Real");
    await expectQuoteScreen(this, "USDBRL", "Rate (BRL)");

    expect(sql("SELECT currency FROM stock_quotes WHERE ticker = 'USDBRL' LIMIT 1")).toBe("BRL");
    expect(Number(sql("SELECT count(*) FROM stock_price_history WHERE ticker = 'USDBRL'"))).toBeGreaterThan(0);
  });

  it("ação da B3 (PETR4) coleta a cotação e mostra os indicadores", async function () {
    await search("Stock (B3)", "PETR4");
    await expectQuoteScreen(this, "PETR4", "SMA 200");

    // A coleta gravou a cotação no banco, com o nome vindo do Yahoo.
    expect(Number(sql("SELECT count(*) FROM stock_quotes WHERE ticker = 'PETR4'"))).toBeGreaterThan(0);
  });

  it("ação dos EUA (AAPL) coleta a cotação em dólar", async function () {
    await search("International stock", "AAPL");
    await expectQuoteScreen(this, "AAPL", "SMA 200");
    expect(sql("SELECT currency FROM stock_quotes WHERE ticker = 'AAPL' LIMIT 1")).toBe("USD");
  });

  it("cripto (ETH) coleta a cotação pela CoinGecko e monta o score", async function () {
    await search("Crypto", "ETH");
    // O score só aparece com a cotação; "429" cobre o limite de chamadas da CoinGecko.
    await expectQuoteScreen(this, "ETH", "of 17 logged");
    await waitForText(
      byId("crypto-score-header"),
      (text) => text.includes("of 17 logged"),
      "o score cripto deveria montar com os 17 indicadores",
    );
    expect(Number(sql("SELECT count(*) FROM stock_quotes WHERE ticker = 'ETH'"))).toBeGreaterThan(0);
  });
});
