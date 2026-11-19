import { $, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql } from "./helpers.mjs";

// D0 sugerido pelos dividendos dos últimos 12 meses também ao EDITAR um valuation salvo
// (ROADMAP item 33, frente 4; o formulário de criação já tem, ver `valuation-trailing-dividend`).
// TEST3 pagou 2,00 e 0,50 no último ano (datas relativas a hoje, porque a janela segue o
// relógio) e 9,00 há mais de um ano, que não conta. Os cálculos salvos têm D0 = 1,50, então
// o "Use" precisa mudar o campo para 2,5. O hint nunca preenche sozinho.
const GORDON = 301;
const CEILING = 302;

const input = (label) => $(`//label[normalize-space(.)="${label}"]/following-sibling::input`);
const editButton = (modelText) =>
  $(`//tr[.//td[contains(normalize-space(.), "${modelText}")]]//button[normalize-space(.)="Edit"]`);
const D0 = "Current dividend — D0";

const daysAgo = (days) => new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 10);

function seed() {
  sql(
    "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
      "VALUES ('TEST3', 25.0, 'e2e', '2026-11-01T00:00:00Z', 'Test SA', 'SAO', 'BRL')",
  );
  for (const [days, amount] of [[90, 2.0], [200, 0.5], [400, 9.0]]) {
    sql(
      "INSERT INTO stock_dividend_payments (ticker, payment_date, amount, source, fetched_at) " +
        `VALUES ('TEST3', '${daysAgo(days)}', ${amount}, 'e2e', '2026-11-01T00:00:00Z')`,
    );
  }
  const valuation = (id, model, minute) =>
    sql(
      "INSERT INTO valuation (id, ticker, reference_year, current_price, model, fair_price, safety_margin, verdict, updated_at) " +
        `VALUES (${id}, 'TEST3', 2025, 25.0, '${model}', 30.0, 0.17, 'CHEAP', '2026-11-01T10:0${minute}:00Z')`,
    );
  valuation(GORDON, "gordon", 1);
  sql(
    `INSERT INTO gordon_inputs (valuation_id, current_dividend, expected_growth, ke) VALUES (${GORDON}, 1.5, 0.04, 0.14)`,
  );
  valuation(CEILING, "projected_ceiling", 2);
  sql(
    "INSERT INTO projected_ceiling_inputs (valuation_id, current_dividend, expected_growth, projection_years, desired_yield, ke) " +
      `VALUES (${CEILING}, 1.5, 0.04, 5, 0.06, 0.14)`,
  );
}

async function openEdit(modelText) {
  const button = await editButton(modelText);
  await button.waitForDisplayed({ timeout: 20_000 });
  await clickCentered(button);
  await (await input("Ticker")).waitForDisplayed({ timeout: 20_000 });
}

async function closeEdit() {
  await clickCentered(await $('//button[normalize-space(.)="Cancel"]'));
}

describe("Anchor — D0 sugerido pelos dividendos de 12 meses ao editar um valuation salvo", () => {
  it("abre os cálculos salvos de TEST3", async () => {
    await byId("workspace-name").waitForDisplayed({ timeout: 60_000 });
    await enterWorkspace("Edit Trailing Dividend E2E");
    seed();

    await byId("nav-lookup").click();
    await pickOption(byId("global-search-class"), "Stock (B3)");
    await byId("global-search-ticker").setValue("TEST3");
    await byId("global-search-submit").click();

    await byId("open-saved-valuations").waitForDisplayed({ timeout: 30_000 });
    await clickCentered(byId("open-saved-valuations"));
    await clickCentered(await $('//td[normalize-space(.)="TEST3"]'));
  });

  it("Gordon: mostra a soma dos 12 meses sem mexer no D0, e Use troca só o D0", async () => {
    await openEdit("Gordon");

    const hint = byId("trailing-dividend-hint");
    await hint.waitForDisplayed({ timeout: 20_000 });
    expect(await hint.getText()).toContain("R$ 2.50");
    expect(await input(D0).getValue()).toBe("1.5");

    await byId("trailing-dividend-use").click();
    expect(await input(D0).getValue()).toBe("2.5");
    expect(await input("Required return — Ke").getValue()).toBe("14");
    await closeEdit();
  });

  it("Teto Projetado: mostra o mesmo hint e Use troca só o D0", async () => {
    await openEdit("Projected");

    await byId("trailing-dividend-hint").waitForDisplayed({ timeout: 20_000 });
    expect(await input(D0).getValue()).toBe("1.5");

    await byId("trailing-dividend-use").click();
    expect(await input(D0).getValue()).toBe("2.5");
    expect(await input("Discount rate — Ke").getValue()).toBe("14");
  });
});
