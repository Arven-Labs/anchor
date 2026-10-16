import { byId, enterWorkspace, pickOption, sql, waitForText } from "./helpers.mjs";

// Impostos (Fase 24): prejuízo a compensar e simulação de venda. As compras e
// vendas são semeadas no banco (cadastrá-las pela tela levaria muitos passos
// que não são o foco aqui).
describe("Anchor — impostos: prejuízo a compensar e simulação de venda", () => {
  it("mostra o prejuízo acumulado a compensar", async () => {
    await enterWorkspace("Tax E2E");

    const workspace = sql("SELECT id FROM workspace WHERE name = 'Tax E2E'");
    sql(
      `INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`,
    );
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main'");
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
    // Jan: compra e vende o BOVA11 com prejuízo de 1.000; compra o IVVB11.
    trade(bova, "compra", 100, 10000, "2026-01-02");
    trade(bova, "venda", 100, 9000, "2026-01-20");
    trade(ivvb, "compra", 10, 3000, "2026-01-02");

    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("loss-balance-common"),
      (text) => text.includes("1.000,00"),
      "o saldo de prejuízo da gaveta comum deveria ser R$ 1.000,00",
    );
  });

  it("simula uma venda que usa o prejuízo e paga imposto só sobre o resto", async () => {
    // Vende 10 IVVB11 a 450 (custo 3.000): lucro 1.500, compensa 1.000 e a
    // alíquota semeada incide sobre 500 (15% → R$ 75,00).
    await pickOption(byId("sim-asset"), "IVVB11");
    await byId("sim-quantity").setValue("10");
    await byId("sim-price").setValue("450");
    await byId("sim-submit").click();

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
