import { $$, browser, expect } from "@wdio/globals";
import { byId, enterWorkspace, pickOption, snap, textOf, waitForText } from "./helpers.mjs";

// `$$` devolve um objeto que não é iterável; `Array.from` dá a lista de verdade.
async function elementsOf(selector) {
  return Array.from(await $$(selector));
}

// Fechar o diálogo de transferência com Escape não limpa o rascunho (só o sucesso limpa): abrir de novo
// traz as contas, os valores e os custos de antes. Os testes de IOF partem de um diálogo sem custos nem valores.
async function openCleanTransferDialog() {
  await byId("transfer-new").click();
  while ((await elementsOf('[data-testid="transfer-cost-remove-0"]')).length > 0) {
    await byId("transfer-cost-remove-0").click();
  }
  await byId("transfer-amount-out").clearValue();
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

  // Custos de transferência. Fica no fim: os testes acima conferem o saldo de -100 da 1ª transferência.
  it("uma transferência com IOF e tarifa mostra o custo total na linha única", async () => {
    await byId("nav-finances").click();
    await byId("finances-tab-transactions").click();
    await byId("transfer-new").click();

    await pickOption(byId("transfer-from"), "Nubank (Me) · BRL");
    await pickOption(byId("transfer-to"), "Banco Nación (Me) · ARS");
    await byId("transfer-amount-out").setValue("50");
    await byId("transfer-amount-in").setValue("9000");

    await byId("transfer-cost-add").click();
    await byId("transfer-cost-valor-0").setValue("0.30");
    await byId("transfer-cost-add").click();
    await pickOption(byId("transfer-cost-kind-1"), "Fee / remittance");
    await byId("transfer-cost-valor-1").setValue("4.70");
    await byId("transfer-submit").click();

    await waitForText(
      byId("transfer-costs-total"),
      (text) => text.includes("5.00 BRL"),
      "a linha da transferência deveria somar os custos (0,30 + 4,70) na moeda da origem",
    );
    // Os custos não viram linhas soltas: são 2 transferências, 2 linhas.
    expect((await elementsOf('[data-testid="transfer-row"]')).length).toBe(2);
  });

  // IOF sugerido por taxa digitada (sem alíquota de lei no app): valor enviado × taxa ÷ 100. A taxa 1,10
  // é só um número de exemplo do teste. Cada caso termina com Escape para o próximo abrir um diálogo limpo.
  it("Calculate preenche o IOF com valor enviado × taxa e o valor segue editável", async () => {
    await byId("nav-finances").click();
    await byId("finances-tab-transactions").click();
    await byId("transfer-new").click();
    await pickOption(byId("transfer-from"), "Nubank (Me) · BRL");
    await pickOption(byId("transfer-to"), "Banco Nación (Me) · ARS");
    await byId("transfer-amount-out").setValue("1000");
    await byId("transfer-amount-in").setValue("180000");
    await byId("transfer-cost-add").click();

    await byId("transfer-cost-rate-0").setValue("1.10");
    await byId("transfer-cost-calc-0").click();
    expect(await byId("transfer-cost-valor-0").getValue()).toBe("11.00");

    // A sugestão não trava o campo: o usuário corrige para o que o extrato mostra.
    await byId("transfer-cost-valor-0").setValue("10.50");
    expect(await byId("transfer-cost-valor-0").getValue()).toBe("10.50");

    // Mudar o valor enviado não recalcula sozinho; só um novo clique.
    await byId("transfer-amount-out").setValue("2000");
    expect(await byId("transfer-cost-valor-0").getValue()).toBe("10.50");
    await byId("transfer-cost-calc-0").click();
    expect(await byId("transfer-cost-valor-0").getValue()).toBe("22.00");
    await browser.keys("Escape");
  });

  it("o campo de taxa só existe no custo IOF, e Calculate desliga sem taxa ou sem valor enviado válidos", async () => {
    await openCleanTransferDialog();
    await pickOption(byId("transfer-from"), "Nubank (Me) · BRL");
    await pickOption(byId("transfer-to"), "Banco Nación (Me) · ARS");
    await byId("transfer-cost-add").click();

    // Sem valor enviado nem taxa.
    expect(await byId("transfer-cost-calc-0").isEnabled()).toBe(false);
    await byId("transfer-amount-out").setValue("1000");
    expect(await byId("transfer-cost-calc-0").isEnabled()).toBe(false);
    // Taxa zero e negativa não servem.
    await byId("transfer-cost-rate-0").setValue("0");
    expect(await byId("transfer-cost-calc-0").isEnabled()).toBe(false);
    await byId("transfer-cost-rate-0").setValue("-1");
    expect(await byId("transfer-cost-calc-0").isEnabled()).toBe(false);
    await byId("transfer-cost-rate-0").setValue("1.10");
    expect(await byId("transfer-cost-calc-0").isEnabled()).toBe(true);

    // Tarifa não tem taxa: o campo e o botão somem.
    await pickOption(byId("transfer-cost-kind-0"), "Fee / remittance");
    expect((await elementsOf('[data-testid="transfer-cost-rate-0"]')).length).toBe(0);
    expect((await elementsOf('[data-testid="transfer-cost-calc-0"]')).length).toBe(0);
    await browser.keys("Escape");
  });

  it("com origem em moeda que não é BRL, o diálogo avisa que a base do IOF é o valor enviado", async () => {
    await openCleanTransferDialog();
    await pickOption(byId("transfer-from"), "Banco Nación (Me) · ARS");
    await pickOption(byId("transfer-to"), "Nubank (Me) · BRL");
    await byId("transfer-cost-add").click();
    await waitForText(
      byId("transfer-iof-base-hint"),
      (text) => text.includes("ARS") && text.includes("amount sent"),
      "origem em ARS: o aviso deveria dizer que a sugestão usa o valor enviado em ARS, e que o IOF incide sobre o valor em reais",
    );
    await browser.keys("Escape");
  });

  it("transferir com o IOF calculado grava o custo e a linha mostra o total", async () => {
    await openCleanTransferDialog();
    await pickOption(byId("transfer-from"), "Nubank (Me) · BRL");
    await pickOption(byId("transfer-to"), "Banco Nación (Me) · ARS");
    await byId("transfer-amount-out").setValue("1000");
    await byId("transfer-amount-in").setValue("180000");
    await byId("transfer-cost-add").click();
    await byId("transfer-cost-rate-0").setValue("1.10");
    await byId("transfer-cost-calc-0").click();
    await byId("transfer-submit").click();

    // Há 2 linhas com custo (a da Sessão 386, 5,00, e esta, 11,00): procurar o 11,00 entre todas, não só na 1ª.
    await browser.waitUntil(
      async () => {
        const totals = await Promise.all(
          (await elementsOf('[data-testid="transfer-costs-total"]')).map((el) => el.getText()),
        );
        return totals.some((text) => text.includes("11.00 BRL"));
      },
      { timeout: 10000, timeoutMsg: "a linha da nova transferência deveria mostrar o IOF calculado (11,00 BRL)" },
    );
    expect((await elementsOf('[data-testid="transfer-row"]')).length).toBe(3);
  });
});
