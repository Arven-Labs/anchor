import { $$, browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql, textOf, waitForText } from "./helpers.mjs";

// Fase 35 — o popup de revisão item a item e a fila de pendentes. O diálogo
// nativo de arquivo não é dirigível pelo WebDriver, então a fila é semeada
// direto no banco (o mesmo JSON que `stage_import` grava) e o spec dirige o
// que vem depois: editar e aprovar, escolher a classe, pular, descartar, fechar
// o popup deixando o resto pendente e voltar a revisar.

function proposed(line, date, type, ticker, quantity, price, assetClass) {
  return {
    proposed: {
      line,
      transaction_date: date,
      transaction_type: type,
      ticker,
      asset_class: null,
      currency: "BRL",
      quantity,
      unit_price: price,
      total_value: quantity * price,
      fee: null,
      custody: null,
      payment_type: null,
      notes: "Importado no E2E",
    },
    asset_class: assetClass,
  };
}

function seedPending(portfolio, payload) {
  sql(
    "INSERT INTO import_pending (portfolio_id, kind, source, payload, status, created_at) " +
      `VALUES (${portfolio}, 'transaction', 'anchor_csv', '${JSON.stringify(payload)}', 'pending', '2026-10-19T00:00:00Z')`,
  );
}

const counter = () => textOf(byId("import-review-counter"));
const pendingCount = (status) =>
  Number(sql(`SELECT count(*) FROM import_pending WHERE status = '${status}'`));
const transactionCount = () => Number(sql("SELECT count(*) FROM transactions"));

describe("Anchor — revisão da importação e fila de pendentes", () => {
  it("semeia quatro itens pendentes e mostra a fila", async () => {
    await enterWorkspace("E2E Review");
    const workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Review'");
    sql(
      `INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`,
    );
    const portfolio = sql(`SELECT id FROM portfolio WHERE workspace_id = ${workspace}`);
    seedPending(portfolio, proposed(1, "2026-01-05", "compra", "PETR4", 10, 30, "acao_br"));
    seedPending(portfolio, proposed(2, "2026-01-06", "compra", "TAEE11", 5, 40, null));
    seedPending(portfolio, proposed(3, "2026-02-05", "venda", "PETR4", 4, 35, "acao_br"));
    seedPending(portfolio, proposed(4, "2026-03-01", "compra", "VALE3", 2, 60, "acao_br"));

    await byId("nav-portfolio").click();
    await byId("portfolio-tab-transactions").waitForDisplayed({ timeout: 30_000 });
    await byId("portfolio-tab-transactions").click();
    await byId("pending-review").waitForDisplayed({ timeout: 30_000 });
    expect(await textOf(byId("pending-items"))).toContain("Pending review (4)");
    expect((await $$('[data-testid="pending-row"]')).length).toBe(4);
    expect(transactionCount()).toBe(0);
  });

  it("edita o primeiro item e o aprova: só o editado é gravado", async () => {
    await clickCentered(byId("pending-review"));
    await byId("import-review-form").waitForDisplayed({ timeout: 20_000 });
    expect(await counter()).toContain("4 left to review");
    expect(await byId("review-ticker").getValue()).toBe("PETR4");

    await byId("review-quantity").setValue("8");
    await byId("review-total").setValue("240");
    await byId("review-correct").click();

    await browser.waitUntil(() => transactionCount() === 1, {
      timeout: 20_000,
      timeoutMsg: "a compra editada deveria estar gravada",
    });
    expect(sql("SELECT quantity FROM transactions")).toBe("8.0");
    expect(sql("SELECT total_value FROM transactions")).toBe("240.0");
    await waitForText(byId("import-review-counter"), (t) => t.includes("3 left to review"), "restam três");
  });

  it("um ativo com final 11 só avança depois de escolher a classe, e o aviso fiscal acompanha", async () => {
    await browser.waitUntil(async () => (await byId("review-ticker").getValue()) === "TAEE11", {
      timeout: 10_000,
      timeoutMsg: "o segundo item deveria ser o TAEE11",
    });
    expect(await byId("review-correct").isEnabled()).toBe(false);

    await pickOption(byId("review-class"), "FII (B3)");
    await waitForText(byId("review-tax-hint"), (t) => t.includes("FII/Fiagro"), "aviso de FII");
    await pickOption(byId("review-class"), "Stock (B3)");
    await waitForText(byId("review-tax-hint"), (t) => t.includes("never exempt"), "unit nunca isenta");

    await byId("review-correct").click();
    await browser.waitUntil(() => transactionCount() === 2, {
      timeout: 20_000,
      timeoutMsg: "o TAEE11 deveria estar gravado",
    });
    expect(sql("SELECT asset_class FROM assets WHERE ticker = 'TAEE11'")).toBe("acao_br");
  });

  it("pular não perde o item, descartar o recusa de vez", async () => {
    await browser.waitUntil(async () => (await byId("review-ticker").getValue()) === "PETR4", {
      timeout: 10_000,
      timeoutMsg: "o terceiro item (a venda) deveria aparecer",
    });
    await byId("review-skip").click();
    await browser.waitUntil(async () => (await byId("review-ticker").getValue()) === "VALE3", {
      timeout: 10_000,
      timeoutMsg: "depois de pular, vem o próximo (VALE3)",
    });
    expect(await counter()).toContain("2 left to review");

    await byId("review-discard").click();
    await browser.waitUntil(() => pendingCount("discarded") === 1, {
      timeout: 20_000,
      timeoutMsg: "o VALE3 deveria estar descartado",
    });
    expect(transactionCount()).toBe(2);
    // Sobrou só a venda que foi pulada.
    await browser.waitUntil(async () => (await byId("review-ticker").getValue()) === "PETR4", {
      timeout: 10_000,
    });
    expect(await counter()).toContain("1 left to review");
  });

  it("fechar o popup deixa o item na fila, e revisar de novo o aprova", async () => {
    await browser.keys("Escape");
    await byId("import-review-form").waitForDisplayed({ reverse: true, timeout: 10_000 });
    await waitForText(byId("pending-items"), (t) => t.includes("Pending review (1)"), "um pendente na fila");
    expect(pendingCount("pending")).toBe(1);

    await clickCentered(byId("pending-review"));
    await byId("import-review-form").waitForDisplayed({ timeout: 20_000 });
    await byId("review-correct").click();
    await browser.waitUntil(() => transactionCount() === 3, {
      timeout: 20_000,
      timeoutMsg: "a venda deveria estar gravada",
    });
    await byId("import-review-empty").waitForDisplayed({ timeout: 10_000 });
    expect(pendingCount("pending")).toBe(0);
    expect(pendingCount("discarded")).toBe(1);
  });

  it("a posição reflete o que foi aprovado: 8 − 4 = 4 PETR4", async () => {
    await browser.keys("Escape");
    await byId("portfolio-tab-positions").click();
    await waitForText(byId("position-qty-PETR4"), (t) => t === "4", "4 ações de PETR4 depois da venda");
  });
});
