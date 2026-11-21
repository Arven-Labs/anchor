import { $, browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql } from "./helpers.mjs";

// Rascunho de tese de ETF (ROADMAP item 33, frente 3, 3ª fatia): na tela de Research do ETF, o
// botão "Draft from data" do diálogo de nova tese preenche título e Markdown com cotação, médias
// móveis, CAGR e dividendo médio já coletados, sem salvar nada. O BDR usa o mesmo comando e é
// coberto pelos testes Rust; o formato e os casos de borda também.

const dialogInput = () => $('[role="dialog"] input');
const dialogTextarea = () => $('[role="dialog"] textarea');

describe("Anchor — rascunho de tese de ETF", () => {
  it("abre a nova tese de TESTE11 com cotação e técnicos coletados", async () => {
    await byId("workspace-name").waitForDisplayed({ timeout: 60_000 });
    await enterWorkspace("ETF Thesis Draft E2E");
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('TESTE11', 130.0, 'e2e', '2026-11-01T00:00:00Z', 'ETF Teste', 'SAO', 'BRL')",
    );
    sql(
      "INSERT INTO stock_technicals (ticker, sma_50, sma_100, sma_200, cagr_5y, cagr_10y, source, fetched_at) " +
        "VALUES ('TESTE11', 128.5, 125.0, 120.25, 12.5, 9.0, 'e2e', '2026-11-01T00:00:00Z')",
    );
    sql(
      "INSERT INTO stock_dividends_avg (ticker, avg_dividend_5y, source, fetched_at) " +
        "VALUES ('TESTE11', 1.2, 'e2e', '2026-11-01T00:00:00Z')",
    );

    await byId("nav-lookup").click();
    await pickOption(byId("global-search-class"), "ETF (B3)");
    await byId("global-search-ticker").setValue("TESTE11");
    await byId("global-search-submit").click();

    const open = await $('//button[normalize-space(.)="New thesis"]');
    await open.waitForDisplayed({ timeout: 30_000 });
    await clickCentered(open);
    await byId("thesis-draft-button").waitForDisplayed({ timeout: 10_000 });
  });

  it("Draft from data preenche o título e o retrato do ETF", async () => {
    await byId("thesis-draft-button").click();

    await browser.waitUntil(async () => (await dialogInput().getValue()) === "TESTE11 thesis", {
      timeout: 20_000,
      timeoutMsg: "o título do rascunho de ETF não apareceu",
    });
    const text = await dialogTextarea().getValue();
    expect(text).toContain("## Snapshot (");
    expect(text).toContain("- Class: ETF");
    expect(text).toContain("- Price: 130.00 BRL");
    expect(text).toContain("- SMA 50: 128.50");
    expect(text).toContain("- CAGR 5y: 12.5%");
    expect(text).toContain("## Why own it");
  });
});
