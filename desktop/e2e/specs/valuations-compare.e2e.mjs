import { $, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, sql, textOf } from "./helpers.mjs";

// Fase 3.4 — comparar cálculos salvos lado a lado. Tudo semeado por SQL: a
// cotação de TEST3 já no banco faz a tela de análise abrir sem rede.
//
// Cálculos de TEST3 (id fixo, para os checkboxes `compare-check-<id>`):
//   101 Bazin  yield 6%   · 102 Graham EPS 3,00 · 103 Bazin yield 8%
//   104 Bazin  yield 10%  · 105 Bazin  yield 12%
const BAZIN_6 = 101;
const GRAHAM = 102;
const BAZIN_8 = 103;
const BAZIN_10 = 104;
const BAZIN_12 = 105;
// Bazin sem a linha em `bazin_inputs`: `get_valuation_inputs` devolve NotFound.
const BAZIN_SEM_PREMISSAS = 106;

function seed() {
  sql(
    "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
      "VALUES ('TEST3', 25.0, 'e2e', '2026-11-01T00:00:00Z', 'Test SA', 'SAO', 'BRL')",
  );
  const valuation = (id, model, fair, margin, verdict, updatedAt) =>
    sql(
      "INSERT INTO valuation (id, ticker, reference_year, current_price, model, fair_price, safety_margin, verdict, updated_at) " +
        `VALUES (${id}, 'TEST3', 2025, 25.0, '${model}', ${fair}, ${margin}, '${verdict}', '${updatedAt}')`,
    );
  const bazin = (id, yieldFraction) =>
    sql(
      `INSERT INTO bazin_inputs (valuation_id, average_dividend, desired_yield) VALUES (${id}, 2.0, ${yieldFraction})`,
    );

  valuation(BAZIN_6, "bazin", 33.33, 0.25, "CHEAP", "2026-11-01T10:00:00Z");
  bazin(BAZIN_6, 0.06);
  valuation(GRAHAM, "graham", 36.74, 0.32, "CHEAP", "2026-11-01T10:01:00Z");
  sql(`INSERT INTO graham_inputs (valuation_id, eps, book_value_per_share) VALUES (${GRAHAM}, 3.0, 20.0)`);
  valuation(BAZIN_8, "bazin", 25.0, 0.0, "EXPENSIVE", "2026-11-01T10:02:00Z");
  bazin(BAZIN_8, 0.08);
  valuation(BAZIN_10, "bazin", 20.0, -0.25, "EXPENSIVE", "2026-11-01T10:03:00Z");
  bazin(BAZIN_10, 0.1);
  valuation(BAZIN_12, "bazin", 16.67, -0.5, "EXPENSIVE", "2026-11-01T10:04:00Z");
  bazin(BAZIN_12, 0.12);
  // Sem `bazin(...)` de propósito.
  valuation(BAZIN_SEM_PREMISSAS, "bazin", 30.0, 0.17, "CHEAP", "2026-11-01T10:05:00Z");
}

const check = (id) => byId(`compare-check-${id}`);
const row = (slug) => byId(`compare-row-${slug}`);

async function setSelected(ids) {
  for (const id of ids) {
    await clickCentered(check(id));
  }
}

async function clearSelection(ids) {
  for (const id of ids) {
    if (await check(id).isSelected()) await clickCentered(check(id));
  }
}

describe("Anchor — comparar cálculos salvos", () => {
  it("abre o detalhe de TEST3 com o botão Compare desabilitado", async () => {
    await enterWorkspace("Compare E2E");
    seed();

    await byId("nav-lookup").click();
    await pickOption(byId("global-search-class"), "Stock (B3)");
    await byId("global-search-ticker").setValue("TEST3");
    await byId("global-search-submit").click();

    await byId("open-saved-valuations").waitForDisplayed({ timeout: 30_000 });
    await clickCentered(byId("open-saved-valuations"));
    await clickCentered(await $('//td[normalize-space(.)="TEST3"]'));

    await byId("compare-button").waitForDisplayed({ timeout: 20_000 });
    expect(await byId("compare-button").isEnabled()).toBe(false);
  });

  it("com 1 cálculo marcado o Compare continua desabilitado", async () => {
    await setSelected([BAZIN_6]);
    expect(await byId("compare-button").isEnabled()).toBe(false);
  });

  it("dois Bazin: mostra os 2 yields e marca só a linha que difere", async () => {
    await setSelected([BAZIN_8]);
    expect(await byId("compare-button").isEnabled()).toBe(true);
    await clickCentered(byId("compare-button"));
    await byId("compare-view").waitForDisplayed({ timeout: 20_000 });

    // Premissas vêm de `get_valuation_inputs`, então esperam a busca terminar.
    await byId("compare-row-desired-yield").waitForDisplayed({ timeout: 20_000 });
    const yieldRow = await textOf(row("desired-yield"));
    expect(yieldRow).toContain("6.0%");
    expect(yieldRow).toContain("8.0%");
    expect(await row("desired-yield").getAttribute("data-differs")).toBe("true");

    // Mesmo ano de referência nos dois: nada a destacar.
    expect(await row("reference-year").getAttribute("data-differs")).toBe("false");
    expect(await row("model").getAttribute("data-differs")).toBe("false");
  });

  it("voltar devolve o detalhe do ticker com a seleção mantida", async () => {
    await clickCentered(byId("compare-back"));
    await byId("compare-button").waitForDisplayed({ timeout: 10_000 });
    expect(await check(BAZIN_6).isSelected()).toBe(true);
    expect(await check(BAZIN_8).isSelected()).toBe(true);
  });

  it("Bazin com Graham: o campo que o modelo não tem aparece como —", async () => {
    await clearSelection([BAZIN_6, BAZIN_8]);
    await setSelected([BAZIN_6, GRAHAM]);
    await clickCentered(byId("compare-button"));
    await byId("compare-row-eps").waitForDisplayed({ timeout: 20_000 });

    const yieldRow = await textOf(row("desired-yield"));
    expect(yieldRow).toContain("6.0%");
    expect(yieldRow).toContain("—");
    const epsRow = await textOf(row("eps"));
    expect(epsRow).toContain("R$ 3.00");
    expect(epsRow).toContain("—");
    expect(await row("model").getAttribute("data-differs")).toBe("true");
    await clickCentered(byId("compare-back"));
    await byId("compare-button").waitForDisplayed({ timeout: 10_000 });
  });

  it("no máximo 4 cálculos: o 5º checkbox não marca", async () => {
    await clearSelection([BAZIN_6, GRAHAM]);
    await setSelected([BAZIN_6, GRAHAM, BAZIN_8, BAZIN_10]);
    await clickCentered(check(BAZIN_12));
    expect(await check(BAZIN_12).isSelected()).toBe(false);
    expect(await byId("compare-button").isEnabled()).toBe(true);
  });

  it("cálculo sem premissas salvas: a célula mostra error e o resto da tabela continua", async () => {
    await clearSelection([BAZIN_6, GRAHAM, BAZIN_8, BAZIN_10]);
    await setSelected([BAZIN_6, BAZIN_SEM_PREMISSAS]);
    await clickCentered(byId("compare-button"));
    await byId("compare-view").waitForDisplayed({ timeout: 20_000 });

    // `NotFound` vem do SQLite local: a busca não é repetida, então o erro
    // aparece na hora (antes, 3 retentativas do react-query levavam ~7s).
    await byId("compare-row-desired-yield").waitForDisplayed({ timeout: 5_000 });
    const yieldRow = await textOf(row("desired-yield"));
    expect(yieldRow).toContain("6.0%");
    expect(yieldRow).toContain("Unavailable");
    expect(await row("desired-yield").getAttribute("data-differs")).toBe("true");

    // As linhas do resultado vêm da tabela `valuation` e não dependem da busca.
    expect(await textOf(row("fair-price"))).toContain("R$ 30.00");
  });
});
