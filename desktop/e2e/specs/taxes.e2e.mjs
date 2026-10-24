import { browser, expect } from "@wdio/globals";
import {
  byId,
  enterWorkspace,
  seedLossScenario,
  pickOption,
  simulateIvvbSale,
  snap,
  sql,
  textOf,
  waitForText,
} from "./helpers.mjs";

// Impostos (Fase 24): prejuízo a compensar e simulação de venda. As compras e
// vendas são semeadas no banco (cadastrá-las pela tela levaria muitos passos
// que não são o foco aqui).
describe("Anchor — impostos: prejuízo a compensar e simulação de venda", () => {
  it("mostra o prejuízo acumulado a compensar", async () => {
    await enterWorkspace("Tax E2E");
    seedLossScenario("Tax E2E");

    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("loss-balance-common"),
      (text) => text.includes("1.000,00"),
      "o saldo de prejuízo da gaveta comum deveria ser R$ 1.000,00",
    );
  });

  // Fase 24: o mesmo saldo aparece no widget da Home, vindo da mesma apuração.
  it("a Home mostra o prejuízo a compensar no widget", async () => {
    await byId("nav-home").click();
    await waitForText(
      byId("losses-widget-common"),
      (text) => text.includes("1.000,00"),
      "o widget da Home deveria mostrar R$ 1.000,00 de prejuízo na gaveta comum",
    );
    await waitForText(
      byId("losses-widget"),
      (text) => text.includes("Saves up to") && text.includes("150,00"),
      "o widget deveria mostrar a economia de 15% sobre o saldo",
    );
    // O widget fica abaixo da dobra: centraliza antes da captura.
    await (await byId("losses-widget")).scrollIntoView({ block: "center" });
    await snap("home-losses-widget");
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();
  });

  it("simula uma venda que usa o prejuízo e paga imposto só sobre o resto", async () => {
    // Lucro 1.500, compensa 1.000 e a alíquota semeada incide sobre 500
    // (15% → R$ 75,00).
    await simulateIvvbSale();

    await waitForText(byId("sim-gain"), (text) => text.includes("1.500,00"), "lucro de R$ 1.500,00");
    await waitForText(
      byId("sim-loss-used"),
      (text) => text.includes("1.000,00"),
      "deveria consumir os R$ 1.000,00 de prejuízo",
    );
    await waitForText(
      byId("sim-month-tax"),
      (text) => text.includes("75,00"),
      "imposto de R$ 75,00 sobre os R$ 500,00 restantes",
    );
  });

  // IR de cripto: apuração e DARF próprios (código 4600), separados dos da B3.
  it("apura o IR de uma venda de cripto acima da isenção e mostra o DARF 4600", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('BTC', 'Bitcoin', 'cripto', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
    );
    const btc = sql("SELECT id FROM assets WHERE ticker = 'BTC'");
    const trade = (type, total, date) =>
      sql(
        "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
          `VALUES (${portfolio}, ${btc}, '${type}', 1, ${total}, ${total}, 0, '${date}', '${date}T00:00:00Z')`,
      );
    // Comprou por 10 mil e vendeu por 40 mil (acima dos 35 mil de isenção): ganho de 30 mil, 15% = 4.500.
    trade("compra", 10000, "2026-01-02");
    trade("venda", 40000, "2026-02-10");

    // Sai e volta da aba para a apuração ser recarregada.
    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("crypto-tax-due-2026-02"),
      (text) => text.includes("4.500,00"),
      "o DARF de cripto de fevereiro deveria ser R$ 4.500,00",
    );
    await byId("crypto-tax-table").scrollIntoView({ block: "center" });
    await snap("taxes-crypto");

    await byId("crypto-darf-2026-02").click();
    await waitForText(byId("darf-text"), (text) => text.includes("Revenue code: 4600"), "o código da receita da cripto é 4600");
    expect(await textOf(byId("darf-text"))).toContain("4500,00");
    await snap("taxes-crypto-darf");
  });

  // Permuta cripto↔cripto: uma venda e uma compra gravadas juntas; a venda entra na apuração do mês (hoje, a data padrão).
  it("registra uma permuta pela tela e ela vira venda na apuração de cripto", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('ETH', 'Ether', 'cripto', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
    );
    const btc = sql("SELECT id FROM assets WHERE ticker = 'BTC'");
    // O BTC da venda de fevereiro já saiu; compra outro por 10 mil para entregar na troca.
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${btc}, 'compra', 1, 10000, 10000, 0, '2026-02-15', '2026-02-15T00:00:00Z')`,
    );

    // O caso anterior termina com o diálogo do DARF aberto, que cobre a navegação.
    await browser.keys("Escape");
    await byId("darf-text").waitForDisplayed({ reverse: true, timeout: 10_000 });

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-transactions").click();
    await byId("swap-card").waitForDisplayed({ timeout: 30_000 });
    await pickOption(byId("swap-from"), "BTC — Bitcoin");
    await pickOption(byId("swap-to"), "ETH — Ether");
    await byId("swap-quantity-out").setValue("1");
    await byId("swap-quantity-in").setValue("20");
    await byId("swap-value").setValue("50000");
    await byId("swap-card").scrollIntoView({ block: "center" });
    await snap("swap-form");
    await byId("swap-submit").click();
    await byId("swap-done").waitForDisplayed({ timeout: 15_000 });

    // Venda de 50 mil (acima dos 35 mil) com custo de 10 mil: ganho de 40 mil, 15% = 6.000, no mês da data de hoje.
    const month = new Date().toISOString().slice(0, 7);
    await byId("portfolio-tab-taxes").click();
    await waitForText(
      byId(`crypto-tax-due-${month}`),
      (text) => text.includes("6.000,00"),
      "o DARF de cripto do mês da permuta deveria ser R$ 6.000,00",
    );
    await byId("crypto-tax-table").scrollIntoView({ block: "center" });
    await snap("taxes-crypto-swap");
  });

  // Exterior (Lei 14.754): apuração anual em reais, com o dólar de cada dia e a retenção lá fora como crédito.
  it("apura o IR anual de uma ação do exterior com dólar da data e crédito da retenção", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('AAPL', 'Apple', 'acao_internacional', 'USD', 'pais', 'US', '2025-01-01T00:00:00Z')",
    );
    const aapl = sql("SELECT id FROM assets WHERE ticker = 'AAPL'");
    const rate = (date, close) =>
      sql(
        "INSERT INTO stock_price_history (ticker, price_date, close_price, source, fetched_at) " +
          `VALUES ('USDBRL', '${date}', ${close}, 'test', '${date}T00:00:00Z')`,
      );
    rate("2025-01-10", 5);
    rate("2025-06-10", 6);
    rate("2025-09-10", 6);
    const row = (type, quantity, total, date, extra = "") =>
      sql(
        "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, payment_type, quantity, unit_price, total_value, fee, foreign_tax_withheld, transaction_date, created_at) " +
          `VALUES (${portfolio}, ${aapl}, '${type}', ${extra || "NULL"}, ${quantity}, ${total / quantity}, ${total}, 0, ${type === "provento" ? 10 : "NULL"}, '${date}', '${date}T00:00:00Z')`,
      );
    // Compra de US$ 1.000 a R$ 5 e venda de US$ 1.200 a R$ 6: ganho de R$ 2.200; provento de US$ 100 (R$ 600) com US$ 10 retidos
    // (R$ 60). Base 2.800, imposto 420, crédito 60: R$ 360,00.
    row("compra", 10, 1000, "2025-01-10");
    row("venda", 10, 1200, "2025-06-10");
    row("provento", 1, 100, "2025-09-10", "'dividendo'");

    await browser.keys("Escape");
    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("foreign-tax-due-2025"),
      (text) => text.includes("360,00"),
      "o imposto estimado do exterior em 2025 deveria ser R$ 360,00",
    );
    await byId("foreign-details-2025").click();
    await byId("foreign-details-panel-2025").waitForDisplayed({ timeout: 10_000 });
    await byId("foreign-tax-table").scrollIntoView({ block: "center" });
    await snap("taxes-foreign");
  });

  // PTAX do Banco Central: a gravada em `ptax_daily` manda (venda; fechamento) e a tela diz de onde veio a taxa.
  it("converte pela PTAX gravada e marca a origem da taxa", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('MSFT', 'Microsoft', 'acao_internacional', 'USD', 'pais', 'US', '2026-01-01T00:00:00Z')",
    );
    const msft = sql("SELECT id FROM assets WHERE ticker = 'MSFT'");
    const ptax = (date, buy, sell) =>
      sql(
        "INSERT INTO ptax_daily (currency, reference_date, buy, sell, source, fetched_at) " +
          `VALUES ('USD', '${date}', ${buy}, ${sell}, 'test', '${date}T00:00:00Z')`,
      );
    ptax("2026-01-15", 4.99, 5);
    ptax("2026-06-15", 5.99, 6);
    const row = (type, quantity, total, date) =>
      sql(
        "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
          `VALUES (${portfolio}, ${msft}, '${type}', ${quantity}, ${total / quantity}, ${total}, 0, '${date}', '${date}T00:00:00Z')`,
      );
    // Compra de US$ 1.000 a R$ 5,00 (custo R$ 5.000) e venda de US$ 1.400 a R$ 6,00 (R$ 8.400): ganho de R$ 3.400, 15% = R$ 510,00.
    row("compra", 10, 1000, "2026-01-15");
    row("venda", 10, 1400, "2026-06-15");
    const saleId = sql("SELECT id FROM transactions WHERE asset_id = " + msft + " AND transaction_type = 'venda'");

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("foreign-tax-due-2026"),
      (text) => text.includes("510,00"),
      "o imposto estimado do exterior em 2026, pela PTAX, deveria ser R$ 510,00",
    );
    await byId("foreign-details-2026").click();
    await waitForText(
      byId(`foreign-sale-fx-${saleId}`),
      (text) => text.includes("6.0000") && text.includes("PTAX"),
      "a venda deveria mostrar o dólar de 6,0000 vindo da PTAX",
    );
    await byId("foreign-tax-table").scrollIntoView({ block: "center" });
    await snap("taxes-foreign-ptax");
  });

  // Informe de Bens e Direitos: custo médio em reais de cada ativo do exterior em 31/12, pela PTAX de venda do dia da compra.
  it("mostra o informe de bens e direitos do exterior com o custo em reais", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('NVDA', 'Nvidia', 'acao_internacional', 'USD', 'pais', 'US', '2025-01-01T00:00:00Z')",
    );
    const nvda = sql("SELECT id FROM assets WHERE ticker = 'NVDA'");
    sql(
      "INSERT INTO ptax_daily (currency, reference_date, buy, sell, source, fetched_at) " +
        "VALUES ('USD', '2025-02-14', 5.19, 5.2, 'test', '2025-02-14T00:00:00Z')",
    );
    // Compra de US$ 500 a R$ 5,20 (PTAX de venda do dia): custo de R$ 2.600,00 e a posição segue em 31/12/2025.
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${nvda}, 'compra', 5, 100, 500, 0, '2025-02-14', '2025-02-14T00:00:00Z')`,
    );

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await byId("foreign-holdings-card").waitForDisplayed({ timeout: 30_000 });
    await waitForText(
      byId("foreign-holding-NVDA-cost"),
      (text) => text.includes("2.600,00") && text.includes("5 un."),
      "o custo de NVDA em reais deveria ser R$ 2.600,00 com 5 unidades",
    );
    await byId("foreign-holdings-card").scrollIntoView({ block: "center" });
    await snap("taxes-holdings");
  });

  // Entidade controlada no exterior: lucro de 31/12 pela PTAX de venda, participação e crédito pela PTAX de compra.
  it("apura uma entidade controlada no exterior pela PTAX de 31/12", async () => {
    const workspace = sql("SELECT id FROM workspace WHERE name = 'Tax E2E'");
    sql(
      "INSERT INTO ptax_daily (currency, reference_date, buy, sell, source, fetched_at) " +
        "VALUES ('USD', '2025-12-31', 5.9, 6.0, 'test', '2025-12-31T00:00:00Z')",
    );
    sql(
      "INSERT INTO offshore_entities (workspace_id, name, country, currency, ownership_percent, created_at) " +
        `VALUES (${workspace}, 'Holding LLC', 'US', 'USD', 50, '2026-01-01T00:00:00Z')`,
    );
    const entity = sql("SELECT id FROM offshore_entities WHERE name = 'Holding LLC'");
    // 50% de US$ 2.000 × 6,00 = R$ 6.000 (imposto 900); crédito 100 × 50% × 5,90 = R$ 295; devido R$ 605.
    sql(
      "INSERT INTO offshore_results (entity_id, year, profit, tax_paid_abroad, created_at) " +
        `VALUES (${entity}, 2025, 2000, 100, '2026-01-01T00:00:00Z')`,
    );

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("offshore-tax-due-2025"),
      (text) => text.includes("605,00"),
      "o imposto estimado da entidade no exterior em 2025 deveria ser R$ 605,00",
    );
    await byId("offshore-details-2025").click();
    await byId("offshore-card").scrollIntoView({ block: "center" });
    await snap("taxes-offshore");
  });

  // Come-cotas: fundo com cotas, tipo e valor de cota lançados. As datas são relativas a hoje (o app usa a data de hoje),
  // então o teste confere o texto da estimativa, não o valor do imposto (que depende da última data de come-cotas).
  it("estima o come-cotas de um fundo pelo último valor de cota lançado", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('FUNDOX', 'Fundo X', 'fundo', 'BRL', 'pais', 'BR', '2025-01-01T00:00:00Z')",
    );
    const fund = sql("SELECT id FROM assets WHERE ticker = 'FUNDOX'");
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${fund}, 'compra', 100, 10, 1000, 0, date('now', '-400 days'), '2025-01-10T00:00:00Z')`,
    );
    sql(`INSERT INTO fund_details (asset_id, term, created_at) VALUES (${fund}, 'curto_prazo', '2025-01-10T00:00:00Z')`);
    sql(
      "INSERT INTO fund_quota_values (asset_id, quota_date, value, created_at) " +
        `VALUES (${fund}, date('now', '-1 day'), 12, '2025-01-10T00:00:00Z')`,
    );

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await byId("come-cotas-card").waitForDisplayed({ timeout: 30_000 });
    await waitForText(
      byId("come-cotas-estimate-FUNDOX"),
      (text) => text.includes("Next come-cotas") && text.includes("20.0%") && text.includes("12,00"),
      "a estimativa do fundo de curto prazo deveria mostrar a alíquota de 20% e a cota de R$ 12,00",
    );
    // A posição existia na última data de come-cotas e nenhum desconto foi registrado: o cartão avisa.
    await byId("come-cotas-unrecorded-FUNDOX").waitForDisplayed({ timeout: 30_000 });
    await byId("come-cotas-card").scrollIntoView({ block: "center" });
    await snap("taxes-come-cotas");
  });

  // O desconto real é uma transação `come_cotas` (cotas retiradas × valor da cota, sem caixa): aparece na lista do cartão.
  it("lista o come-cotas registrado como transação", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    const fund = sql("SELECT id FROM assets WHERE ticker = 'FUNDOX'");
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${fund}, 'come_cotas', 5, 12, 60, NULL, date('now', '-1 day'), '2025-01-10T00:00:00Z')`,
    );

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("come-cotas-deductions-FUNDOX"),
      (text) => text.includes("5 quotas") && text.includes("12,00") && text.includes("60,00"),
      "o come-cotas registrado deveria aparecer com 5 cotas a R$ 12,00 e imposto de R$ 60,00",
    );
    await byId("come-cotas-card").scrollIntoView({ block: "center" });
    await snap("taxes-come-cotas-recorded");
  });

  // Resgate (venda do fundo): IOF e IR estimados por lote, com o come-cotas pago abatendo o IR. Compra a 400 dias do resgate,
  // fundo de curto prazo (20% acima de 180 dias); o come-cotas pago (R$ 12,63 desta fração) passa do IR devido (R$ 12,42),
  // então o IR estimado é zero e o excesso vira aviso. As datas são relativas a hoje, mas o prazo (400 dias) é fixo.
  it("estima o IR do resgate de um fundo com o come-cotas já pago", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    const fund = sql("SELECT id FROM assets WHERE ticker = 'FUNDOX'");
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${fund}, 'venda', 20, 13, 260, NULL, date('now'), '2025-01-10T00:00:00Z')`,
    );

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("fund-redemptions-FUNDOX"),
      (text) => text.includes("20 quotas") && text.includes("260,00") && text.includes("excess come-cotas"),
      "o resgate de 20 cotas por R$ 260,00 deveria aparecer com o aviso de come-cotas em excesso",
    );
    await waitForText(
      byId("fund-refundable-FUNDOX"),
      (text) => text.includes("provides no refund") && text.includes("loss to offset later gains") && text.includes("already adds that loss"),
      "o excesso de come-cotas deveria aparecer somado, dizendo que não há restituição e que vira perda a compensar",
    );
    await byId("come-cotas-card").scrollIntoView({ block: "center" });
    await snap("taxes-fund-redemption");
  });

  // Compensação de prejuízo entre fundos: dois fundos de longo prazo da mesma administradora. O FUNDOL perde R$ 200,00 num
  // resgate de 01/06/2025; o FUNDOG ganha R$ 200,00 num resgate de 01/09/2025 (243 dias, 20%: IR de R$ 40,00) e, com o
  // crédito, não paga IR. As datas são fixas, então a conta não depende de "hoje".
  it("compensa o prejuízo de um fundo no IR do resgate de outro da mesma administradora", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    const trade = (fundId, type, quantity, price, date) =>
      sql(
        "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
          `VALUES (${portfolio}, ${fundId}, '${type}', ${quantity}, ${price}, ${quantity * price}, NULL, '${date}', '${date}T00:00:00Z')`,
      );
    for (const [ticker, name] of [
      ["FUNDOL", "Fundo L"],
      ["FUNDOG", "Fundo G"],
    ]) {
      sql(
        "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
          `VALUES ('${ticker}', '${name}', 'fundo', 'BRL', 'pais', 'BR', '2025-01-01T00:00:00Z')`,
      );
      const id = sql(`SELECT id FROM assets WHERE ticker = '${ticker}'`);
      sql(
        "INSERT INTO fund_details (asset_id, term, administrator, created_at) " +
          `VALUES (${id}, 'longo_prazo', '${ticker === "FUNDOL" ? "XP" : " xp "}', '2025-01-10T00:00:00Z')`,
      );
      trade(id, "compra", 100, 10, "2025-01-01");
      trade(id, "venda", 100, ticker === "FUNDOL" ? 8 : 12, ticker === "FUNDOL" ? "2025-06-01" : "2025-09-01");
    }

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    await waitForText(
      byId("fund-redemptions-FUNDOL"),
      (text) => text.includes("loss") && text.includes("200,00") && text.includes("to offset"),
      "o resgate com prejuízo do FUNDOL deveria mostrar R$ 200,00 a compensar",
    );
    // O formatBrl separa "R$" do valor com um espaço inseparável (U+00A0): troca por espaço comum antes de conferir. Se
    // não casar, o erro leva o texto que a tela mostrou.
    let seen = "";
    try {
      await browser.waitUntil(
        async () => {
          seen = (await textOf(byId("fund-redemptions-FUNDOG"))).replace(/ /g, " ");
          return seen.includes("IR R$ 0,00, net R$ 1.200,00") && seen.includes("loss credit R$ 200,00 used");
        },
        { timeoutMsg: "o resgate do FUNDOG deveria usar o crédito de R$ 200,00 e ficar com IR de R$ 0,00" },
      );
    } catch (error) {
      throw new Error(`${error.message}. Texto na tela: ${seen}`);
    }
    await byId("fund-administrator-input-FUNDOG").waitForDisplayed({ timeout: 20_000 });
    await byId("fund-redemptions-FUNDOG").scrollIntoView({ block: "center" });
    await snap("taxes-fund-compensation");
  });

  // Fundo fechado (Lei 14.754): estoque de 2023 a 8% e uma amortização. 100 cotas a R$ 10,00 (01/06/2022); cota de 29/12/2023 a
  // R$ 14,00: ganho de R$ 400,00 e imposto de R$ 32,00 (8%), pago em dinheiro. Amortização de R$ 600,00 com a cota a R$ 12,00
  // em 01/03/2026: metade da posição (50 cotas equivalentes, as 100 ficam); 1.369 dias (15%): rendimento 50 × 2 = 100 + 200
  // já tributados = 300, IR devido 45 − 16 pagos do estoque = 29. As datas são fixas, então a conta não depende de "hoje".
  it("estima o estoque de 2023 e a amortização de um fundo fechado", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('FUNDOC', 'Fundo C', 'fundo', 'BRL', 'pais', 'BR', '2022-01-01T00:00:00Z')",
    );
    const fund = sql("SELECT id FROM assets WHERE ticker = 'FUNDOC'");
    sql(
      "INSERT INTO fund_details (asset_id, term, structure, estoque_option, created_at) " +
        `VALUES (${fund}, 'longo_prazo', 'fechado', 'opcao_8', '2022-06-01T00:00:00Z')`,
    );
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${fund}, 'compra', 100, 10, 1000, NULL, '2022-06-01', '2022-06-01T00:00:00Z')`,
    );
    sql(
      "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
        `VALUES (${portfolio}, ${fund}, 'amortizacao', NULL, 12, 600, NULL, '2026-03-01', '2026-03-01T00:00:00Z')`,
    );
    sql(
      "INSERT INTO fund_quota_values (asset_id, quota_date, value, source, created_at) " +
        `VALUES (${fund}, '2023-12-29', 14, 'manual', '2023-12-29T00:00:00Z')`,
    );

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    // O formatBrl separa "R$" do valor com um espaço inseparável (U+00A0): troca por espaço comum antes de conferir.
    const waitFor = async (testId, predicate, message) => {
      let seen = "";
      try {
        await browser.waitUntil(
          async () => {
            seen = (await textOf(byId(testId))).replace(/ /g, " ");
            return predicate(seen);
          },
          { timeoutMsg: message },
        );
      } catch (error) {
        throw new Error(`${error.message}. Texto na tela: ${seen}`);
      }
    };
    await waitFor(
      "fund-estoque-status-FUNDOC",
      (text) => text.includes("gain of R$ 400,00") && text.includes("taxed at 8%") && text.includes("R$ 32,00"),
      "o estoque de 2023 do FUNDOC deveria mostrar ganho de R$ 400,00 e imposto de R$ 32,00 a 8%",
    );
    await waitFor(
      "fund-redemptions-FUNDOC",
      (text) =>
        text.includes("amortization of R$ 600,00") &&
        text.includes("IR R$ 29,00, net R$ 571,00"),
      "a amortização do FUNDOC deveria pagar IR de R$ 29,00 e sobrar R$ 571,00",
    );
    await byId("fund-structure-FUNDOC").scrollIntoView({ block: "center" });
    await snap("taxes-fund-closed");
  });

  // Cota da CVM: o E2E roda sem rede (ANCHOR_DISABLE_INGESTION), então a leitura do informe diário não acontece. Aqui
  // confere o que a tela mostra sem CNPJ e, com o CNPJ e uma cota gravada com `source = 'cvm'`, o selo e a data.
  it("mostra de onde vem a cota do fundo: sem CNPJ, e lida da CVM", async () => {
    const fund = sql("SELECT id FROM assets WHERE ticker = 'FUNDOX'");

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();
    await waitForText(
      byId("fund-cvm-status-FUNDOX"),
      (text) => text.includes("No CNPJ"),
      "um fundo sem CNPJ deveria dizer que a cota é lançada à mão",
    );

    sql(`UPDATE assets SET cnpj = '00.017.024/0001-53' WHERE id = ${fund}`);
    sql(
      "INSERT INTO fund_quota_values (asset_id, quota_date, value, source, created_at) " +
        `VALUES (${fund}, date('now', '-2 days'), 12.5, 'cvm', '2025-01-10T00:00:00Z')`,
    );

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();
    await waitForText(
      byId("fund-cvm-status-FUNDOX"),
      (text) => text.includes("CVM") && text.includes("Latest quota from the CVM daily report"),
      "com o CNPJ e uma cota da CVM, a tela deveria mostrar o selo CVM e a data da última cota",
    );
    await browser.waitUntil(async () => (await byId("fund-cnpj-FUNDOX").getValue()) === "00.017.024/0001-53", {
      timeout: 20_000,
      timeoutMsg: "o campo do CNPJ deveria mostrar o CNPJ gravado do fundo",
    });
    await byId("come-cotas-card").scrollIntoView({ block: "center" });
    await snap("taxes-fund-cvm");
  });

  // Aviso da guarda do registro das perdas (IN RFB 1.585, art. 15 §2): a Zeta tem um único fundo, zerado por um resgate com
  // perda de R$ 200,00 em 01/06/2025 (100 cotas a 10,00 vendidas a 8,00). O saldo fica no grupo e todos os fundos dela estão
  // zerados, então a tela avisa. A guarda vai até 31/12/2026: o texto depende de "hoje" estar antes ou depois dessa data,
  // por isso o teste aceita as duas versões, e confere o que não muda (administradora, data do resgate, saldo e a norma).
  it("avisa a guarda do registro das perdas quando todos os fundos da administradora foram resgatados", async () => {
    const portfolio = sql("SELECT id FROM portfolio WHERE name = 'Main' ORDER BY id DESC LIMIT 1");
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('FUNDOZ', 'Fundo Z', 'fundo', 'BRL', 'pais', 'BR', '2025-01-01T00:00:00Z')",
    );
    const fund = sql("SELECT id FROM assets WHERE ticker = 'FUNDOZ'");
    sql(
      "INSERT INTO fund_details (asset_id, term, administrator, created_at) " +
        `VALUES (${fund}, 'longo_prazo', 'Zeta', '2025-01-10T00:00:00Z')`,
    );
    for (const [type, price, date] of [
      ["compra", 10, "2025-01-01"],
      ["venda", 8, "2025-06-01"],
    ]) {
      sql(
        "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
          `VALUES (${portfolio}, ${fund}, '${type}', 100, ${price}, ${100 * price}, NULL, '${date}', '${date}T00:00:00Z')`,
      );
    }

    await byId("nav-home").click();
    await byId("nav-portfolio").click();
    await byId("portfolio-tab-taxes").click();

    // O formatBrl separa "R$" do valor com um espaço inseparável (U+00A0): troca por espaço comum antes de conferir. Se não
    // casar, o erro leva o texto que a tela mostrou.
    let seen = "";
    try {
      await browser.waitUntil(
        async () => {
          seen = (await textOf(byId("fund-record-note-Zeta"))).replace(/ /g, " ");
          return (
            seen.includes("with Zeta were redeemed by 01/06/2025") &&
            seen.includes("R$ 200,00") &&
            seen.includes("IN RFB 1.585, art. 15 §2") &&
            (seen.includes("is only required to keep the loss record") || seen.includes("ended on 31/12/2026"))
          );
        },
        { timeoutMsg: "o aviso da Zeta deveria dizer a data do resgate, o saldo de R$ 200,00 e a norma" },
      );
    } catch (error) {
      throw new Error(`${error.message}. Texto na tela: ${seen}`);
    }
    await byId("fund-record-notes").scrollIntoView({ block: "center" });
    await snap("taxes-fund-record-note");
  });
});
