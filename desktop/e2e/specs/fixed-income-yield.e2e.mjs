import { $$, browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql, textOf, waitForText } from "./helpers.mjs";

// Fase 35, entrega 4 — rendimento do porquinho derivado da Movimentação da B3. O
// diálogo nativo de arquivo não é dirigível pelo WebDriver, então a fila é semeada no
// banco (o mesmo JSON que `stage_fixed_income_yield` grava), junto com as pernas do
// extrato que o porquinho recebeu, e o spec dirige o que vem depois: a checagem contra o
// extrato do dia, editar o IR, o papel isento (sem linha de IR) e descartar. Dados inventados.

function seedYield(workspace, account, item) {
  const proposed = {
    line: 2,
    quantity: 5000,
    principal: 50,
    gross_value: 50.4,
    gross_yield: 0.4,
    base_unit_price: 0.01,
    base_price_source: "application",
    ...item,
  };
  sql(
    "INSERT INTO import_pending (workspace_id, bank_account_id, kind, source, payload, status, created_at) " +
      `VALUES (${workspace}, ${account}, 'fixed_income_yield', 'b3_fixed_income', '${JSON.stringify({ proposed })}', 'pending', '2026-10-19T00:00:00Z')`,
  );
}

const ledger = (account, type) =>
  Number(sql(`SELECT count(*) FROM general_transactions WHERE bank_account_id = ${account} AND transaction_type = '${type}'`));
const pendingCount = (status) =>
  Number(sql(`SELECT count(*) FROM import_pending WHERE kind = 'fixed_income_yield' AND status = '${status}'`));
const balance = (account) =>
  Number(
    sql(
      "SELECT coalesce(sum(CASE WHEN transaction_type IN ('despesa','transferencia_saida') THEN -valor ELSE valor END), 0) " +
        `FROM general_transactions WHERE bank_account_id = ${account}`,
    ),
  );

let workspace;
let pocket;

describe("Anchor — rendimento do porquinho pela Movimentação da B3", () => {
  it("semeia o extrato do porquinho e três rendimentos pendentes", async () => {
    await enterWorkspace("E2E Yield");
    workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Yield'");
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
    // O que o extrato aprovado já registrou: aplicou 100,00; resgatou 50,08 em 05/03 e 50,20 em 12/03.
    leg("transferencia_entrada", 100, "2026-03-01");
    leg("transferencia_saida", 50.08, "2026-03-05");
    leg("transferencia_saida", 50.2, "2026-03-12");

    const cdb = { instrument: "cdb", code: "CDBAAA", ir_rate: 0.225, ir_amount: 0.09, net_yield: 0.31 };
    // Aplicado 4 dias antes: o IOF leva 0,30 do rendimento bruto de 0,40; o IR de 22,5% sobre 0,10 é 0,02.
    seedYield(workspace, pocket, {
      ...cdb,
      iof_amount: 0.3,
      holding_days: 4,
      iof_basis: "full",
      ir_amount: 0.02,
      net_yield: 0.08,
      transaction_date: "2026-03-05",
      day_derived_total: 50.08,
    });
    seedYield(workspace, pocket, {
      instrument: "lci",
      code: "LCIAAA",
      iof_basis: "not_applicable",
      ir_rate: 0,
      ir_amount: 0,
      net_yield: 0.4,
      transaction_date: "2026-03-09",
      day_derived_total: 50.4,
    });
    seedYield(workspace, pocket, { ...cdb, transaction_date: "2026-03-12", day_derived_total: 50.31 });

    await byId("nav-pending").click();
    await byId("statement-review-open").waitForDisplayed({ timeout: 30_000 });
    expect(await textOf(byId("statement-pending"))).toContain("Statement pending review (3)");
    expect(balance(pocket)).toBeCloseTo(-0.28, 2);
  });

  it("o botão de importar o rendimento só liga depois de escolher a conta", async () => {
    // O painel de importar continua em Finances.
    await byId("nav-finances").click();
    await byId("finances-tab-transactions").waitForDisplayed({ timeout: 30_000 });
    await byId("finances-tab-transactions").click();
    await byId("yield-import-button").waitForDisplayed({ timeout: 30_000 });
    expect(await byId("yield-import-button").isEnabled()).toBe(false);
    await pickOption(byId("yield-account"), "Cdb Porquinho");
    expect(await byId("yield-import-button").isEnabled()).toBe(true);

    await byId("nav-pending").click();
    await byId("statement-review-open").waitForDisplayed({ timeout: 30_000 });
  });

  it("o CDB recente mostra o IOF e a checagem que bate, e editar o IR muda o líquido", async () => {
    await clickCentered(byId("statement-review-open"));
    await byId("yield-gross").waitForDisplayed({ timeout: 20_000 });
    expect(await textOf(byId("yield-original"))).toContain("CDB CDBAAA");
    expect(await textOf(byId("yield-iof-note"))).toContain("Held 4 day(s)");
    expect(await byId("yield-iof").getValue()).toBe("0.3");
    expect(await textOf(byId("yield-check"))).toContain("exactly what the B3 file explains");
    expect(await textOf(byId("yield-net"))).toBe("0.08");

    await byId("yield-ir").setValue("0.05");
    await waitForText(byId("yield-net"), (t) => t === "0.05", "líquido = 0,40 − 0,30 − 0,05");
    await byId("statement-correct").click();

    // Três linhas: receita bruta, despesa de IOF e despesa de IR, as duas na categoria Taxes.
    await browser.waitUntil(() => ledger(pocket, "receita") === 1, { timeout: 20_000, timeoutMsg: "a receita bruta deveria estar gravada" });
    expect(ledger(pocket, "despesa")).toBe(2);
    expect(sql("SELECT round(sum(valor), 2) FROM general_transactions WHERE transaction_type = 'despesa'")).toBe("0.35");
    expect(
      sql(
        "SELECT group_concat(DISTINCT c.nome) FROM general_transactions g JOIN general_transaction_categories c ON c.id = g.category_id WHERE g.transaction_type = 'despesa'",
      ),
    ).toBe("Taxes");
    expect(Number(sql("SELECT count(*) FROM general_transactions WHERE notes LIKE 'IOF%'"))).toBe(1);
  });

  it("a LCI é isenta: uma linha só e nenhuma checagem sem extrato do dia", async () => {
    await browser.waitUntil(async () => (await textOf(byId("yield-original"))).includes("LCI LCIAAA"), {
      timeout: 10_000,
      timeoutMsg: "o segundo item é a LCI",
    });
    expect(await byId("yield-ir").getValue()).toBe("0");
    expect(await byId("yield-iof").getValue()).toBe("0");
    expect(await textOf(byId("yield-iof-note"))).toContain("No IOF for this instrument");
    expect((await $$('[data-testid="yield-check"]')).length).toBe(0);
    await byId("statement-correct").click();
    await browser.waitUntil(() => ledger(pocket, "receita") === 2, { timeout: 20_000, timeoutMsg: "a receita da LCI deveria estar gravada" });
    expect(ledger(pocket, "despesa")).toBe(2);
  });

  it("quando o banco pagou menos que a B3 explica, avisa e deixa descartar", async () => {
    await browser.waitUntil(async () => (await byId("yield-check").isExisting()), { timeout: 10_000, timeoutMsg: "o terceiro item tem checagem" });
    const text = await textOf(byId("yield-check"));
    expect(text).toContain("difference R$ -0.11");
    expect(text).toContain("The bank paid less");
    await byId("statement-discard").click();
    await browser.waitUntil(() => pendingCount("discarded") === 1, { timeout: 20_000, timeoutMsg: "o terceiro item deveria estar descartado" });
    await byId("statement-review-empty").waitForDisplayed({ timeout: 10_000 });
    expect(pendingCount("pending")).toBe(0);
  });

  it("o saldo do porquinho soma o rendimento líquido que ficou", async () => {
    // −0,28 do extrato semeado + 0,40 (CDB) − 0,30 (IOF) − 0,05 (IR) + 0,40 (LCI) = 0,17.
    expect(balance(pocket)).toBeCloseTo(0.17, 2);
  });

  // Entrega 9: o que a B3 não explica entra pela conciliação com o saldo que o banco mostra.
  it("conciliar com o saldo do banco grava a diferença como rendimento e iguala o saldo", async () => {
    // O caso anterior deixa o popup de revisão aberto, mostrando a fila vazia; ele cobre a barra de abas.
    await browser.keys("Escape");
    await byId("statement-review-empty").waitForExist({ reverse: true, timeout: 10_000 });
    await byId("nav-finances").click();
    await byId("finances-tab-accounts").click();
    await byId("account-reconcile").waitForClickable({ timeout: 20_000 });
    await byId("account-reconcile").click();

    // Sem saldo digitado não há prévia nem botão ligado.
    await byId("reconcile-balance").waitForDisplayed({ timeout: 10_000 });
    expect(await byId("reconcile-confirm").isEnabled()).toBe(false);

    await byId("reconcile-balance").setValue("0.50");
    await waitForText(byId("reconcile-ledger"), (text) => text === "0.17", "o saldo do ledger é o semeado, 0,17");
    expect(await textOf(byId("reconcile-difference"))).toBe("0.33");

    await byId("reconcile-confirm").click();
    await browser.waitUntil(() => ledger(pocket, "receita") === 3, {
      timeout: 20_000,
      timeoutMsg: "a receita da conciliação deveria estar gravada",
    });
    expect(balance(pocket)).toBeCloseTo(0.5, 2);
    expect(sql(`SELECT notes FROM general_transactions WHERE bank_account_id = ${pocket} ORDER BY id DESC LIMIT 1`)).toContain(
      "Yield by balance reconciliation",
    );
  });
});
