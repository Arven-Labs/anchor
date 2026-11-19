import { $, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql } from "./helpers.mjs";

// Premissas sugeridas nos valuations (ROADMAP item 33, frente 4): o dividendo pago nos
// últimos 12 meses aparece ao lado do D0 de Gordon e do Teto Projetado e só entra no
// formulário quando o usuário clica em "Use". TEST3 pagou 1,00 e 0,50 no último ano
// (datas relativas a hoje, porque a janela segue o relógio) e 5,00 há mais de um ano,
// que não conta. As regras da janela (futuro, parou de pagar) são cobertas pelos testes Rust.

const modelSelect = () =>
  $('//label[normalize-space(.)="Valuation model"]/following-sibling::button[@role="combobox"]');
const input = (label) => $(`//label[normalize-space(.)="${label}"]/following-sibling::input`);
const D0 = "Current dividend — D0 (R$/share)";

async function selectModel(label) {
  await (await modelSelect()).scrollIntoView({ block: "center", inline: "center" });
  await pickOption(modelSelect(), label);
}

const daysAgo = (days) => new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 10);

describe("Anchor — D0 sugerido pelos dividendos dos últimos 12 meses", () => {
  it("abre o popup New Valuation de TEST3 com proventos dentro e fora do último ano", async () => {
    await byId("workspace-name").waitForDisplayed({ timeout: 60_000 });
    await enterWorkspace("Trailing Dividend E2E");
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('TEST3', 25.0, 'e2e', '2026-11-01T00:00:00Z', 'Test SA', 'SAO', 'BRL')",
    );
    for (const [days, amount] of [[90, 1.0], [200, 0.5], [400, 5.0]]) {
      sql(
        "INSERT INTO stock_dividend_payments (ticker, payment_date, amount, source, fetched_at) " +
          `VALUES ('TEST3', '${daysAgo(days)}', ${amount}, 'e2e', '2026-11-01T00:00:00Z')`,
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

  it("Gordon: mostra a soma dos 12 meses, sem preencher o D0", async () => {
    await selectModel("Gordon / DDM");

    const hint = byId("trailing-dividend-hint");
    await hint.waitForDisplayed({ timeout: 20_000 });
    expect(await hint.getText()).toContain("R$ 1.50");

    expect(await input(D0).getValue()).toBe("");
  });

  it("Gordon: clicar em Use preenche só o D0", async () => {
    await byId("trailing-dividend-use").click();

    expect(await input(D0).getValue()).toBe("1.5");
    expect(await input("Required return — Ke (%)").getValue()).toBe("");
  });

  it("Projected Ceiling: mostra o mesmo hint e Use preenche o D0", async () => {
    await selectModel("Projected Ceiling");

    await byId("trailing-dividend-hint").waitForDisplayed({ timeout: 20_000 });
    await byId("trailing-dividend-use").click();
    expect(await input(D0).getValue()).toBe("1.5");
  });
});
