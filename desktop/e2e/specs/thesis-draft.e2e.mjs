import { $, browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql } from "./helpers.mjs";

// Rascunho de tese a partir dos dados (ROADMAP item 33, frente 3): o botão "Draft from data" do
// diálogo de nova tese preenche título e Markdown com o retrato datado dos números já coletados,
// sem salvar nada. TEST3: preço 25,00 e LPA 2,00 dão P/L 12,50. O formato do texto e os casos de
// borda (campo ausente, último valuation de cada modelo) são cobertos pelos testes Rust.

const dialogInput = () => $('[role="dialog"] input');
const dialogTextarea = () => $('[role="dialog"] textarea');

describe("Anchor — rascunho de tese a partir dos dados", () => {
  it("abre a nova tese de TEST3 com preço e fundamentos coletados", async () => {
    await byId("workspace-name").waitForDisplayed({ timeout: 60_000 });
    await enterWorkspace("Thesis Draft E2E");
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('TEST3', 25.0, 'e2e', '2026-11-01T00:00:00Z', 'Test SA', 'SAO', 'BRL')",
    );
    sql(
      "INSERT INTO stock_fundamentals (ticker, lpa, vpa, roe, source, fetched_at) " +
        "VALUES ('TEST3', 2.0, 10.0, 15.0, 'e2e', '2026-11-01T00:00:00Z')",
    );

    await byId("nav-lookup").click();
    await pickOption(byId("global-search-class"), "Stock (B3)");
    await byId("global-search-ticker").setValue("TEST3");
    await byId("global-search-submit").click();

    const open = await $('//button[normalize-space(.)="New thesis"]');
    await open.waitForDisplayed({ timeout: 30_000 });
    await clickCentered(open);
    await byId("thesis-draft-button").waitForDisplayed({ timeout: 10_000 });
  });

  it("Draft from data preenche o título e o retrato dos números", async () => {
    await byId("thesis-draft-button").click();

    await browser.waitUntil(async () => (await dialogInput().getValue()) === "TEST3 thesis", {
      timeout: 20_000,
      timeoutMsg: "o título do rascunho não apareceu",
    });
    const text = await dialogTextarea().getValue();
    expect(text).toContain("## Snapshot (");
    expect(text).toContain("- P/E: 12.50");
    expect(text).toContain("No saved valuation (n/d).");
    expect(text).toContain("## Why own it");
  });

  it("com texto já digitado, o botão pede confirmação antes de substituir", async () => {
    await dialogTextarea().setValue("minha anotação");

    await byId("thesis-draft-button").click();
    expect(await byId("thesis-draft-button").getText()).toBe("Replace current text?");
    expect(await dialogTextarea().getValue()).toBe("minha anotação");

    await byId("thesis-draft-button").click();
    await browser.waitUntil(async () => (await dialogTextarea().getValue()).includes("## Snapshot ("), {
      timeout: 20_000,
      timeoutMsg: "o rascunho não substituiu o texto depois da confirmação",
    });
  });
});
