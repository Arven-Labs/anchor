import { $, expect } from "@wdio/globals";
import {
  byId,
  clickCentered,
  enterWorkspace,
  pickOption,
  sql,
  textOf,
  waitForText,
} from "./helpers.mjs";

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

/**
 * Espera a tela do ativo (ou a falha de rede) e pula o teste se a fonte limitou a chamada.
 * `skipOnNoData` é só pro câmbio: o Yahoo às vezes devolve "No data found" pra um par
 * válido, o que é instabilidade da fonte; pros demais tickers isso continua sendo falha.
 */
async function expectQuoteScreen(test, ticker, marker, { skipOnNoData = false } = {}) {
  await $("body").waitUntil(
    async () => {
      const text = await textOf($("body"));
      return text.includes(marker) || text.includes("No data found") || /429|Too Many/i.test(text);
    },
    { timeout: NETWORK_TIMEOUT, timeoutMsg: `a tela de ${ticker} não carregou` },
  );
  const text = await textOf($("body"));
  if (/429|Too Many/i.test(text)) test.skip();
  if (skipOnNoData && text.includes("No data found")) test.skip();
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
    await expectQuoteScreen(this, "USDBRL", "Rate (BRL)", { skipOnNoData: true });

    expect(sql("SELECT currency FROM stock_quotes WHERE ticker = 'USDBRL' LIMIT 1")).toBe("BRL");
    expect(Number(sql("SELECT count(*) FROM stock_price_history WHERE ticker = 'USDBRL'"))).toBeGreaterThan(0);
  });

  it("ação da B3 (PETR4) coleta a cotação e mostra os indicadores", async function () {
    await search("Stock (B3)", "PETR4");
    await expectQuoteScreen(this, "PETR4", "SMA 200");

    // A coleta gravou a cotação no banco, com o nome vindo do Yahoo.
    expect(Number(sql("SELECT count(*) FROM stock_quotes WHERE ticker = 'PETR4'"))).toBeGreaterThan(0);

    // A seção de opções aparece, mas não baixa os ~80 MB da B3 sozinha: só com o clique.
    await byId("load-options").waitForDisplayed({ timeout: 30_000 });
    const section = await textOf(byId("options-section"));
    expect(section).toContain("Load options");
    expect(section).not.toContain("Downloading");

    // O simulador de estratégia funciona sem a cadeia: uma call comprada (strike 40,
    // prêmio 2, 100 opções) perde no máximo o prêmio, R$ 200, e empata em 42.
    await byId("add-leg").click();
    await byId("leg-strike-0").setValue("40");
    await byId("leg-price-0").setValue("2");
    await byId("leg-qty-0").setValue("100");
    await waitForText(
      byId("strategy-result"),
      (text) => text.includes("R$ 200.00") && text.includes("R$ 42.00") && text.includes("Unlimited"),
      "o simulador deveria mostrar perda máxima de R$ 200, equilíbrio em R$ 42 e ganho ilimitado",
    );
    expect(await textOf(byId("strategy-result"))).toContain("200.00 debit");

    // Salva a perna como posição de uma carteira. A perna é manual (sem série da cadeia),
    // então o diálogo pede o ticker; vencimento (hoje + 30 dias) e carteira (a única) já vêm.
    const workspace = sql("SELECT id FROM workspace WHERE name = 'Lookup E2E'");
    sql(
      `INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`,
    );
    await clickCentered(byId("save-strategy"));
    await byId("save-series-0").setValue("petrk400");
    await clickCentered(byId("save-submit"));
    await byId("save-done").waitForDisplayed({ timeout: 20_000 });
    expect(
      sql("SELECT series_ticker || ':' || side || ':' || premium || ':' || underlying_ticker FROM option_positions"),
    ).toBe("PETRK400:long:2.0:PETR4");

    // O diálogo é modal: fecha, senão a busca global fica inerte para os casos seguintes.
    await clickCentered(byId("save-close"));
    await byId("save-strategy-dialog").waitForExist({ reverse: true, timeout: 10_000 });
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

  it("cripto (BTC) monta o score próprio, com 12 indicadores e sem fundamentos de rede", async function () {
    await search("Crypto", "BTC");
    await expectQuoteScreen(this, "BTC", "of 12 logged");
    await waitForText(
      byId("crypto-score-header"),
      (text) => text.includes("of 12 logged"),
      "o score do BTC deveria montar com os 12 indicadores",
    );
    expect(await textOf($("body"))).not.toContain("Network fundamentals");
    expect(Number(sql("SELECT count(*) FROM stock_quotes WHERE ticker = 'BTC'"))).toBeGreaterThan(0);
  });
});
