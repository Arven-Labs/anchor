import { $, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql } from "./helpers.mjs";

// Premissas sugeridas nos valuations (Sessão 370, 1ª fatia): a taxa livre de
// risco aparece ao lado do campo, com fonte e data, e só entra no formulário
// quando o usuário clica em "Use". No E2E não há rede (ANCHOR_DISABLE_INGESTION),
// então não há curva DI: o que se exercita aqui é a Selic de reserva. A regra
// "DI de 10 anos primeiro" é coberta pelos testes Rust de `risk_free_suggestion`.
//
// A Selic é semeada ANTES de entrar no workspace: o widget da Home já busca os
// juros ao entrar, e o resultado fica em cache por 30 minutos.

const modelSelect = () =>
  $('//label[normalize-space(.)="Valuation model"]/following-sibling::button[@role="combobox"]');
const input = (label) => $(`//label[normalize-space(.)="${label}"]/following-sibling::input`);

// O formulário é longo: depois de clicar em "Use" ou digitar num campo, a página fica rolada e o
// seletor de modelo (no topo) deixa de ser clicável ("element not interactable"). Rolar antes de abrir.
async function selectModel(label) {
  await (await modelSelect()).scrollIntoView({ block: "center", inline: "center" });
  await pickOption(modelSelect(), label);
}

async function fill(label, value) {
  const field = await input(label);
  await field.scrollIntoView({ block: "center" });
  await field.setValue(value);
}

describe("Anchor — taxa livre de risco sugerida nos valuations", () => {
  it("abre o popup New Valuation de TEST3 com a Selic gravada", async () => {
    await byId("workspace-name").waitForDisplayed({ timeout: 60_000 });
    sql(
      "INSERT INTO selic_daily (series_code, reference_date, value_pct, source, fetched_at) " +
        "VALUES ('meta', '2026-06-18', 10.50, 'e2e', '2026-06-18T00:00:00Z')",
    );
    await enterWorkspace("Risk Free E2E");
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
  });

  it("DCF: mostra a taxa livre com a fonte e a data, sem preencher o campo", async () => {
    await selectModel("DCF / FCFF");

    const hint = byId("risk-free-hint");
    await hint.waitForDisplayed({ timeout: 20_000 });
    const text = await hint.getText();
    expect(text).toContain("10.50%");
    expect(text).toContain("Selic");
    expect(text).toContain("18/06/2026");

    expect(await input("Risk-free rate — Rf (%)").getValue()).toBe("");
  });

  it("DCF: clicar em Use preenche só a taxa livre de risco", async () => {
    await byId("risk-free-use").click();

    expect(await input("Risk-free rate — Rf (%)").getValue()).toBe("10.5");
    expect(await input("Beta").getValue()).toBe("");
    expect(await input("Market risk premium (%)").getValue()).toBe("");
  });

  it("Gordon: Ke abaixo da taxa livre mostra o aviso e continua editável", async () => {
    await selectModel("Gordon / DDM");
    await byId("risk-free-hint").waitForDisplayed({ timeout: 20_000 });

    await fill("Required return — Ke (%)", "8");
    await byId("ke-below-risk-free").waitForDisplayed({ timeout: 10_000 });
    expect(await input("Required return — Ke (%)").getValue()).toBe("8");
  });

  it("Gordon: Ke acima da taxa livre não mostra o aviso", async () => {
    await fill("Required return — Ke (%)", "14");
    expect(await byId("ke-below-risk-free").isExisting()).toBe(false);
  });

  it("Gordon: o hint não preenche o Ke (não há botão Use nele)", async () => {
    expect(await byId("risk-free-use").isExisting()).toBe(false);
  });
});
