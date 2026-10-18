import { $$, browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, snap, sql, textOf, waitForText } from "./helpers.mjs";

// Fase 35, entrega 6 — a aba Pending com as sugestões de provento. As três filas
// (carteira, extrato e proventos) aparecem juntas e o selo soma o que está acionável.
// As sugestões são semeadas direto no banco; o spec dirige confirmar com a quantidade
// editada, descartar e o selo descendo até zerar.

function seedSuggestion(portfolio, asset, status, date, amount, quantity) {
  sql(
    "INSERT INTO suggested_dividends (portfolio_id, asset_id, payment_date, amount, quantity, total, status, source, payment_type, created_at) " +
      `VALUES (${portfolio}, ${asset}, '${date}', ${amount}, ${quantity}, ${amount * quantity}, '${status}', 'manual', 'dividendo', '2026-10-19T00:00:00Z')`,
  );
}

const rows = async () => (await $$('[data-testid="dividend-pending-row"]')).length;
const badge = () => textOf(byId("pending-badge"));
const suggestionStatus = (id) => sql(`SELECT status FROM suggested_dividends WHERE id = ${id}`);

describe("Anchor — aba Pending com sugestões de provento", () => {
  it("lista só as sugestões acionáveis e o selo soma", async () => {
    await enterWorkspace("E2E Pending");
    const workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Pending'");
    sql(
      `INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`,
    );
    const portfolio = sql(`SELECT id FROM portfolio WHERE workspace_id = ${workspace}`);
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('PETR4', 'Petrobras', 'acao_br', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
    );
    const asset = sql("SELECT id FROM assets WHERE ticker = 'PETR4'");
    seedSuggestion(portfolio, asset, "pending", "2026-03-10", 0.5, 100);
    seedSuggestion(portfolio, asset, "matched", "2026-04-10", 0.4, 100);
    seedSuggestion(portfolio, asset, "divergent", "2026-05-10", 0.3, 100);
    seedSuggestion(portfolio, asset, "confirmed", "2026-01-10", 0.2, 100);
    seedSuggestion(portfolio, asset, "discarded", "2026-02-10", 0.1, 100);

    await byId("nav-pending").click();
    await byId("dividend-pending").waitForDisplayed({ timeout: 30_000 });
    await browser.waitUntil(async () => (await rows()) === 3, {
      timeout: 20_000,
      timeoutMsg: "as três sugestões acionáveis deveriam aparecer",
    });
    expect(await textOf(byId("dividend-pending"))).toContain("Dividend suggestions (3)");
    await byId("pending-badge").waitForDisplayed({ timeout: 10_000 });
    expect(await badge()).toBe("3");
    await snap("pending-tab-with-badge");
  });

  it("confirma a primeira com a quantidade editada e grava o provento", async () => {
    await clickCentered(byId("dividend-confirm"));
    await byId("dividend-confirm-form").waitForDisplayed({ timeout: 20_000 });
    await byId("dividend-quantity").setValue("80");
    await byId("dividend-confirm-submit").click();

    await browser.waitUntil(() => suggestionStatus(1) === "confirmed", {
      timeout: 20_000,
      timeoutMsg: "a sugestão deveria estar confirmada",
    });
    expect(sql("SELECT quantity FROM transactions WHERE transaction_type = 'provento'")).toBe("80.0");
    expect(sql("SELECT total_value FROM transactions WHERE transaction_type = 'provento'")).toBe("40.0");
    await browser.waitUntil(async () => (await rows()) === 2, { timeout: 10_000 });
    await waitForText(byId("pending-badge"), (t) => t === "2", "selo em 2");
  });

  it("descarta as outras duas e o selo some", async () => {
    await clickCentered(byId("dividend-discard"));
    await browser.waitUntil(async () => (await rows()) === 1, { timeout: 10_000 });
    await waitForText(byId("pending-badge"), (t) => t === "1", "selo em 1");

    await clickCentered(byId("dividend-discard"));
    await byId("dividend-pending-empty").waitForDisplayed({ timeout: 10_000 });
    await byId("pending-badge").waitForDisplayed({ reverse: true, timeout: 10_000 });
    expect(suggestionStatus(2)).toBe("discarded");
    expect(suggestionStatus(3)).toBe("discarded");
    expect(sql("SELECT count(*) FROM transactions WHERE transaction_type = 'provento'")).toBe("1");
  });

  // Descarte em lote: seleção por linha, confirmação em dois cliques e "Select all".
  it("descarta duas selecionadas de uma vez, com confirmação, e depois o resto por Select all", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main'");
    const asset = sql("SELECT id FROM assets WHERE ticker = 'PETR4'");
    for (const day of ["11", "12", "13", "14"]) seedSuggestion(portfolio, asset, "pending", `2026-06-${day}`, 0.5, 100);
    const ids = sql("SELECT id FROM suggested_dividends WHERE status = 'pending' ORDER BY id").split("\n");
    expect(ids.length).toBe(4);

    // As sugestões entraram pelo banco: sair e voltar para a lista recarregar.
    await byId("nav-home").click();
    await byId("nav-pending").click();
    await browser.waitUntil(async () => (await rows()) === 4, { timeout: 20_000, timeoutMsg: "as quatro novas aparecem" });

    // Sem seleção o botão fica desligado.
    expect(await byId("dividend-pending-discard-selected").isEnabled()).toBe(false);

    await clickCentered(byId(`dividend-pending-select-${ids[0]}`));
    await clickCentered(byId(`dividend-pending-select-${ids[1]}`));
    await waitForText(byId("dividend-pending-discard-selected"), (t) => t === "Discard selected (2)", "dois selecionados");

    // Primeiro clique só pede confirmação; nada é descartado ainda.
    await clickCentered(byId("dividend-pending-discard-selected"));
    await waitForText(byId("dividend-pending-discard-selected"), (t) => t === "Confirm discarding 2?", "pede confirmação");
    await snap("pending-bulk-discard-confirm");
    expect(suggestionStatus(ids[0])).toBe("pending");

    await clickCentered(byId("dividend-pending-discard-selected"));
    await browser.waitUntil(async () => (await rows()) === 2, { timeout: 20_000, timeoutMsg: "duas saem da lista" });
    expect(suggestionStatus(ids[0])).toBe("discarded");
    expect(suggestionStatus(ids[1])).toBe("discarded");
    expect(suggestionStatus(ids[2])).toBe("pending");
    await waitForText(byId("pending-badge"), (t) => t === "2", "selo em 2");

    // Select all pega o que sobrou.
    await clickCentered(byId("dividend-pending-select-all"));
    await waitForText(byId("dividend-pending-discard-selected"), (t) => t === "Discard selected (2)", "tudo selecionado");
    await clickCentered(byId("dividend-pending-discard-selected"));
    await clickCentered(byId("dividend-pending-discard-selected"));
    await byId("dividend-pending-empty").waitForDisplayed({ timeout: 20_000 });
    expect(suggestionStatus(ids[3])).toBe("discarded");
    await byId("pending-badge").waitForDisplayed({ reverse: true, timeout: 10_000 });
  });
});
