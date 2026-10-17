import { execFileSync } from "node:child_process";
import { $, browser } from "@wdio/globals";

// Seletores por `data-testid` (os que o app expõe de propósito pro E2E).
export const byId = (id) => $(`[data-testid="${id}"]`);
export const byIdPrefix = (prefix) => $(`[data-testid^="${prefix}"]`);

/**
 * Texto de um elemento via `textContent`. O `getText()` do WebKitWebDriver
 * devolve vazio pra controles do Radix (o texto vem num portal), então os
 * specs leem sempre por aqui.
 */
export async function textOf(element) {
  const el = await element;
  return ((await el.getProperty("textContent")) ?? "").trim();
}

/** Espera o texto do elemento passar no predicado (retry até o timeout). */
export async function waitForText(element, predicate, message, timeout) {
  await browser.waitUntil(async () => predicate(await textOf(element)), {
    timeoutMsg: message,
    ...(timeout ? { timeout } : {}),
  });
}

/**
 * Clica num elemento depois de centralizá-lo na janela. O WebDriver rola o
 * elemento até a borda de cima, onde o cabeçalho fixo (`sticky`) do app o cobre
 * e intercepta o clique ("element click intercepted").
 */
export async function clickCentered(element) {
  const el = await element;
  await el.scrollIntoView({ block: "center", inline: "center" });
  await el.click();
}

/** Abre um Select do Radix e escolhe a opção pelo texto exato. */
export async function pickOption(trigger, label) {
  await (await trigger).click();
  const option = await $(`//*[@role="option"][normalize-space(.)="${label}"]`);
  await option.waitForDisplayed({ timeout: 10_000 });
  await option.click();
}

/** Cria um Workspace pela tela inicial e entra nele. */
export async function enterWorkspace(name) {
  const input = await byId("workspace-name");
  await input.waitForDisplayed({ timeout: 60_000 });
  await input.setValue(name);
  await byId("workspace-create").click();
  await byId("nav-finances").waitForDisplayed({ timeout: 30_000 });
}

/**
 * Semeia o cenário fiscal usado pelos specs de impostos: uma carteira, dois
 * ETFs e, em jan/2026, um prejuízo de R$ 1.000 no BOVA11 (compra 10.000,
 * venda 9.000) mais 10 IVVB11 comprados por R$ 3.000. Vender os 10 IVVB11 a
 * R$ 450 dá lucro de 1.500, compensa os 1.000 e tributa os 500 restantes.
 * Precisa ser chamado depois de `enterWorkspace(workspaceName)`.
 */
export function seedLossScenario(workspaceName) {
  const workspace = sql(`SELECT id FROM workspace WHERE name = '${workspaceName}'`);
  sql(
    `INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`,
  );
  const portfolio = sql(`SELECT id FROM portfolio WHERE workspace_id = ${workspace}`);
  for (const ticker of ["BOVA11", "IVVB11"]) {
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        `VALUES ('${ticker}', '${ticker}', 'etf_br', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')`,
    );
  }
  const bova = sql("SELECT id FROM assets WHERE ticker = 'BOVA11'");
  const ivvb = sql("SELECT id FROM assets WHERE ticker = 'IVVB11'");

  const trade = (asset, type, quantity, total, date) =>
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${asset}, '${type}', ${quantity}, ${total / quantity}, ${total}, 0, '${date}', '${date}T00:00:00Z')`,
    );
  trade(bova, "compra", 100, 10000, "2026-01-02");
  trade(bova, "venda", 100, 9000, "2026-01-20");
  trade(ivvb, "compra", 10, 3000, "2026-01-02");
}

/** Vende 10 IVVB11 a R$ 450 na simulação da aba Taxes. */
export async function simulateIvvbSale() {
  await pickOption(byId("sim-asset"), "IVVB11");
  await byId("sim-quantity").setValue("10");
  await byId("sim-price").setValue("450");
  await byId("sim-submit").click();
}

/**
 * Roda SQL direto no banco do teste (`ANCHOR_DB_PATH`), pra semear o que o
 * app só obtém por rede ou por telas longas (cotações, transações). O app já
 * migrou o banco ao abrir, então as tabelas existem. Devolve a saída do
 * `sqlite3` sem espaços nas pontas.
 */
export function sql(statement) {
  return execFileSync(
    "sqlite3",
    ["-cmd", ".timeout 5000", process.env.ANCHOR_DB_PATH, statement],
    { encoding: "utf8" },
  ).trim();
}
