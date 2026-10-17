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

  it("uma transferência entre moedas vira uma linha só e mostra a taxa efetiva", async () => {
    await byId("finances-tab-transactions").click();
    await byId("transfer-new").click();

    await pickOption(byId("transfer-from"), "Nubank (Me) · BRL");
    await pickOption(byId("transfer-to"), "Banco Nación (Me) · ARS");
    await byId("transfer-amount-out").setValue("100");
    await byId("transfer-amount-in").setValue("18000");
    await waitForText(
      byId("transfer-rate"),
      (text) => text.includes("180.0000"),
      "a taxa efetiva (18000 ÷ 100) deveria aparecer no formulário",
    );
    await byId("transfer-submit").click();

    await waitForText(
      byId("transfer-value"),
      (text) => text === "100.00 BRL → 18000.00 ARS",
      "a transferência deveria aparecer como uma linha com saída e entrada",
    );
    // Uma linha só, não duas pernas.
    expect((await elementsOf('[data-testid="transfer-row"]')).length).toBe(1);
  });
});
