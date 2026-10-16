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
export async function waitForText(element, predicate, message) {
  await browser.waitUntil(async () => predicate(await textOf(element)), {
    timeoutMsg: message,
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
