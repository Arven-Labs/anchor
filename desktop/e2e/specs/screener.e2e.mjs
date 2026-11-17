import { expect } from "@wdio/globals";
import { byId, enterWorkspace, sql, textOf, waitForText } from "./helpers.mjs";

// Screener de ações: filtra os tickers que já estão na base. Os dados são
// semeados direto no banco (sem rede), então o teste vale em qualquer runner.
const AT = "2026-11-01T10:00:00+00:00";

function seedTicker(ticker, price, lpa, vpa, roe, dividend) {
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
    seedTicker("CHEAP3", 16, 2, 16, 20, 1.6); // P/L 8, P/VP 1, DY 10%
    seedTicker("PRICY3", 40, 2, 10, 15, 1); // P/L 20, P/VP 4, DY 2,5%

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
});
