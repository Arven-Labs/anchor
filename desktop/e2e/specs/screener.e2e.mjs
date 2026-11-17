import { $, expect } from "@wdio/globals";
import { byId, enterWorkspace, pickOption, sql, textOf, waitForText } from "./helpers.mjs";

// Screener de ações: filtra os tickers que já estão na base. Os dados são
// semeados direto no banco (sem rede), então o teste vale em qualquer runner.
const AT = "2026-11-01T10:00:00+00:00";

// `paidDaysAgo`: pagamentos de dividendo como [dias atrás, valor]. As datas são relativas ao
// relógio real (o app calcula a janela de 12 meses com a data de hoje).
function seedTicker(ticker, price, lpa, vpa, roe, dividend, paidDaysAgo = []) {
  for (const [daysAgo, amount] of paidDaysAgo) {
    sql(
      "INSERT INTO stock_dividend_payments (ticker, payment_date, amount, source, fetched_at) " +
        `VALUES ('${ticker}', date('now', '-${daysAgo} days'), ${amount}, 'e2e', '${AT}')`,
    );
  }
  sql(
    "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
      `VALUES ('${ticker}', ${price}, 'e2e', '${AT}', '${ticker} S.A.', NULL, 'BRL')`,
  );
  sql(
    "INSERT INTO stock_fundamentals (ticker, lpa, vpa, roe, source, fetched_at, payout) " +
      `VALUES ('${ticker}', ${lpa}, ${vpa}, ${roe}, 'e2e', '${AT}', 0.4)`,
  );
  sql(
    "INSERT INTO stock_dividends_avg (ticker, avg_dividend_5y, source, fetched_at) " +
      `VALUES ('${ticker}', ${dividend}, 'e2e', '${AT}')`,
  );
}

describe("Anchor — Screener de ações", function () {
  it("mostra os tickers da base e filtra por P/L máximo", async function () {
    await enterWorkspace("Screener E2E");
    // CHEAP3: P/L 8, P/VP 1, DY 12m 10% (1,6 pago há 30 dias), DY 5a 10%.
    seedTicker("CHEAP3", 16, 2, 16, 20, 1.6, [[30, 1.6]]);
    // PRICY3: P/L 20, P/VP 4, DY 5a 20% (média 8) mas DY 12m 0% (último pagamento há 400 dias).
    seedTicker("PRICY3", 40, 2, 10, 15, 8, [[400, 8]]);

    await byId("nav-lookup").click();
    await byId("open-screener").click();

    await waitForText(
      byId("screener-summary"),
      (text) => text.includes("2 of 2"),
      "o screener deveria listar os 2 tickers semeados",
    );
    const table = await textOf(byId("screener-table"));
    expect(table).toContain("CHEAP3");
    expect(table).toContain("PRICY3");

    await byId("screener-pe_max").setValue("10");
    await byId("screener-apply").click();

    await waitForText(
      byId("screener-summary"),
      (text) => text.includes("1 of 2"),
      "com P/L máx. 10 só CHEAP3 deveria passar",
    );
    const filtered = await textOf(byId("screener-table"));
    expect(filtered).toContain("CHEAP3");
    expect(filtered).not.toContain("PRICY3");
  });

  it("mostra o DY de 12 meses e o da média de 5 anos em colunas separadas", async function () {
    const table = await textOf(byId("screener-table"));
    expect(table).toContain("Div. yield 12m");
    expect(table).toContain("Div. yield 5y avg");
  });

  it("o filtro de DY compara o rendimento de 12 meses, não a média de 5 anos", async function () {
    const clear = await $('//button[normalize-space(.)="Clear"]');
    await clear.click();
    await waitForText(
      byId("screener-summary"),
      (text) => text.includes("2 of 2"),
      "limpar os filtros deveria voltar aos 2 tickers",
    );

    await byId("screener-dy_min").setValue("5");
    await byId("screener-apply").click();

    // PRICY3 tem média de 5 anos de 20%, mas 0% nos últimos 12 meses: só CHEAP3 passa.
    await waitForText(
      byId("screener-summary"),
      (text) => text.includes("1 of 2"),
      "com DY 12m mínimo 5 só CHEAP3 deveria passar",
    );
    const filtered = await textOf(byId("screener-table"));
    expect(filtered).toContain("CHEAP3");
    expect(filtered).not.toContain("PRICY3");
  });

  // P20: depois de abrir o Screener (ou um ativo) não havia como voltar à tela vazia de
  // Research, onde ficam os atalhos de juros, câmbio e Screener.
  it("Voltar sai do Screener e mostra os atalhos de novo", async function () {
    await byId("research-back").waitForDisplayed({ timeout: 10_000 });
    await byId("research-back").click();

    await byId("open-screener").waitForDisplayed({ timeout: 10_000 });
    await byId("open-fx").waitForDisplayed();
    await byId("screener-table").waitForExist({ reverse: true });
    // Na tela vazia não há para onde voltar.
    await byId("research-back").waitForExist({ reverse: true });
  });

  it("Voltar sai da tela de um ativo e mostra os atalhos de novo", async function () {
    await pickOption(byId("global-search-class"), "Stock (B3)");
    await byId("global-search-ticker").setValue("CHEAP3");
    await byId("global-search-submit").click();

    // O botão depende só do ticker aberto; não espera a rede responder.
    await byId("research-back").waitForDisplayed({ timeout: 10_000 });
    await byId("open-screener").waitForExist({ reverse: true });
    await byId("research-back").click();

    await byId("open-screener").waitForDisplayed({ timeout: 10_000 });
    await byId("research-back").waitForExist({ reverse: true });
  });
});
