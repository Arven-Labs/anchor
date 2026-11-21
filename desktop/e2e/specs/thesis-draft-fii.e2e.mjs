import { $, browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql } from "./helpers.mjs";

// Rascunho de tese de FII (ROADMAP item 33, frente 3, 2ª fatia): na tela de Research do FII, o
// botão "Draft from data" do diálogo de nova tese preenche título e Markdown com cota, P/VP e
// indicadores da CVM já coletados, sem salvar nada. TESTE11: preço 160,00 e VP da cota 200,00 dão
// P/VP 0,80. O formato e os casos de borda são cobertos pelos testes Rust.

const CNPJ = "11.111.111/0001-11";
const dialogInput = () => $('[role="dialog"] input');
const dialogTextarea = () => $('[role="dialog"] textarea');

describe("Anchor — rascunho de tese de FII", () => {
  it("abre a nova tese de TESTE11 com cota e indicadores da CVM coletados", async () => {
    await byId("workspace-name").waitForDisplayed({ timeout: 60_000 });
    await enterWorkspace("FII Thesis Draft E2E");
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('TESTE11', 160.0, 'e2e', '2026-11-01T00:00:00Z', 'Fundo Teste', 'SAO', 'BRL')",
    );
    sql(
      "INSERT INTO fii_cnpj_cache (ticker, cnpj, fund_name, resolved_at) " +
        `VALUES ('TESTE11', '${CNPJ}', 'Fundo Teste', '2026-11-01T00:00:00Z')`,
    );
    sql(
      "INSERT INTO fii_cvm_monthly (cnpj, reference_date, patrimonio_liquido, valor_patrimonial_cota, " +
        "numero_cotistas, dividend_yield_mes, rentabilidade_efetiva_mes, source, fetched_at) " +
        `VALUES ('${CNPJ}', '2026-10-31', 3000000000.0, 200.0, 150000, 0.008, 0.012, 'e2e', '2026-11-01T00:00:00Z')`,
    );
    sql(
      "INSERT INTO fii_cvm_properties (cnpj, reference_date, nome_imovel, area_m2, percentual_vacancia, " +
        "percentual_inadimplencia, percentual_locado, source, fetched_at) " +
        `VALUES ('${CNPJ}', '2026-10-31', 'Galpão A', 100.0, 0.10, 0.0, 0.90, 'e2e', '2026-11-01T00:00:00Z')`,
    );

    await byId("nav-lookup").click();
    await pickOption(byId("global-search-class"), "FII (B3)");
    await byId("global-search-ticker").setValue("TESTE11");
    await byId("global-search-submit").click();

    const open = await $('//button[normalize-space(.)="New thesis"]');
    await open.waitForDisplayed({ timeout: 30_000 });
    await clickCentered(open);
    await byId("thesis-draft-button").waitForDisplayed({ timeout: 10_000 });
  });

  it("Draft from data preenche o título e o retrato do fundo", async () => {
    await byId("thesis-draft-button").click();

    await browser.waitUntil(async () => (await dialogInput().getValue()) === "TESTE11 thesis", {
      timeout: 20_000,
      timeoutMsg: "o título do rascunho de FII não apareceu",
    });
    const text = await dialogTextarea().getValue();
    expect(text).toContain("## Snapshot (");
    expect(text).toContain("CVM reference 2026-10-31.");
    expect(text).toContain("- P/VP: 0.80");
    expect(text).toContain("- Vacancy (area-weighted): 10.0%");
    expect(text).toContain("## Why own it");
  });
});
