import { $, browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql } from "./helpers.mjs";

// Hints de premissa também ao EDITAR um valuation salvo: g (histórico de dividendos)
// em Gordon e Teto Projetado, taxa livre de risco no DCF e piso do Ke. Tudo semeado
// por SQL: TEST3 paga 10% a mais por ano de 2020 a 2025, a Selic de 10,50% é a taxa
// livre (sem rede não há curva DI) e os 3 cálculos salvos têm g, Rf e Ke conhecidos.
const GORDON = 201;
const DCF = 202;
const CEILING = 203;

const input = (label) => $(`//label[normalize-space(.)="${label}"]/following-sibling::input`);
const editButton = (modelText) =>
  $(`//tr[.//td[contains(normalize-space(.), "${modelText}")]]//button[normalize-space(.)="Edit"]`);

function seed() {
  sql(
    "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
      "VALUES ('TEST3', 25.0, 'e2e', '2026-11-01T00:00:00Z', 'Test SA', 'SAO', 'BRL')",
  );
  for (const [i, amount] of [1.0, 1.1, 1.21, 1.331, 1.4641, 1.61051].entries()) {
    sql(
      "INSERT INTO stock_dividend_payments (ticker, payment_date, amount, source, fetched_at) " +
        `VALUES ('TEST3', '${2020 + i}-06-15', ${amount}, 'e2e', '2026-11-01T00:00:00Z')`,
    );
  }
  const valuation = (id, model, minute) =>
    sql(
      "INSERT INTO valuation (id, ticker, reference_year, current_price, model, fair_price, safety_margin, verdict, updated_at) " +
        `VALUES (${id}, 'TEST3', 2025, 25.0, '${model}', 30.0, 0.17, 'CHEAP', '2026-11-01T10:0${minute}:00Z')`,
    );
  valuation(GORDON, "gordon", 1);
  sql(
    `INSERT INTO gordon_inputs (valuation_id, current_dividend, expected_growth, ke) VALUES (${GORDON}, 1.5, 0.04, 0.14)`,
  );
  valuation(DCF, "dcf", 2);
  sql(
    "INSERT INTO dcf_inputs (valuation_id, ebit, tax_rate, depreciation_amortization, capex, nwc_change, total_debt, cash, " +
      "shares_outstanding, beta, risk_free_rate, market_risk_premium, kd, perpetuity_growth) " +
      `VALUES (${DCF}, 1000, 0.34, 100, 120, 10, 500, 200, 100, 1.1, 0.06, 0.05, 0.12, 0.03)`,
  );
  valuation(CEILING, "projected_ceiling", 3);
  sql(
    "INSERT INTO projected_ceiling_inputs (valuation_id, current_dividend, expected_growth, projection_years, desired_yield, ke) " +
      `VALUES (${CEILING}, 1.5, 0.04, 5, 0.06, 0.14)`,
  );
}

// Os campos de edição já vêm preenchidos: o `setValue` do WebDriver não os substitui num input
// controlado pelo React (o texto novo entra no fim do antigo). O setter nativo + `input` faz o React ler.
async function replaceValue(label, value) {
  const field = await input(label);
  await field.scrollIntoView({ block: "center" });
  await browser.execute((el, next) => {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
    setter.call(el, next);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  }, field, value);
}

async function openEdit(modelText) {
  const button = await editButton(modelText);
  await button.waitForDisplayed({ timeout: 20_000 });
  await clickCentered(button);
  await (await input("Ticker")).waitForDisplayed({ timeout: 20_000 });
}

async function closeEdit() {
  await clickCentered(await $('//button[normalize-space(.)="Cancel"]'));
}

describe("Anchor — hints de premissa ao editar um valuation salvo", () => {
  it("abre os cálculos salvos de TEST3", async () => {
    await byId("workspace-name").waitForDisplayed({ timeout: 60_000 });
    // A Selic entra antes do workspace: a Home já busca os juros ao entrar (cache de 30 min).
    sql(
      "INSERT INTO selic_daily (series_code, reference_date, value_pct, source, fetched_at) " +
        "VALUES ('meta', '2026-06-18', 10.50, 'e2e', '2026-06-18T00:00:00Z')",
    );
    await enterWorkspace("Edit Hints E2E");
    seed();

    await byId("nav-lookup").click();
    await pickOption(byId("global-search-class"), "Stock (B3)");
    await byId("global-search-ticker").setValue("TEST3");
    await byId("global-search-submit").click();

    await byId("open-saved-valuations").waitForDisplayed({ timeout: 30_000 });
    await clickCentered(byId("open-saved-valuations"));
    await clickCentered(await $('//td[normalize-space(.)="TEST3"]'));
  });

  it("Gordon: mostra o g histórico, Use troca só o g e Ke abaixo do g avisa", async () => {
    await openEdit("Gordon");

    const hint = byId("dividend-growth-hint");
    await hint.waitForDisplayed({ timeout: 20_000 });
    expect(await hint.getText()).toContain("10.0%");
    expect(await input("Expected dividend growth — g").getValue()).toBe("4");

    await byId("dividend-growth-use").click();
    expect(await input("Expected dividend growth — g").getValue()).toBe("10");
    // 0,14 × 100 aparece como 14.000000000000002 (limitação antiga de `toEditableString`): compara o número.
    expect(Number(await input("Required return — Ke").getValue())).toBeCloseTo(14, 6);
    expect(await byId("growth-above-ke").isExisting()).toBe(false);

    await replaceValue("Required return — Ke", "9");
    // DIAGNÓSTICO TEMPORÁRIO (remover): o que há no DOM quando o aviso não aparece.
    await browser.pause(1500);
    console.log(
      "DIAG ke=", await (await input("Required return — Ke")).getValue(),
      "| g=", await (await input("Expected dividend growth — g")).getValue(),
      "| hint=", await byId("dividend-growth").isExisting() ? await byId("dividend-growth").getText() : "(sem hint)",
      "| rf=", await byId("risk-free").isExisting() ? await byId("risk-free").getText() : "(sem rf)",
    );
    await byId("growth-above-ke").waitForDisplayed({ timeout: 10_000 });
    await byId("ke-below-risk-free").waitForDisplayed({ timeout: 10_000 });
    await closeEdit();
  });

  it("DCF: Use no hint troca só a taxa livre de risco", async () => {
    await openEdit("DCF");

    await byId("risk-free-hint").waitForDisplayed({ timeout: 20_000 });
    expect(await input("Risk-free rate — Rf").getValue()).toBe("6");

    await byId("risk-free-use").click();
    expect(await input("Risk-free rate — Rf").getValue()).toBe("10.5");
    expect(await input("Beta").getValue()).toBe("1.1");
    await closeEdit();
  });

  it("Teto Projetado: mostra o g histórico, Use troca o g e não há aviso de g contra Ke", async () => {
    await openEdit("Projected");

    await byId("dividend-growth-hint").waitForDisplayed({ timeout: 20_000 });
    await byId("dividend-growth-use").click();
    expect(await input("Expected growth — g").getValue()).toBe("10");

    await replaceValue("Discount rate — Ke", "9");
    await byId("ke-below-risk-free").waitForDisplayed({ timeout: 10_000 });
    expect(await byId("growth-above-ke").isExisting()).toBe(false);
  });
});
