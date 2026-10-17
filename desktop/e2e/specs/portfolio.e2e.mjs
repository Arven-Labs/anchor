import { browser, expect } from "@wdio/globals";
import { byId, enterWorkspace, pickOption, sql, textOf, waitForText } from "./helpers.mjs";

// Fase 34 — compra, venda e recompra pelo formulário de lançamento, com o preço
// médio certo na aba Positions (o custo médio baixa ao vender, em vez da média
// de todas as compras) e a recusa de uma venda acima do saldo.

async function logTrade(type, quantity, unitPrice) {
  await pickOption(byId("tx-type"), type);
  await byId("tx-quantity").setValue(String(quantity));
  await byId("tx-unit-price").setValue(String(unitPrice));
  await byId("tx-submit").click();
}

describe("Anchor — carteira: compra, venda e recompra", () => {
  it("cria um Workspace e semeia o ativo", async () => {
    await enterWorkspace("E2E Portfolio");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('PETR4', 'Petrobras', 'acao_br', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
    );
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-transactions").click();
    await byId("tx-type").waitForDisplayed({ timeout: 30_000 });
  });

  it("o preço médio acompanha a venda: 100 @ 10, vende 50 @ 20, compra 50 @ 30 dá 20", async () => {
    await pickOption(byId("tx-asset"), "PETR4 — Petrobras");
    await logTrade("Buy", 100, 10);
    await logTrade("Sell", 50, 20);
    await logTrade("Buy", 50, 30);

    await browser.waitUntil(() => Number(sql("SELECT count(*) FROM transactions")) === 3, {
      timeout: 20_000,
      timeoutMsg: "os três lançamentos deveriam estar gravados",
    });

    await byId("portfolio-tab-positions").click();
    await waitForText(
      byId("position-qty-PETR4"),
      (text) => text === "100",
      "a posição deveria ter 100 ações (100 − 50 + 50)",
    );
    await waitForText(
      byId("position-avg-PETR4"),
      (text) => text.includes("20.00"),
      "o preço médio deveria ser 20,00; a média histórica das compras daria 16,67",
    );
  });

  it("recusa a venda acima do saldo e não grava nada", async () => {
    await byId("portfolio-tab-transactions").click();
    await pickOption(byId("tx-asset"), "PETR4 — Petrobras");
    await logTrade("Sell", 1000, 10);

    const error = byId("tx-error");
    await error.waitForDisplayed({ timeout: 20_000 });
    expect(await textOf(error)).toContain("quantidade em carteira");
    expect(Number(sql("SELECT count(*) FROM transactions"))).toBe(3);
  });

  it("um desdobramento 2:1 dobra a quantidade e divide o preço médio", async () => {
    await pickOption(byId("tx-type"), "Stock split");
    await byId("tx-quantity").setValue("2");
    await byId("tx-submit").click();

    await browser.waitUntil(() => Number(sql("SELECT count(*) FROM transactions")) === 4, {
      timeout: 20_000,
      timeoutMsg: "o desdobramento deveria estar gravado",
    });
    await byId("portfolio-tab-positions").click();
    await waitForText(byId("position-qty-PETR4"), (text) => text === "200", "200 ações depois do desdobramento");
    await waitForText(byId("position-avg-PETR4"), (text) => text.includes("10.00"), "preço médio 20 / 2 = 10,00");
  });
});
