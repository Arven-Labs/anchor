import { browser, expect } from "@wdio/globals";
import { byId, enterWorkspace, sql } from "./helpers.mjs";

// TEMPORÁRIO (medição do flaky do `portfolio.e2e`, sai antes do PR final). Repete N vezes a parte
// suspeita: apagar a venda e, logo depois, a compra, que exigem 2 cliques cada (o 1º arma o "Confirm?").
// Hipótese: o `onSuccess` da exclusão da venda chega depois do 1º clique na compra e desarma o botão.
// WAIT_ON_SCREEN = false reproduz o teste de hoje (espera só o banco); true espera a venda sumir da tela.
const ROUNDS = 20;
const WAIT_ON_SCREEN = false;

const exists = (id) => Number(sql(`SELECT count(*) FROM transactions WHERE id = ${id}`)) === 1;

async function round(i, portfolio) {
  const ticker = `STR${i}`;
  sql(
    "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
      `VALUES ('${ticker}', 'Stress ${i}', 'acao_br', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')`,
  );
  const asset = sql(`SELECT id FROM assets WHERE ticker = '${ticker}'`);
  const row = (type, quantity, price, date) =>
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${asset}, '${type}', ${quantity}, ${price}, ${quantity * price}, 0, '${date}', '${date}T00:00:00Z')`,
    );
  row("compra", 100, 10, "2026-02-02");
  row("venda", 80, 12, "2026-03-02");
  const idOf = (type) => sql(`SELECT id FROM transactions WHERE asset_id = ${asset} AND transaction_type = '${type}'`);
  const buyId = idOf("compra");
  const sellId = idOf("venda");

  await byId("portfolio-tab-positions").click();
  await byId("portfolio-tab-ledger").click();
  await byId(`ledger-delete-${sellId}`).waitForDisplayed({ timeout: 30_000 });

  await byId(`ledger-delete-${sellId}`).click(); // arma
  await byId(`ledger-delete-${sellId}`).click(); // confirma
  await browser.waitUntil(() => !exists(sellId), { timeout: 20_000, timeoutMsg: "a venda deveria ter sido apagada" });
  if (WAIT_ON_SCREEN) {
    await byId(`ledger-delete-${sellId}`).waitForExist({ timeout: 20_000, reverse: true });
  }

  await byId(`ledger-delete-${buyId}`).click(); // arma
  await byId(`ledger-delete-${buyId}`).click(); // confirma
  await browser.waitUntil(() => !exists(buyId), { timeout: 5_000, timeoutMsg: "a compra deveria ter sido apagada" });
}

describe("STRESS — apagar venda e logo depois a compra no Ledger", () => {
  it(`repete ${ROUNDS} vezes (WAIT_ON_SCREEN=${WAIT_ON_SCREEN}) e conta as falhas`, async () => {
    await enterWorkspace("E2E Stress Ledger");
    const workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Stress Ledger'");
    sql(`INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`);
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-ledger").waitForDisplayed({ timeout: 30_000 });

    let failures = 0;
    for (let i = 1; i <= ROUNDS; i++) {
      try {
        await round(i, portfolio);
      } catch (error) {
        failures += 1;
        console.log(`STRESS_ROUND_FAIL ${i}: ${error.message}`);
      }
    }
    console.log(`STRESS_RESULT WAIT_ON_SCREEN=${WAIT_ON_SCREEN} falhas=${failures}/${ROUNDS}`);
    expect(failures).toBe(0);
  });
});
