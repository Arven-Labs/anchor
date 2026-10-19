import { browser, expect } from "@wdio/globals";
import {
  byId,
  enterWorkspace,
  seedLossScenario,
  pickOption,
  simulateIvvbSale,
  snap,
  sql,
  textOf,
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

  // IR de cripto: apuração e DARF próprios (código 4600), separados dos da B3.
  it("apura o IR de uma venda de cripto acima da isenção e mostra o DARF 4600", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('BTC', 'Bitcoin', 'cripto', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
    );
    const btc = sql("SELECT id FROM assets WHERE ticker = 'BTC'");
    const trade = (type, total, date) =>
      sql(
        "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
          `VALUES (${portfolio}, ${btc}, '${type}', 1, ${total}, ${total}, 0, '${date}', '${date}T00:00:00Z')`,
      );
    // Comprou por 10 mil e vendeu por 40 mil (acima dos 35 mil de isenção): ganho de 30 mil, 15% = 4.500.
    trade("compra", 10000, "2026-01-02");
    trade("venda", 40000, "2026-02-10");

    // Sai e volta da aba para a apuração ser recarregada.
    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("crypto-tax-due-2026-02"),
      (text) => text.includes("4.500,00"),
      "o DARF de cripto de fevereiro deveria ser R$ 4.500,00",
    );
    await byId("crypto-tax-table").scrollIntoView({ block: "center" });
    await snap("taxes-crypto");

    await byId("crypto-darf-2026-02").click();
    await waitForText(byId("darf-text"), (text) => text.includes("Revenue code: 4600"), "o código da receita da cripto é 4600");
    expect(await textOf(byId("darf-text"))).toContain("4500,00");
    await snap("taxes-crypto-darf");
  });

  // Permuta cripto↔cripto: uma venda e uma compra gravadas juntas; a venda entra na apuração do mês (hoje, a data padrão).
  it("registra uma permuta pela tela e ela vira venda na apuração de cripto", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('ETH', 'Ether', 'cripto', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
    );
    const btc = sql("SELECT id FROM assets WHERE ticker = 'BTC'");
    // O BTC da venda de fevereiro já saiu; compra outro por 10 mil para entregar na troca.
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${btc}, 'compra', 1, 10000, 10000, 0, '2026-02-15', '2026-02-15T00:00:00Z')`,
    );

    // O caso anterior termina com o diálogo do DARF aberto, que cobre a navegação.
    await browser.keys("Escape");
    await byId("darf-text").waitForDisplayed({ reverse: true, timeout: 10_000 });

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-transactions").click();
    await byId("swap-card").waitForDisplayed({ timeout: 30_000 });
    await pickOption(byId("swap-from"), "BTC — Bitcoin");
    await pickOption(byId("swap-to"), "ETH — Ether");
    await byId("swap-quantity-out").setValue("1");
    await byId("swap-quantity-in").setValue("20");
    await byId("swap-value").setValue("50000");
    await byId("swap-card").scrollIntoView({ block: "center" });
    await snap("swap-form");
    await byId("swap-submit").click();
    await byId("swap-done").waitForDisplayed({ timeout: 15_000 });

    // Venda de 50 mil (acima dos 35 mil) com custo de 10 mil: ganho de 40 mil, 15% = 6.000, no mês da data de hoje.
    const month = new Date().toISOString().slice(0, 7);
    await byId("portfolio-tab-taxes").click();
    await waitForText(
      byId(`crypto-tax-due-${month}`),
      (text) => text.includes("6.000,00"),
      "o DARF de cripto do mês da permuta deveria ser R$ 6.000,00",
    );
    await byId("crypto-tax-table").scrollIntoView({ block: "center" });
    await snap("taxes-crypto-swap");
  });
});
