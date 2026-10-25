import { $, $$, browser, expect } from "@wdio/globals";
import {
  byId,
  clickCentered,
  enterWorkspace,
  pickOption,
  seedLossScenario,
  simulateIvvbSale,
  sql,
  textOf,
  waitForText,
} from "./helpers.mjs";

// `$$` devolve um objeto cujo `.map` não é o de array (devolve uma promessa
// encadeada), então vira array de verdade antes de mapear.
async function elementsOf(selector) {
  return Array.from(await $$(selector));
}

async function testIds(selector) {
  const elements = await elementsOf(selector);
  const ids = await Promise.all(elements.map((el) => el.getAttribute("data-testid")));
  return ids.sort();
}

// Navegação (Fase 20), Settings → Storage (Fase 15), regras fiscais editáveis
// (Fase 24) e a Home (Fase 17).
describe("Anchor — navegação, Settings e regras fiscais", () => {
  it("a navegação principal não tem aba de IA e a Home mostra seus widgets", async () => {
    await enterWorkspace("Settings E2E");

    // Fase 20: sem chat/aba de IA. Fase 35: mais a aba Pending.
    expect(await testIds('[data-testid^="nav-"]')).toEqual([
      "nav-alerts",
      "nav-finances",
      "nav-home",
      "nav-lookup",
      "nav-pending",
      "nav-portfolio",
    ]);

    // Fase 17: o widget de juros existe na Home (os dados vêm do BCB e da B3, que
    // o teste não consulta, então só conferimos que o widget está lá).
    await waitForText(
      $("body"),
      (text) => text.includes("Total portfolio value") && text.includes("Interest rates"),
      "a Home deveria mostrar os widgets de valor total e de juros",
    );
  });

  it("Settings tem as seções esperadas e nenhuma de IA", async () => {
    await byId("open-settings").click();
    await byId("settings-back").waitForDisplayed({ timeout: 20_000 });

    const sections = await elementsOf('[data-testid^="settings-section-"]');
    const names = await Promise.all(sections.map((el) => textOf(el)));
    expect(names).toEqual(["TruthID", "Data sources", "Storage", "Backup", "Taxes", "Currency", "Updates"]);
  });

  it("Currency: a moeda base começa em BRL e a troca fica gravada", async () => {
    await byId("settings-section-Currency").click();
    await waitForText(
      byId("base-currency-select"),
      (text) => text.includes("BRL"),
      "sem configuração gravada, a moeda base deveria ser BRL",
    );

    // Nenhum ativo em ARS neste workspace, então a troca não consulta a rede. O ARS (e não
    // o USD) porque a lista abre alinhada ao BRL e o item do topo fica cortado pela janela.
    await pickOption(byId("base-currency-select"), "ARS — Argentine Peso");
    await waitForText(
      byId("base-currency-select"),
      (text) => text.includes("ARS"),
      "o seletor deveria mostrar a moeda escolhida",
    );
    await browser.waitUntil(() => sql("SELECT base_currency FROM workspace_settings") === "ARS", {
      timeout: 20_000,
      timeoutMsg: "a moeda base deveria ter sido gravada em workspace_settings",
    });

    // Volta para BRL para não afetar os casos seguintes.
    await pickOption(byId("base-currency-select"), "BRL — Brazilian Real");
    await browser.waitUntil(() => sql("SELECT base_currency FROM workspace_settings") === "BRL", {
      timeout: 20_000,
      timeoutMsg: "a moeda base deveria ter voltado para BRL",
    });
  });

  it("Data sources: lista a chave da bolsai e o e-mail da SEC sem expor valores", async () => {
    await byId("settings-section-Data sources").click();
    // Não salvamos nada: o CI não tem cofre (secret service) para o keyring.
    await waitForText(
      $("body"),
      (text) => text.includes("bolsai API key") && text.includes("SEC EDGAR contact email"),
      "a seção deveria listar as duas credenciais",
    );
  });

  it("Storage: o padrão é o disco local e as outras opções estão desabilitadas", async () => {
    await byId("settings-section-Storage").click();
    await waitForText(
      byId("storage-provider"),
      (text) => text.includes("Local disk"),
      "o provedor padrão deveria ser o disco local",
    );

    await byId("storage-provider").click();
    const options = await elementsOf('[role="option"]');
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

  // Fase 33. Sem ativos neste workspace a rodada só busca os benchmarks (rede real,
  // que pode falhar ou limitar); o que o teste garante é que a rodada é registrada
  // e termina, não o resultado das fontes.
  it("Updates: 'Update now' registra uma rodada manual e libera o botão", async function () {
    this.timeout(240_000);
    // A rodada baixa 8 fontes em sequência, e a Selic diária é a mais pesada (até 4 janelas de 45 s no BCB). O Home já a baixava
    // antes deste caso; desde a Sessão 236 ele não vai à rede com a ingestão desligada, então a Selic cairia aqui dentro. Uma
    // Selic gravada agora (fresca por 1 h) tira esse download do caminho, como no spec do porquinho; as demais fontes seguem reais.
    sql(
      "INSERT OR REPLACE INTO selic_daily (series_code, reference_date, value_pct, source, fetched_at) " +
        "VALUES ('meta', '2024-01-01', 10.10, 'test', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
    );
    await byId("open-settings").click();
    await byId("settings-section-Updates").click();
    await byId("ingestion-run-now").waitForDisplayed({ timeout: 20_000 });
    await byId("ingestion-run-now").click();

    await browser.waitUntil(
      async () => Number(sql("SELECT count(*) FROM ingestion_runs WHERE trigger_kind = 'manual'")) === 1,
      { timeout: 20_000, timeoutMsg: "a rodada manual deveria ter sido registrada" },
    );
    await waitForText(
      byId("ingestion-run-now"),
      (text) => text.includes("Update now"),
      "o botão deveria voltar quando a rodada terminasse",
      180_000,
    );
    const status = sql("SELECT status FROM ingestion_runs WHERE trigger_kind = 'manual'");
    expect(["ok", "partial", "error"]).toContain(status);
    await waitForText(byId("ingestion-runs"), (text) => text.includes("Manual"), "a rodada aparece no histórico");
  });
});
