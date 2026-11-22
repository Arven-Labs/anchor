import { $, browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, snap, sql, textOf, waitForText } from "./helpers.mjs";

// Previdência privada: simulador de resgate (estimativa informativa do IR, regime com tabela por prazo).
// O ativo é semeado no banco (o spec testa o simulador, não o cadastro) e o app real calcula com as
// alíquotas que a migration semeou em `tax_rules`: 35/30/25/20/15/10% por faixa e 15% de antecipação.
// Dados inventados.

// `toLocaleString("pt-BR")` separa "R$" do número com espaço não separável; tiramos todo espaço para comparar.
const compact = (text) => text.replace(/\s+/g, "");

async function fillForm({ redeemed, years, income }) {
  await byId("pension-redeemed").setValue(redeemed);
  if (income !== undefined) await byId("pension-income").setValue(income);
  await byId("pension-years").setValue(years);
}

async function estimate(form) {
  await fillForm(form);
  await clickCentered(byId("pension-submit"));
  await byId("pension-result").waitForDisplayed({ timeout: 20_000 });
}

describe("Anchor — simulador de resgate de previdência", () => {
  it("abre o detalhe da previdência com PGBL, sem o campo de rendimento e com Estimate desabilitado", async () => {
    await enterWorkspace("E2E Pension");
    const workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Pension'");
    sql(
      `INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`,
    );
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('PREV1', 'Test Pension Plan', 'previdencia', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
    );

    await byId("nav-portfolio").click();
    await byId("portfolio-tab-assets").waitForDisplayed({ timeout: 30_000 });
    await byId("portfolio-tab-assets").click();
    await clickCentered($("//button[normalize-space(.)='Pension details']"));
    await byId("pension-redemption-simulator").waitForDisplayed({ timeout: 20_000 });

    expect(await byId("pension-income").isExisting()).toBe(false);
    expect(await byId("pension-submit").isEnabled()).toBe(false);
    await snap("pension-simulator-empty");
  });

  it("VGBL passa a pedir o rendimento", async () => {
    await pickOption(byId("pension-kind"), "VGBL");
    await byId("pension-income").waitForDisplayed({ timeout: 10_000 });
    await pickOption(byId("pension-kind"), "PGBL");
    await browser.waitUntil(async () => !(await byId("pension-income").isExisting()), {
      timeout: 10_000,
      timeoutMsg: "voltar para PGBL deveria esconder o campo de rendimento",
    });
  });

  it("PGBL: 10.000 resgatados após 5 anos (faixa de 4 a 6 anos) pagam 25% sobre o total", async () => {
    await estimate({ redeemed: "10000", years: "5" });
    const result = compact(await textOf(byId("pension-result")));
    expect(result).toContain("Rate:25.00%");
    expect(result).toContain("R$10.000,00(thewholeredemption)");
    expect(result).toContain("Estimatedtax:R$2.500,00");
    expect(result).toContain("R$7.500,00");
    expect(result).toContain("15.00%");
    await snap("pension-simulator-pgbl");
  });

  it("exatamente 2 anos ainda está na faixa de 35% (limite superior inclusivo)", async () => {
    await estimate({ redeemed: "10000", years: "2" });
    await waitForText(
      byId("pension-result"),
      (text) => compact(text).includes("Rate:35.00%"),
      "2 anos exatos deveriam cair em 35%, não em 30%",
    );
  });

  it("VGBL: só o rendimento é tributado (35% sobre 2.000 num resgate de 10.000 após 1 ano)", async () => {
    await pickOption(byId("pension-kind"), "VGBL");
    await byId("pension-income").waitForDisplayed({ timeout: 10_000 });
    await estimate({ redeemed: "10000", income: "2000", years: "1" });
    // O resultado do caso anterior (também 35%) ainda pode estar na tela: espera o do VGBL aparecer.
    await waitForText(
      byId("pension-result"),
      (text) => compact(text).includes("(theincomeonly)"),
      "o resultado do VGBL deveria aparecer",
    );
    const result = compact(await textOf(byId("pension-result")));
    expect(result).toContain("Rate:35.00%");
    expect(result).toContain("R$2.000,00(theincomeonly)");
    expect(result).toContain("Estimatedtax:R$700,00");
    expect(result).toContain("R$9.300,00");
    await snap("pension-simulator-vgbl-result");
  });

  it("VGBL com rendimento maior que o resgate mostra o erro e não mostra resultado", async () => {
    await fillForm({ redeemed: "1000", income: "1500", years: "1" });
    await clickCentered(byId("pension-submit"));
    const error = $("//p[contains(@class,'text-destructive')]");
    await error.waitForDisplayed({ timeout: 20_000 });
    expect(await textOf(error)).toContain("greater than the redeemed amount");
    expect(await byId("pension-result").isExisting()).toBe(false);
    await snap("pension-simulator-error");
  });
});
