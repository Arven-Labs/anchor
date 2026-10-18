import { browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, snap, sql, textOf } from "./helpers.mjs";

// Fase 35, entrega 12 — o caixa da conta Corretora. O diálogo nativo de arquivo não é dirigível
// pelo WebDriver, então a fila é semeada no banco (o mesmo JSON que `stage_statement` e
// `stage_b3_settlements` gravam) e o spec dirige o que vem depois: o painel de importar com a conta
// Corretora, o aporte do banco como transferência, a compra e a venda da liquidação na conta
// Corretora e os saldos no fim. Dados inventados.

function seedItem(workspace, account, counterpart, source, proposed) {
  const payload = JSON.stringify({
    proposed: { reference: null, porquinho: false, broker: counterpart !== null, ...proposed },
  });
  sql(
    "INSERT INTO import_pending (workspace_id, bank_account_id, counterpart_account_id, kind, source, payload, status, created_at) " +
      `VALUES (${workspace}, ${account}, ${counterpart ?? "NULL"}, 'bank_transaction', '${source}', '${payload}', 'pending', '2026-10-19T00:00:00Z')`,
  );
}

const ledgerRows = () => Number(sql("SELECT count(*) FROM general_transactions"));
const balance = (account) =>
  Number(
    sql(
      "SELECT coalesce(sum(CASE WHEN transaction_type IN ('despesa','transferencia_saida') THEN -valor ELSE valor END), 0) " +
        `FROM general_transactions WHERE bank_account_id = ${account}`,
    ),
  );

let checking;
let broker;

describe("Anchor — caixa da conta Corretora", () => {
  it("semeia o aporte do banco e a compra e a venda da liquidação", async () => {
    await enterWorkspace("E2E Brokerage");
    const workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Brokerage'");
    const account = (name) => {
      sql(
        "INSERT INTO bank_accounts (workspace_id, nome, titular, currency, created_at) " +
          `VALUES (${workspace}, '${name}', 'Test holder', 'BRL', '2026-01-01T00:00:00Z')`,
      );
      return sql(`SELECT id FROM bank_accounts WHERE workspace_id = ${workspace} AND nome = '${name}'`);
    };
    checking = account("Checking");
    broker = account("Brokerage");
    seedItem(workspace, checking, null, "inter_csv", {
      line: 1, transaction_date: "2025-03-10", valor: 2000, label: "Pix recebido", description: "Pessoa A",
    });
    seedItem(workspace, checking, broker, "inter_csv", {
      line: 2, transaction_date: "2025-03-11", valor: -1000, label: "Transferência enviada", description: "Titular",
    });
    seedItem(workspace, broker, null, "b3_settlement", {
      line: 3, transaction_date: "2025-03-13", valor: -900, label: "Compra", description: "ABCD3 · 100",
    });
    seedItem(workspace, broker, null, "b3_settlement", {
      line: 4, transaction_date: "2025-03-21", valor: 400, label: "Venda", description: "ABCD3 · 40",
    });

    await byId("nav-pending").click();
    await byId("statement-review-open").waitForDisplayed({ timeout: 30_000 });
    expect(await textOf(byId("statement-pending"))).toContain("Statement pending review (4)");
    expect(ledgerRows()).toBe(0);
  });

  it("o painel de importar oferece a conta Corretora e a importação das liquidações", async () => {
    await byId("nav-finances").click();
    await byId("finances-tab-transactions").click();
    await byId("statement-broker").waitForDisplayed({ timeout: 30_000 });
    expect(await textOf(byId("statement-broker"))).toContain("None");
    const importButton = byId("settlement-import-button");
    await importButton.scrollIntoView({ block: "center" });
    expect(await importButton.isEnabled()).toBe(false);
    await snap("brokerage-import-panel");
  });

  it("o aporte vira transferência e a compra e a venda mexem só no caixa da Corretora", async () => {
    await byId("nav-pending").click();
    await clickCentered(byId("statement-review-open"));
    await byId("statement-review-form").waitForDisplayed({ timeout: 20_000 });

    await byId("statement-correct").click(); // Pix recebido: +2000
    await browser.waitUntil(() => ledgerRows() === 1, { timeout: 20_000, timeoutMsg: "a receita deveria estar gravada" });

    await byId("statement-transfer").waitForDisplayed({ timeout: 10_000 });
    expect(await textOf(byId("statement-transfer"))).toBe("Checking → Brokerage");
    await snap("brokerage-transfer-review");
    await byId("statement-correct").click();
    await browser.waitUntil(() => ledgerRows() === 3, { timeout: 20_000, timeoutMsg: "as duas pernas deveriam estar gravadas" });

    await browser.waitUntil(async () => (await byId("statement-valor").getValue()) === "900", {
      timeout: 10_000,
      timeoutMsg: "o terceiro item é a compra de 900",
    });
    await byId("statement-correct").click();
    await browser.waitUntil(() => ledgerRows() === 4, { timeout: 20_000, timeoutMsg: "a compra deveria estar gravada" });

    await browser.waitUntil(async () => (await byId("statement-valor").getValue()) === "400", {
      timeout: 10_000,
      timeoutMsg: "o quarto item é a venda de 400",
    });
    await byId("statement-correct").click();
    await byId("statement-review-empty").waitForDisplayed({ timeout: 20_000 });
    expect(ledgerRows()).toBe(5);
  });

  it("os saldos fecham: o aporte só muda o dinheiro de conta, a compra e a venda mexem no total", async () => {
    // Banco: +2000 −1000 (aporte). Corretora: +1000 −900 (compra) +400 (venda).
    expect(balance(checking)).toBeCloseTo(1000, 2);
    expect(balance(broker)).toBeCloseTo(500, 2);
    expect(balance(checking) + balance(broker)).toBeCloseTo(1500, 2);
    expect(sql("SELECT group_concat(DISTINCT transaction_type) FROM general_transactions WHERE bank_account_id = " + broker)).toContain("transferencia_entrada");

    // O popup, vazio, continua aberto: fecha antes de navegar.
    await browser.keys("Escape");
    await byId("statement-review-empty").waitForDisplayed({ reverse: true, timeout: 10_000 });
    await byId("nav-finances").click();
    await byId("finances-tab-accounts").waitForDisplayed({ timeout: 30_000 });
    await byId("finances-tab-accounts").click();
    await browser.pause(500);
    await snap("brokerage-accounts");
  });
});
