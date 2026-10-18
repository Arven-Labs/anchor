import { expect } from "@wdio/globals";
import { byId, enterWorkspace, pickOption, snap, sql, textOf, waitForText } from "./helpers.mjs";

// Fase 22 — aba Backtest do Portfolio. A carteira, o histórico de preços (fechamento ajustado) e o CDI são
// semeados no banco (baixar 10 anos de cotação não é o foco aqui). Dados inventados: A sobe 1 por dia, B fica parado.

function seed() {
  const workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Backtest'");
  sql(`INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`);
  const portfolio = sql(`SELECT id FROM portfolio WHERE workspace_id = ${workspace}`);
  for (const ticker of ["AAAA3", "BBBB3"]) {
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        `VALUES ('${ticker}', '${ticker}', 'acao_br', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')`,
    );
    const asset = sql(`SELECT id FROM assets WHERE ticker = '${ticker}'`);
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${asset}, 'compra', 10, 100, 1000, 0, '2026-01-02', '2026-01-02T00:00:00Z')`,
    );
  }
  const series = (ticker, price) =>
    sql(
      "INSERT INTO stock_adjusted_close (ticker, price_date, adj_close_price, fetched_at) " +
        `WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM n WHERE i < 99) ` +
        `SELECT '${ticker}', date('2026-01-01', '+' || i || ' days'), ${price}, '2026-01-01T00:00:00Z' FROM n`,
    );
  series("AAAA3", "100 + i");
  series("BBBB3", "100");
  for (const month of ["2026-02", "2026-03", "2026-04"]) {
    sql(
      "INSERT INTO macro_index_monthly (index_code, year_month, value_pct, source, fetched_at) " +
        `VALUES ('cdi', '${month}', 1.0, 'test', '2026-01-01T00:00:00Z')`,
    );
  }
}

describe("Anchor — backtest da carteira", () => {
  it("semeia a carteira e abre a aba Backtest com os pesos do valor de mercado", async () => {
    await enterWorkspace("E2E Backtest");
    seed();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-backtest").click();
    await byId("backtest-weight-AAAA3").waitForDisplayed({ timeout: 30_000 });
    // Sem cotação, o valor de mercado cai no preço médio: 1000 e 1000, 50% cada.
    expect(await byId("backtest-weight-AAAA3").getValue()).toBe("50.0");
    expect(await byId("backtest-weight-BBBB3").getValue()).toBe("50.0");
    expect(await byId("backtest-run").isEnabled()).toBe(true);
  });

  it("roda sem rebalancear e mostra o resultado, o gráfico e os benchmarks", async () => {
    await pickOption(byId("backtest-rebalance"), "Never (buy and hold)");
    await byId("backtest-initial").setValue("10000");
    await byId("backtest-run").click();
    await byId("backtest-result").waitForDisplayed({ timeout: 30_000 });
    await byId("backtest-chart").waitForDisplayed({ timeout: 10_000 });

    // A: 100 → 199 (+99%), B: 100 → 100; 50/50 sem rebalancear dá +49,5% (patrimônio de 14.950,00).
    expect(await textOf(byId("backtest-final-value"))).toContain("14.950,00");
    expect(await textOf(byId("backtest-metrics"))).toContain("49.50%");
    // CDI de fev, mar e abr (a janela vai até 10/abr): 1% em cada mês encadeado.
    expect(await textOf(byId("backtest-benchmarks"))).toContain("CDI");
    expect(await textOf(byId("backtest-correlation"))).toContain("1.00");
    await byId("backtest-chart").scrollIntoView({ block: "start" });
    await snap("backtest-chart");
    await byId("backtest-metrics").scrollIntoView({ block: "center" });
    await snap("backtest-result");
  });

  it("rebalancear mensalmente rende menos numa alta de um dos ativos", async () => {
    await pickOption(byId("backtest-rebalance"), "Monthly");
    await byId("backtest-run").click();
    await waitForText(byId("backtest-final-value"), (t) => !t.includes("14.950,00"), "o resultado muda ao rebalancear");
    const text = await textOf(byId("backtest-metrics"));
    expect(text).not.toContain("49.50%");
    expect(text).toMatch(/Rebalances\s*[1-9]/);
    await byId("backtest-metrics").scrollIntoView({ block: "center" });
    await snap("backtest-rebalanced");
  });

  it("a corretagem por ordem entra no resultado e aparece nas métricas", async () => {
    await pickOption(byId("backtest-rebalance"), "Never (buy and hold)");
    await byId("backtest-brokerage").setValue("10");
    await byId("backtest-run").click();
    // Duas ordens de compra (A e B) de R$ 10 = R$ 20; o patrimônio de 14.950,00 encolhe na mesma proporção (× 0,998).
    await waitForText(byId("backtest-final-value"), (t) => t.includes("14.920,10"), "o resultado desconta a corretagem");
    expect(await textOf(byId("backtest-fees"))).toContain("20,00");
    await byId("backtest-metrics").scrollIntoView({ block: "center" });
    await snap("backtest-brokerage");
    await byId("backtest-brokerage").setValue("0");
  });

  it("pesos zerados desligam o botão e um ativo sem peso vira aviso", async () => {
    await byId("backtest-weight-AAAA3").setValue("100");
    await byId("backtest-weight-BBBB3").setValue("0");
    await byId("backtest-run").click();
    await waitForText(byId("backtest-warnings"), (t) => t.includes("BBBB3") && t.includes("no weight given"), "aviso do ativo sem peso");
    expect(await textOf(byId("backtest-assets"))).not.toContain("BBBB3");
  });
});
