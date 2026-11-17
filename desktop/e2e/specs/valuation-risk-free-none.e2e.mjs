import { $, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql } from "./helpers.mjs";

// Sem nenhuma taxa disponível (sem Selic gravada e sem rede), o hint da taxa
// livre de risco some e o formulário segue funcionando. Spec à parte porque os
// juros ficam em cache no app por 30 minutos (ver `valuation-risk-free.e2e.mjs`).

const modelSelect = () =>
  $('//label[normalize-space(.)="Valuation model"]/following-sibling::button[@role="combobox"]');
const input = (label) => $(`//label[normalize-space(.)="${label}"]/following-sibling::input`);

describe("Anchor — valuation sem taxa livre de risco disponível", () => {
  it("o formulário do DCF abre sem hint e aceita a taxa digitada", async () => {
    await enterWorkspace("Risk Free None E2E");
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('TEST3', 25.0, 'e2e', '2026-11-01T00:00:00Z', 'Test SA', 'SAO', 'BRL')",
    );

    await byId("nav-lookup").click();
    await pickOption(byId("global-search-class"), "Stock (B3)");
    await byId("global-search-ticker").setValue("TEST3");
    await byId("global-search-submit").click();

    const open = await $('//button[normalize-space(.)="New Valuation"]');
    await open.waitForDisplayed({ timeout: 30_000 });
    await clickCentered(open);
    await (await modelSelect()).waitForDisplayed({ timeout: 10_000 });
    await pickOption(modelSelect(), "DCF / FCFF");

    // `risk-free` aparece quando a consulta dos juros termina, com ou sem taxa.
    await byId("risk-free").waitForExist({ timeout: 20_000 });
    expect(await byId("risk-free-hint").isExisting()).toBe(false);

    await input("Risk-free rate — Rf (%)").setValue("6");
    expect(await input("Risk-free rate — Rf (%)").getValue()).toBe("6");
  });
});
