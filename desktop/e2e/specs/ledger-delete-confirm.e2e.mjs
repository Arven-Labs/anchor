import { browser, expect } from "@wdio/globals";
import { byId, enterWorkspace, sql, textOf } from "./helpers.mjs";

// P22: o Delete do Ledger tem 2 cliques (o 1º arma o "Confirm?", o 2º exclui). O `onSuccess` de uma
// exclusão não pode desarmar o botão de OUTRA linha. Para a corrida ser determinística (e não uma
// questão de milissegundos), a resposta de `delete_transaction` é atrasada no navegador: o backend
// apaga na hora, mas a tela só fica sabendo depois. Nesse intervalo o teste arma a linha B.
const DELAY_MS = 3_000;

const exists = (id) => Number(sql(`SELECT count(*) FROM transactions WHERE id = ${id}`)) === 1;

function seedBuy(portfolio, ticker, date) {
  sql(
    "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
      `VALUES ('${ticker}', '${ticker}', 'acao_br', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')`,
  );
  const asset = sql(`SELECT id FROM assets WHERE ticker = '${ticker}'`);
  sql(
    "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
      `VALUES (${portfolio}, ${asset}, 'compra', 10, 10, 100, 0, '${date}', '${date}T00:00:00Z')`,
  );
  return sql(`SELECT id FROM transactions WHERE asset_id = ${asset}`);
}

describe("Anchor — Ledger: a exclusão de uma linha não desarma o Confirm? de outra", () => {
  let rowA;
  let rowB;

  it("semeia duas compras independentes e abre o Ledger", async () => {
    await enterWorkspace("E2E Ledger Confirm");
    const workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Ledger Confirm'");
    sql(`INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`);
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    rowA = seedBuy(portfolio, "AAAA3", "2026-02-02");
    rowB = seedBuy(portfolio, "BBBB3", "2026-02-03");

    await byId("nav-portfolio").click();
    await byId("portfolio-tab-ledger").waitForDisplayed({ timeout: 30_000 });
    await byId("portfolio-tab-ledger").click();
    await byId(`ledger-delete-${rowA}`).waitForDisplayed({ timeout: 30_000 });
    await byId(`ledger-delete-${rowB}`).waitForDisplayed({ timeout: 30_000 });
  });

  it("atrasa a resposta de delete_transaction no navegador", async () => {
    // `invoke` do @tauri-apps/api é resolvido na hora da chamada (core.js), então trocar a função vale.
    const applied = await browser.execute((delay) => {
      const internals = window.__TAURI_INTERNALS__;
      const original = internals.invoke;
      window.__delayedDeletes = 0;
      const patched = (cmd, args, options) => {
        const result = original.call(internals, cmd, args, options);
        if (cmd !== "delete_transaction") return result;
        window.__delayedDeletes += 1;
        return result.then((value) => new Promise((resolve) => setTimeout(() => resolve(value), delay)));
      };
      internals.invoke = patched;
      return internals.invoke === patched;
    }, DELAY_MS);
    console.log(`DIAG patch aplicado=${applied}`);
    expect(applied).toBe(true);
  });

  it("excluir A e armar B enquanto a resposta de A não chegou: B continua em Confirm?", async () => {
    await byId(`ledger-delete-${rowA}`).click(); // arma A
    await byId(`ledger-delete-${rowA}`).click(); // confirma A: o backend apaga já, a tela só sabe daqui a DELAY_MS
    await browser.waitUntil(() => !exists(rowA), { timeout: 20_000, timeoutMsg: "A deveria ter sido apagada no banco" });

    // A resposta de A ainda está atrasada: a linha A continua na tela e B ainda não foi armada.
    console.log(
      `DIAG delete interceptadas=${await browser.execute(() => window.__delayedDeletes)} ` +
        `linhaA na tela=${await byId(`ledger-delete-${rowA}`).isExisting()}`,
    );
    expect(await byId(`ledger-delete-${rowA}`).isExisting()).toBe(true);
    await byId(`ledger-delete-${rowB}`).click(); // arma B
    expect(await textOf(byId(`ledger-delete-${rowB}`))).toBe("Confirm?");

    // A resposta de A chega: a tela tira a linha A e roda o `onSuccess` dela.
    await byId(`ledger-delete-${rowA}`).waitForExist({ timeout: 20_000, reverse: true });

    // O `onSuccess` de A só pode limpar o arme de A, não o de B.
    expect(await textOf(byId(`ledger-delete-${rowB}`))).toBe("Confirm?");
  });

  it("o clique seguinte em B exclui de fato", async () => {
    await byId(`ledger-delete-${rowB}`).click();
    await browser.waitUntil(() => !exists(rowB), { timeout: 20_000, timeoutMsg: "B deveria ter sido apagada" });
  });
});
