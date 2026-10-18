import {
  byId,
  enterWorkspace,
  seedLossScenario,
  simulateIvvbSale,
  snap,
  waitForText,
} from "./helpers.mjs";

// Impostos (Fase 24): prejuízo a compensar e simulação de venda. As compras e
// vendas são semeadas no banco (cadastrá-las pela tela levaria muitos passos
// que não são o foco aqui).
describe("Anchor — impostos: prejuízo a compensar e simulação de venda", () => {
  it("mostra o prejuízo acumulado a compensar", async () => {
    await enterWorkspace("Tax E2E");
    seedLossScenario("Tax E2E");

    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("loss-balance-common"),
      (text) => text.includes("1.000,00"),
      "o saldo de prejuízo da gaveta comum deveria ser R$ 1.000,00",
    );
  });

  // Fase 24: o mesmo saldo aparece no widget da Home, vindo da mesma apuração.
  it("a Home mostra o prejuízo a compensar no widget", async () => {
    await byId("nav-home").click();
    await waitForText(
      byId("losses-widget-common"),
      (text) => text.includes("1.000,00"),
      "o widget da Home deveria mostrar R$ 1.000,00 de prejuízo na gaveta comum",
    );
    await waitForText(
      byId("losses-widget"),
      (text) => text.includes("Saves up to") && text.includes("150,00"),
      "o widget deveria mostrar a economia de 15% sobre o saldo",
    );
    // O widget fica abaixo da dobra: centraliza antes da captura.
    await (await byId("losses-widget")).scrollIntoView({ block: "center" });
    await snap("home-losses-widget");
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();
  });

  it("simula uma venda que usa o prejuízo e paga imposto só sobre o resto", async () => {
    // Lucro 1.500, compensa 1.000 e a alíquota semeada incide sobre 500
    // (15% → R$ 75,00).
    await simulateIvvbSale();

    await waitForText(byId("sim-gain"), (text) => text.includes("1.500,00"), "lucro de R$ 1.500,00");
    await waitForText(
      byId("sim-loss-used"),
      (text) => text.includes("1.000,00"),
      "deveria consumir os R$ 1.000,00 de prejuízo",
    );
    await waitForText(
      byId("sim-month-tax"),
      (text) => text.includes("75,00"),
      "imposto de R$ 75,00 sobre os R$ 500,00 restantes",
    );
  });
});
