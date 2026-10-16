import { $, $$, browser, expect } from "@wdio/globals";
import {
  byId,
  clickCentered,
  enterWorkspace,
  seedLossScenario,
  simulateIvvbSale,
  sql,
  textOf,
  waitForText,
} from "./helpers.mjs";

async function testIds(selector) {
  const elements = await $$(selector);
  const ids = await Promise.all(elements.map((el) => el.getAttribute("data-testid")));
  return ids.sort();
}

// Navegação (Fase 20), Settings → Storage (Fase 15), regras fiscais editáveis
// (Fase 24) e a Home (Fase 17).
describe("Anchor — navegação, Settings e regras fiscais", () => {
  it("a navegação principal não tem aba de IA e a Home mostra seus widgets", async () => {
    await enterWorkspace("Settings E2E");

    // Fase 20: sem chat/aba de IA, só as 5 abas de sempre.
    expect(await testIds('[data-testid^="nav-"]')).toEqual([
      "nav-alerts",
      "nav-finances",
      "nav-home",
      "nav-lookup",
      "nav-portfolio",
    ]);

    // Fase 17: o widget de juros existe na Home (os dados vêm do Lume, ausente
    // aqui, então só conferimos que o widget está lá).
    await waitForText(
      $("body"),
      (text) => text.includes("Total portfolio value") && text.includes("Interest rates"),
      "a Home deveria mostrar os widgets de valor total e de juros",
    );
  });

  it("Settings tem as seções esperadas e nenhuma de IA", async () => {
    await byId("open-settings").click();
    await byId("settings-back").waitForDisplayed({ timeout: 20_000 });

    const sections = await $$('[data-testid^="settings-section-"]');
    const names = await Promise.all(sections.map((el) => textOf(el)));
    expect(names).toEqual(["TruthID", "Finance API", "Storage", "Backup", "Taxes"]);
  });

  it("Storage: o padrão é o disco local e as outras opções estão desabilitadas", async () => {
    await byId("settings-section-Storage").click();
    await waitForText(
      byId("storage-provider"),
      (text) => text.includes("Local disk"),
      "o provedor padrão deveria ser o disco local",
    );

    await byId("storage-provider").click();
    const options = await $$('[role="option"]');
    expect(options.length).toBe(4);
    const disabled = await Promise.all(options.map((o) => o.getAttribute("aria-disabled")));
    expect(disabled.filter((d) => d === "true").length).toBe(3);
    await browser.keys("Escape");

    // Salvar o padrão não pode dar erro, e o valor persiste ao reabrir.
    await byId("storage-save").click();
    await byId("settings-back").click();
    await byId("open-settings").click();
    await byId("settings-section-Storage").click();
    await waitForText(
      byId("storage-provider"),
      (text) => text.includes("Local disk"),
      "depois de salvar, o disco local continua selecionado",
    );
  });

  it("editar a alíquota em Settings muda a simulação de venda, e o Reset desfaz", async () => {
    seedLossScenario("Settings E2E");
    const ruleId = sql(
      "SELECT id FROM tax_rules WHERE rule_key = 'common_rate' AND effective_from <= date('now') " +
        "ORDER BY effective_from DESC LIMIT 1",
    );

    await byId("settings-section-Taxes").click();
    const value = byId(`tax-value-${ruleId}`);
    await waitForText(value, (text) => text === "15%", "a alíquota de fábrica é 15%");

    await clickCentered(byId(`tax-edit-${ruleId}`));
    await byId(`tax-input-${ruleId}`).setValue("20");
    await byId(`tax-save-${ruleId}`).click();
    await waitForText(value, (text) => text === "20%", "a alíquota editada deveria ser 20%");
    await waitForText(
      byId(`tax-rule-${ruleId}`),
      (text) => text.includes("edited by you"),
      "a regra deveria marcar que foi editada por você",
    );

    // A simulação passa a usar os 20%: imposto sobre os R$ 500 restantes = R$ 100.
    await byId("settings-back").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();
    await simulateIvvbSale();
    await waitForText(
      byId("sim-month-tax"),
      (text) => text.includes("100,00"),
      "com 20%, o imposto sobre R$ 500,00 deveria ser R$ 100,00",
    );

    // Reset: volta aos 15% e a simulação volta a R$ 75,00.
    await byId("open-settings").click();
    await byId("settings-section-Taxes").click();
    await clickCentered(byId(`tax-reset-${ruleId}`));
    await waitForText(byId(`tax-value-${ruleId}`), (text) => text === "15%", "o Reset volta aos 15%");

    await byId("settings-back").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();
    await simulateIvvbSale();
    await waitForText(
      byId("sim-month-tax"),
      (text) => text.includes("75,00"),
      "depois do Reset o imposto volta a R$ 75,00",
    );
  });
});
