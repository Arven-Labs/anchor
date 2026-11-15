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
    // Um Workspace novo não tem carteira nem ativos: semeia os dois.
    const workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Portfolio'");
    sql(
      `INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`,
    );
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('PETR4', 'Petrobras', 'acao_br', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
    );
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-transactions").waitForDisplayed({ timeout: 30_000 });
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

  // Editar transação (ADR 0004). A edição refaz o replay do ativo: só recusa o que piora a cobertura das vendas.
  describe("editar transação pelo Ledger", () => {
    // A coluna é REAL: o `sqlite3` imprime 90.0, então compara-se como número.
    const qtyOf = (id) => Number(sql(`SELECT quantity FROM transactions WHERE id = ${id}`));

    it("o Edit abre o formulário preenchido, salva na mesma linha e volta ao Ledger", async () => {
      // Linhas novas num ativo próprio, para não mexer nas contas dos testes acima.
      const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
      sql(
        "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
          "VALUES ('ITUB4', 'Itau', 'acao_br', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
      );
      const itub = sql("SELECT id FROM assets WHERE ticker = 'ITUB4'");
      const row = (type, quantity, price, date) =>
        sql(
          "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
            `VALUES (${portfolio}, ${itub}, '${type}', ${quantity}, ${price}, ${quantity * price}, 0, '${date}', '${date}T00:00:00Z')`,
        );
      row("compra", 100, 10, "2026-02-02");
      row("venda", 80, 12, "2026-03-02");
      const buyId = sql(`SELECT id FROM transactions WHERE asset_id = ${itub} AND transaction_type = 'compra'`);
      const total = Number(sql("SELECT count(*) FROM transactions"));

      await byId("portfolio-tab-ledger").click();
      await byId(`ledger-edit-${buyId}`).waitForDisplayed({ timeout: 30_000 });
      await byId(`ledger-edit-${buyId}`).click();

      // abre a aba Transactions em modo edição, com os valores da linha
      await byId("tx-edit-title").waitForDisplayed({ timeout: 10_000 });
      expect(await byId("tx-quantity").getValue()).toBe("100");
      expect(await byId("tx-unit-price").getValue()).toBe("10");

      await byId("tx-quantity").setValue("90");
      await byId("tx-submit").click();
      await browser.waitUntil(() => qtyOf(buyId) === 90, {
        timeout: 20_000,
        timeoutMsg: "a compra deveria ter sido editada para 90 na mesma linha",
      });
      expect(Number(sql("SELECT count(*) FROM transactions"))).toBe(total);
      // volta ao Ledger
      await byId(`ledger-edit-${buyId}`).waitForDisplayed({ timeout: 10_000 });
    });

    it("recusa a edição que deixaria a venda sem cobertura e não muda nada", async () => {
      const buyId = sql("SELECT id FROM transactions WHERE asset_id = (SELECT id FROM assets WHERE ticker = 'ITUB4') AND transaction_type = 'compra'");
      await byId(`ledger-edit-${buyId}`).click();
      await byId("tx-edit-title").waitForDisplayed({ timeout: 10_000 });
      // a venda posterior é de 80: com 50 em carteira ela ficaria descoberta
      await byId("tx-quantity").setValue("50");
      await byId("tx-submit").click();

      const error = byId("tx-error");
      await error.waitForDisplayed({ timeout: 20_000 });
      expect(await textOf(error)).toContain("quantidade em carteira");
      expect(qtyOf(buyId)).toBe(90);

      // Cancel volta ao Ledger sem gravar
      await byId("tx-cancel").click();
      await byId(`ledger-edit-${buyId}`).waitForDisplayed({ timeout: 10_000 });
      expect(qtyOf(buyId)).toBe(90);
    });

    it("não oferece Edit nas pernas de um swap", async () => {
      const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
      sql(
        "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
          "VALUES ('BTC', 'Bitcoin', 'cripto', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
      );
      const btc = sql("SELECT id FROM assets WHERE ticker = 'BTC'");
      const row = (type, notes) =>
        sql(
          "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, notes, transaction_date, created_at) " +
            `VALUES (${portfolio}, ${btc}, '${type}', 1, 1000, 1000, 0, ${notes}, '2026-04-01', '2026-04-01T00:00:00Z')`,
        );
      row("compra", "'Swap ETH → BTC'");
      row("compra", "NULL");
      const swapLeg = sql(`SELECT id FROM transactions WHERE asset_id = ${btc} AND notes LIKE 'Swap %'`);
      const plain = sql(`SELECT id FROM transactions WHERE asset_id = ${btc} AND notes IS NULL`);

      await byId("portfolio-tab-positions").click();
      await byId("portfolio-tab-ledger").click();
      await byId(`ledger-edit-${plain}`).waitForDisplayed({ timeout: 30_000 });
      expect(await byId(`ledger-edit-${swapLeg}`).isExisting()).toBe(false);
    });

    it("recusa apagar a compra de que a venda depende e deixa apagar a venda e depois a compra", async () => {
      const itub = sql("SELECT id FROM assets WHERE ticker = 'ITUB4'");
      const idOf = (type) => sql(`SELECT id FROM transactions WHERE asset_id = ${itub} AND transaction_type = '${type}'`);
      const buyId = idOf("compra");
      const sellId = idOf("venda");
      const exists = (id) => Number(sql(`SELECT count(*) FROM transactions WHERE id = ${id}`)) === 1;

      await byId("portfolio-tab-positions").click();
      await byId("portfolio-tab-ledger").click();
      await byId(`ledger-delete-${buyId}`).waitForDisplayed({ timeout: 30_000 });
      await byId(`ledger-delete-${buyId}`).click(); // arma o "Confirm?"
      await byId(`ledger-delete-${buyId}`).click(); // confirma
      const error = byId("ledger-delete-error");
      await error.waitForDisplayed({ timeout: 20_000 });
      expect(await textOf(error)).toContain("quantidade em carteira");
      expect(exists(buyId)).toBe(true);

      // a venda sai (nunca piora) e, sem ela, a compra também
      await byId(`ledger-delete-${sellId}`).click();
      await byId(`ledger-delete-${sellId}`).click();
      await browser.waitUntil(() => !exists(sellId), { timeout: 20_000, timeoutMsg: "a venda deveria ter sido apagada" });
      await byId(`ledger-delete-${buyId}`).click();
      await byId(`ledger-delete-${buyId}`).click();
      await browser.waitUntil(() => !exists(buyId), { timeout: 20_000, timeoutMsg: "a compra deveria ter sido apagada" });
    });
  });
});
