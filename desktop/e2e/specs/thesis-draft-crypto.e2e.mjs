import { $, browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql } from "./helpers.mjs";

// Rascunho de tese de cripto (ROADMAP item 33, frente 3, PR B): na tela de Research da moeda, o
// botão "Draft from data" do diálogo de nova tese preenche título e Markdown com cotação, médias
// móveis, variações, Fear & Greed e score já coletados, sem salvar nada e sem rede (a cotação
// semeada evita o coletor automático). Formato e casos de borda estão nos testes Rust.

const dialogInput = () => $('[role="dialog"] input');
const dialogTextarea = () => $('[role="dialog"] textarea');

describe("Anchor — rascunho de tese de cripto", () => {
  it("abre a nova tese de TCRIPTO com cotação, histórico e Fear & Greed coletados", async () => {
    await byId("workspace-name").waitForDisplayed({ timeout: 60_000 });
    await enterWorkspace("Crypto Thesis Draft E2E");
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('TCRIPTO', 65000.0, 'e2e', '2026-11-01T00:00:00Z', 'Moeda Teste', 'CoinGecko', 'USD')",
    );
    sql(
      "INSERT INTO stock_price_history (ticker, price_date, close_price, source, fetched_at) " +
        "VALUES ('TCRIPTO', date('now', '-7 days'), 100.0, 'e2e', '2026-11-01T00:00:00Z'), " +
        "('TCRIPTO', date('now'), 110.0, 'e2e', '2026-11-01T00:00:00Z')",
    );
    sql(
      "INSERT INTO crypto_fear_greed (value, classification, reading_date, source, fetched_at) " +
        "VALUES (72, 'Greed', '2026-11-19', 'e2e', '2026-11-19T00:00:00Z')",
    );

    await byId("nav-lookup").click();
    await pickOption(byId("global-search-class"), "Crypto");
    await byId("global-search-ticker").setValue("TCRIPTO");
    await byId("global-search-submit").click();

    const open = await $('//button[normalize-space(.)="New thesis"]');
    await open.waitForDisplayed({ timeout: 30_000 });
    await clickCentered(open);
    await byId("thesis-draft-button").waitForDisplayed({ timeout: 10_000 });
  });

  it("Draft from data preenche o título e o retrato da moeda", async () => {
    await byId("thesis-draft-button").click();

    await browser.waitUntil(async () => (await dialogInput().getValue()) === "TCRIPTO thesis", {
      timeout: 20_000,
      timeoutMsg: "o título do rascunho de cripto não apareceu",
    });
    const text = await dialogTextarea().getValue();
    expect(text).toContain("## Snapshot (");
    expect(text).toContain("- Price: 65000.00 USD");
    expect(text).toContain("- Change 7d: +10.0%");
    expect(text).toContain("- SMA 200: n/d");
    expect(text).toContain("- Fear & Greed: 72 (Greed), read on 2026-11-19");
    expect(text).toContain("- Crypto score: n/d");
    expect(text).toContain("## Why own it");
  });
});
