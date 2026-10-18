import { browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, sql, textOf, waitForText } from "./helpers.mjs";

// Fase 35, entrega 10 — estimativa do rendimento do porquinho por resgate. A conta, as pernas do extrato e a
// Selic meta são semeadas no banco (a Selic com `fetched_at` de agora, para o app não ir ao BCB); o spec dirige
// o diálogo, a fila de Pending e o texto do popup. Dados inventados.

let workspace;
let pocket;

const pendingYields = () => Number(sql("SELECT count(*) FROM import_pending WHERE kind = 'fixed_income_yield' AND status = 'pending'"));

describe("Anchor — estimativa do rendimento do porquinho", () => {
  it("semeia a conta com uma aplicação, um resgate e a Selic", async () => {
    await enterWorkspace("E2E Estimate");
    workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Estimate'");
    sql(
      "INSERT INTO bank_accounts (workspace_id, nome, titular, currency, created_at) " +
        `VALUES (${workspace}, 'Cdb Porquinho', 'Test holder', 'BRL', '2026-01-01T00:00:00Z')`,
    );
    pocket = sql(`SELECT id FROM bank_accounts WHERE workspace_id = ${workspace}`);
    const leg = (type, valor, date) =>
      sql(
        "INSERT INTO general_transactions (workspace_id, bank_account_id, transaction_type, valor, transaction_date, created_at) " +
          `VALUES (${workspace}, ${pocket}, '${type}', ${valor}, '${date}', '${date}T00:00:00Z')`,
      );
    leg("transferencia_entrada", 1000, "2025-03-03");
    leg("transferencia_saida", 250, "2025-06-03");
    sql(
      "INSERT INTO selic_daily (series_code, reference_date, value_pct, source, fetched_at) " +
        "VALUES ('meta', '2024-01-01', 10.10, 'test', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
    );
    expect(pendingYields()).toBe(0);
  });

  it("estimar manda uma proposta para a fila de Pending", async () => {
    await byId("nav-finances").click();
    await byId("finances-tab-accounts").click();
    await clickCentered(byId("account-estimate"));

    await byId("estimate-pct").waitForDisplayed({ timeout: 20_000 });
    expect(await byId("estimate-pct").getValue()).toBe("100");
    await byId("estimate-run").click();

    await waitForText(
      byId("estimate-result"),
      (text) => text.includes("1 estimate(s) sent to Pending out of 1 redemption(s)"),
      "a estimativa deveria enfileirar o único resgate",
    );
    expect(pendingYields()).toBe(1);

    // Rodar de novo não duplica: o dia já tem item na fila.
    await byId("estimate-run").click();
    await waitForText(
      byId("estimate-result"),
      (text) => text.includes("0 estimate(s) sent") && text.includes("already has a yield"),
      "a segunda rodada deveria pular o dia que já está na fila",
    );
    expect(pendingYields()).toBe(1);
  });

  it("o popup diz que é estimativa e a aprovação deixa o principal na conta", async () => {
    await browser.keys("Escape");
    await byId("nav-pending").click();
    await clickCentered(byId("statement-review-open"));

    await byId("yield-original").waitForDisplayed({ timeout: 20_000 });
    const text = await textOf(byId("yield-original"));
    expect(text).toContain("estimated from the CDI curve at 100% of the CDI");
    expect(await textOf(byId("yield-iof-note"))).toContain("Estimated from the applications");
    // Três meses a 10% ao ano sobre cerca de 250: um rendimento bruto de poucos reais, com IR de 22,5% e sem IOF.
    const gross = Number(await byId("yield-gross").getValue());
    expect(gross).toBeGreaterThan(3);
    expect(gross).toBeLessThan(9);
    expect(await byId("yield-iof").getValue()).toBe("0");

    await byId("statement-correct").click();
    await browser.waitUntil(() => pendingYields() === 0, { timeout: 20_000, timeoutMsg: "a proposta deveria ter sido aprovada" });
    expect(Number(sql(`SELECT count(*) FROM general_transactions WHERE bank_account_id = ${pocket} AND transaction_type = 'receita'`))).toBe(1);
    expect(Number(sql(`SELECT count(*) FROM general_transactions WHERE bank_account_id = ${pocket} AND transaction_type = 'despesa'`))).toBe(1);
  });
});
