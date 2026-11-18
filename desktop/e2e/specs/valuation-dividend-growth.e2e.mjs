import { $, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql } from "./helpers.mjs";

// Premissas sugeridas nos valuations (ROADMAP item 33, frente 4): o crescimento
// histórico dos dividendos aparece ao lado do campo g de Gordon e do Teto Projetado,
// com o período, e só entra no formulário quando o usuário clica em "Use".
// TEST3 paga 10% a mais por ano de 2020 a 2025. As regras do cálculo (histórico curto,
// parou de pagar etc.) são cobertas pelos testes Rust.

const modelSelect = () =>
  $('//label[normalize-space(.)="Valuation model"]/following-sibling::button[@role="combobox"]');
const input = (label) => $(`//label[normalize-space(.)="${label}"]/following-sibling::input`);

async function selectModel(label) {
  await (await modelSelect()).scrollIntoView({ block: "center", inline: "center" });
  await pickOption(modelSelect(), label);
}

async function fill(label, value) {
  const field = await input(label);
  await field.scrollIntoView({ block: "center" });
  await field.setValue(value);
}

describe("Anchor — g sugerido pelo histórico de dividendos", () => {
  it("abre o popup New Valuation de TEST3 com proventos de 2020 a 2025", async () => {
    await byId("workspace-name").waitForDisplayed({ timeout: 60_000 });
    await enterWorkspace("Dividend Growth E2E");
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('TEST3', 25.0, 'e2e', '2026-11-01T00:00:00Z', 'Test SA', 'SAO', 'BRL')",
    );
    const amounts = [1.0, 1.1, 1.21, 1.331, 1.4641, 1.61051];
    for (const [i, amount] of amounts.entries()) {
      sql(
        "INSERT INTO stock_dividend_payments (ticker, payment_date, amount, source, fetched_at) " +
          `VALUES ('TEST3', '${2020 + i}-06-15', ${amount}, 'e2e', '2026-11-01T00:00:00Z')`,
      );
    }

    await byId("nav-lookup").click();
    await pickOption(byId("global-search-class"), "Stock (B3)");
    await byId("global-search-ticker").setValue("TEST3");
    await byId("global-search-submit").click();

    const open = await $('//button[normalize-space(.)="New Valuation"]');
    await open.waitForDisplayed({ timeout: 30_000 });
    await clickCentered(open);
    await (await modelSelect()).waitForDisplayed({ timeout: 10_000 });
  });

  it("Gordon: mostra o crescimento e o período, sem preencher o campo", async () => {
    await selectModel("Gordon / DDM");

    const hint = byId("dividend-growth-hint");
    await hint.waitForDisplayed({ timeout: 20_000 });
    const text = await hint.getText();
    expect(text).toContain("10.0%");
    expect(text).toContain("2020");
    expect(text).toContain("2025");

    expect(await input("Expected dividend growth — g (%)").getValue()).toBe("");
  });

  it("Gordon: clicar em Use preenche só o g", async () => {
    await byId("dividend-growth-use").click();

    expect(await input("Expected dividend growth — g (%)").getValue()).toBe("10");
    expect(await input("Required return — Ke (%)").getValue()).toBe("");
  });

  it("Gordon: Ke igual ou abaixo do g sugerido mostra o aviso", async () => {
    await fill("Required return — Ke (%)", "9");
    await byId("growth-above-ke").waitForDisplayed({ timeout: 10_000 });
  });

  it("Gordon: Ke acima do g sugerido não mostra o aviso", async () => {
    await fill("Required return — Ke (%)", "14");
    expect(await byId("growth-above-ke").isExisting()).toBe(false);
  });

  it("Projected Ceiling: mostra o mesmo hint, sem aviso de Ke, e Use preenche o g", async () => {
    await selectModel("Projected Ceiling");

    await byId("dividend-growth-hint").waitForDisplayed({ timeout: 20_000 });
    expect(await byId("growth-above-ke").isExisting()).toBe(false);

    await byId("dividend-growth-use").click();
    expect(await input("Expected growth — g (%)").getValue()).toBe("10");
  });
});
