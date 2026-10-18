import { browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql, textOf, waitForText } from "./helpers.mjs";

// Fase 35, entrega 3 — o popup de revisão do extrato bancário e a fila de pendentes
// em Finances. O diálogo nativo de arquivo não é dirigível pelo WebDriver, então a
// fila é semeada no banco (o mesmo JSON que `stage_statement` grava) e o spec dirige
// o que vem depois: aprovar com edição, a transferência com o porquinho, pular,
// descartar, fechar o popup deixando o resto pendente e conferir os saldos.
// Dados inventados.

function seedItem(workspace, account, counterpart, proposed) {
  const payload = JSON.stringify({
    proposed: { reference: null, porquinho: counterpart !== null, ...proposed },
  });
  sql(
    "INSERT INTO import_pending (workspace_id, bank_account_id, counterpart_account_id, kind, source, payload, status, created_at) " +
      `VALUES (${workspace}, ${account}, ${counterpart ?? "NULL"}, 'bank_transaction', 'inter_csv', '${payload}', 'pending', '2026-10-19T00:00:00Z')`,
  );
}

const counter = () => textOf(byId("statement-review-counter"));
const ledgerRows = () => Number(sql("SELECT count(*) FROM general_transactions"));
const pendingCount = (status) =>
  Number(sql(`SELECT count(*) FROM import_pending WHERE kind = 'bank_transaction' AND status = '${status}'`));
const balance = (account) =>
  Number(
    sql(
      "SELECT coalesce(sum(CASE WHEN transaction_type IN ('despesa','transferencia_saida') THEN -valor ELSE valor END), 0) " +
        `FROM general_transactions WHERE bank_account_id = ${account}`,
    ),
  );

let checking;
let pocket;

describe("Anchor — revisão do extrato bancário", () => {
  it("semeia cinco itens do extrato e mostra a fila em Finances", async () => {
    await enterWorkspace("E2E Statement");
    const workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Statement'");
    const account = (name) => {
      sql(
        "INSERT INTO bank_accounts (workspace_id, nome, titular, currency, created_at) " +
          `VALUES (${workspace}, '${name}', 'Test holder', 'BRL', '2026-01-01T00:00:00Z')`,
      );
      return sql(`SELECT id FROM bank_accounts WHERE workspace_id = ${workspace} AND nome = '${name}'`);
    };
    checking = account("Checking");
    pocket = account("Cdb Porquinho");
    const item = (line, date, valor, label, description, withPocket) =>
      seedItem(workspace, checking, withPocket ? pocket : null, {
        line,
        transaction_date: date,
        valor,
        label,
        description,
      });
    item(1, "2025-01-15", 1000, "Pix recebido", "Pessoa A", false);
    item(2, "2025-01-16", -150.25, "Pix enviado", "Loja B", false);
    item(3, "2025-01-16", -300, "Aplicação", "Cdb Porquinho Banco X", true);
    item(4, "2025-01-20", 50, "Resgate", "Cdb Porquinho Banco X", true);
    item(5, "2025-01-21", -20, "Pix enviado", "Loja C", false);

    await byId("nav-finances").click();
    await byId("finances-tab-transactions").waitForDisplayed({ timeout: 30_000 });
    await byId("finances-tab-transactions").click();
    await byId("statement-review-open").waitForDisplayed({ timeout: 30_000 });
    expect(await textOf(byId("statement-pending"))).toContain("Statement pending review (5)");
    expect((await $$('[data-testid="statement-pending-row"]')).length).toBe(5);
    expect(ledgerRows()).toBe(0);
  });

  it("o botão de importar só liga depois de escolher a conta", async () => {
    expect(await byId("statement-import-inter_ofx").isEnabled()).toBe(false);
    await pickOption(byId("statement-account"), "Checking");
    expect(await byId("statement-import-inter_ofx").isEnabled()).toBe(true);
    expect(await byId("statement-import-inter_csv").isEnabled()).toBe(true);
  });

  it("aprova o primeiro item com o valor editado", async () => {
    await clickCentered(byId("statement-review-open"));
    await byId("statement-review-form").waitForDisplayed({ timeout: 20_000 });
    expect(await counter()).toContain("5 left to review");
    expect(await byId("statement-valor").getValue()).toBe("1000");

    await byId("statement-valor").setValue("900");
    await byId("statement-correct").click();
    await browser.waitUntil(() => ledgerRows() === 1, { timeout: 20_000, timeoutMsg: "a receita deveria estar gravada" });
    expect(sql("SELECT transaction_type || ':' || valor FROM general_transactions")).toBe("receita:900.0");
    await waitForText(byId("statement-review-counter"), (t) => t.includes("4 left to review"), "restam quatro");
  });

  it("aprova a despesa como veio", async () => {
    await browser.waitUntil(async () => (await byId("statement-valor").getValue()) === "150.25", {
      timeout: 10_000,
      timeoutMsg: "o segundo item é a despesa de 150,25",
    });
    await byId("statement-correct").click();
    await browser.waitUntil(() => ledgerRows() === 2, { timeout: 20_000, timeoutMsg: "a despesa deveria estar gravada" });
    expect(sql("SELECT transaction_type FROM general_transactions WHERE valor = 150.25")).toBe("despesa");
  });

  it("a aplicação do porquinho mostra a transferência e grava as duas pernas", async () => {
    await byId("statement-transfer").waitForDisplayed({ timeout: 10_000 });
    expect(await textOf(byId("statement-transfer"))).toBe("Checking → Cdb Porquinho");
    await byId("statement-correct").click();
    await browser.waitUntil(() => ledgerRows() === 4, { timeout: 20_000, timeoutMsg: "as duas pernas deveriam estar gravadas" });
    expect(Number(sql("SELECT count(*) FROM general_transactions WHERE transfer_id IS NOT NULL"))).toBe(2);
    expect(balance(pocket)).toBe(300);
  });

  it("pular mantém o resgate na fila e descartar recusa o último item de vez", async () => {
    await browser.waitUntil(async () => (await textOf(byId("statement-transfer"))) === "Cdb Porquinho → Checking", {
      timeout: 10_000,
      timeoutMsg: "o resgate vai do porquinho para a conta",
    });
    await byId("statement-skip").click();
    await browser.waitUntil(async () => (await byId("statement-valor").getValue()) === "20", {
      timeout: 10_000,
      timeoutMsg: "depois de pular, vem a despesa de 20",
    });
    await byId("statement-discard").click();
    await browser.waitUntil(() => pendingCount("discarded") === 1, { timeout: 20_000, timeoutMsg: "a despesa de 20 deveria estar descartada" });
    expect(ledgerRows()).toBe(4);
    await browser.waitUntil(async () => (await textOf(byId("statement-transfer"))) === "Cdb Porquinho → Checking", {
      timeout: 10_000,
    });
    expect(await counter()).toContain("1 left to review");
  });

  it("fechar o popup deixa o resgate na fila, e revisar de novo o aprova", async () => {
    await browser.keys("Escape");
    await byId("statement-review-form").waitForDisplayed({ reverse: true, timeout: 10_000 });
    await waitForText(byId("statement-pending"), (t) => t.includes("Statement pending review (1)"), "um pendente na fila");
    expect(pendingCount("pending")).toBe(1);

    await clickCentered(byId("statement-review-open"));
    await byId("statement-review-form").waitForDisplayed({ timeout: 20_000 });
    await byId("statement-correct").click();
    await browser.waitUntil(() => ledgerRows() === 6, { timeout: 20_000, timeoutMsg: "o resgate deveria estar gravado" });
    await byId("statement-review-empty").waitForDisplayed({ timeout: 10_000 });
    expect(pendingCount("pending")).toBe(0);
  });

  it("os saldos fecham: o caixa total só mudou pelo que entrou e saiu de verdade", async () => {
    // Conta: +900 −150,25 −300 (aplicação) +50 (resgate). Porquinho: +300 −50.
    expect(balance(checking)).toBeCloseTo(499.75, 2);
    expect(balance(pocket)).toBeCloseTo(250, 2);
    expect(balance(checking) + balance(pocket)).toBeCloseTo(749.75, 2);
  });
});
