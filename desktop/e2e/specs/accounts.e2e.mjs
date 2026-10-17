import { $$, browser, expect } from "@wdio/globals";
import { byId, enterWorkspace, pickOption, textOf, waitForText } from "./helpers.mjs";

// `$$` devolve um objeto que não é iterável; `Array.from` dá a lista de verdade.
async function elementsOf(selector) {
  return Array.from(await $$(selector));
}

describe("Anchor — moeda por conta, de ponta a ponta", () => {
  it("cria um Workspace e entra nele", async () => {
    await enterWorkspace("E2E Currency");
  });

  it("uma conta em BRL e outra em outra moeda aparecem com a moeda na lista", async () => {
    await byId("nav-finances").click();
    await byId("finances-tab-accounts").click();

    // Padrão do formulário: BRL.
    await byId("account-new").click();
    await byId("account-name").setValue("Nubank");
    await byId("account-holder").setValue("Me");
    await waitForText(
      byId("account-currency-select"),
      (text) => text.includes("BRL"),
      "a moeda padrão de uma conta nova deveria ser BRL",
    );
    await byId("account-create").click();

    // ARS e não USD/EUR: a lista abre alinhada ao BRL e os itens do topo ficam
    // cortados pela janela (mesmo motivo do caso de Settings → Currency).
    await byId("account-new").waitForClickable({ timeout: 20_000 });
    await byId("account-new").click();
    await byId("account-name").setValue("Banco Nación");
    await byId("account-holder").setValue("Me");
    await pickOption(byId("account-currency-select"), "ARS — Argentine Peso");
    await byId("account-create").click();

    await browser.waitUntil(async () => (await elementsOf('[data-testid="account-currency"]')).length === 2, {
      timeout: 20_000,
      timeoutMsg: "as duas contas deveriam aparecer na lista",
    });
    const cells = await elementsOf('[data-testid="account-currency"]');
    const currencies = await Promise.all(cells.map((cell) => textOf(cell)));
    expect(currencies.sort()).toEqual(["ARS", "BRL"]);
  });
});
