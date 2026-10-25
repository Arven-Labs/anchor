import { $$, browser, expect } from "@wdio/globals";
import { byId, enterWorkspace, pickOption, snap, textOf, waitForText } from "./helpers.mjs";

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

  // Fase 35, entrega 8. A conta marcada "já na carteira" ganha o selo; o efeito no patrimônio é coberto
  // pelos testes de fluxo (Rust), aqui só o que a tela mostra e persiste.
  it("marcar a conta como já na carteira mostra o selo, e desmarcar o tira", async () => {
    expect((await elementsOf('[data-testid="account-mirrored-badge"]')).length).toBe(0);

    await byId("account-mirrored-toggle").click();
    await browser.waitUntil(async () => (await elementsOf('[data-testid="account-mirrored-badge"]')).length === 1, {
      timeout: 20_000,
      timeoutMsg: "a conta marcada deveria ganhar o selo 'In Portfolio'",
    });
    expect(await textOf(byId("account-mirrored-toggle"))).toBe("Count as cash");
    await snap("accounts-list-with-actions");

    await byId("account-mirrored-toggle").click();
    await browser.waitUntil(async () => (await elementsOf('[data-testid="account-mirrored-badge"]')).length === 0, {
      timeout: 20_000,
      timeoutMsg: "desmarcar deveria tirar o selo",
    });
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

  // Fase 33. A conta em ARS usa um par que nunca foi coletado (a ingestão fica
  // desligada no E2E), então a Home avisa em vez de converter em silêncio.
  it("a Home avisa que o câmbio de uma moeda em uso nunca foi coletado", async () => {
    await byId("nav-home").click();
    await waitForText(
      byId("fx-staleness-notice"),
      (text) => text.includes("ARS/BRL") && text.includes("never fetched"),
      "a Home deveria avisar que o par ARS/BRL nunca foi coletado",
    );
  });

  // Sessão 229. A conta em BRL ficou com -100 da transferência e a em ARS com 18.000 sem câmbio:
  // o widget soma só o que tem rota e conta o que ficou de fora.
  it("o widget da Home mostra o caixa na moeda base e conta a conta sem câmbio", async () => {
    await waitForText(
      byId("net-worth-widget-cash"),
      (text) => text === "BRL -100,00",
      "o caixa deveria somar só a conta em BRL (a de ARS não tem câmbio)",
    );
    await waitForText(
      byId("net-worth-widget-missing-fx"),
      (text) => text.includes("1 item is left out"),
      "o widget deveria contar a conta em ARS que ficou de fora",
    );
    await (await byId("net-worth-widget")).scrollIntoView({ block: "center" });
    await snap("home-net-worth-widget");
  });

  it("a lista de contas mostra o saldo com a moeda e o aviso de câmbio faltando", async () => {
    await byId("nav-finances").click();
    await byId("finances-tab-accounts").click();
    await waitForText(
      byId("account-list"),
      (text) => text.includes("ARS 18.000,00") && text.includes("no ARS/BRL rate") && text.includes("BRL -100,00"),
      "cada saldo deveria sair com o código da moeda, e a conta em ARS avisar que falta câmbio",
    );
    await snap("accounts-foreign-balance");
  });
});
