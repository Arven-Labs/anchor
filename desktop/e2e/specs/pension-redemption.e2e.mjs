import { $, browser, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, snap, sql, textOf, waitForText } from "./helpers.mjs";

// Previdência privada: simulador de resgate (estimativa informativa do IR, regime com tabela por prazo).
// O ativo é semeado no banco (o spec testa o simulador, não o cadastro) e o app real calcula com as
// alíquotas que a migration semeou em `tax_rules`: 35/30/25/20/15/10% por faixa e 15% de antecipação.
// Dados inventados.

// `toLocaleString("pt-BR")` separa "R$" do número com espaço não separável; tiramos todo espaço para comparar.
const compact = (text) => text.replace(/\s+/g, "");

async function fillForm({ redeemed, years, income }) {
  await byId("pension-redeemed").setValue(redeemed);
  if (income !== undefined) await byId("pension-income").setValue(income);
  await byId("pension-years").setValue(years);
}

async function estimate(form) {
  await fillForm(form);
  await clickCentered(byId("pension-submit"));
  await byId("pension-result").waitForDisplayed({ timeout: 20_000 });
}

describe("Anchor — simulador de resgate de previdência", () => {
  it("abre o detalhe da previdência com PGBL, sem o campo de rendimento e com Estimate desabilitado", async () => {
    await enterWorkspace("E2E Pension");
    const workspace = sql("SELECT id FROM workspace WHERE name = 'E2E Pension'");
    sql(
      `INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`,
    );
    sql(
      "INSERT INTO assets (ticker, name, asset_class, currency, exposure_type, exposure_value, created_at) " +
        "VALUES ('PREV1', 'Test Pension Plan', 'previdencia', 'BRL', 'pais', 'BR', '2026-01-01T00:00:00Z')",
    );
    // Para a declaração de 2025: aportes de 500 em jan e fev, extrato de 1.080 em março e resgate de 300 cotas (custo final 700).
    // Semeado antes de abrir o diálogo, para a consulta do ano não guardar um "plano não mantido" em cache.
    const portfolio = sql(`SELECT id FROM portfolio WHERE workspace_id = ${workspace}`);
    const plan = sql("SELECT id FROM assets WHERE ticker = 'PREV1'");
    const trade = (type, quantity, total, date) =>
      sql(
        "INSERT INTO transactions (portfolio_id, asset_id, transaction_type, quantity, unit_price, total_value, fee, transaction_date, created_at) " +
          `VALUES (${portfolio}, ${plan}, '${type}', ${quantity}, ${total / quantity}, ${total}, 0, '${date}', '${date}T00:00:00Z')`,
      );
    trade("compra", 500, 500, "2025-01-05");
    trade("compra", 500, 500, "2025-02-05");
    trade("venda", 300, 360, "2025-03-10");
    sql(
      "INSERT INTO asset_valuations (asset_id, valuation_date, value, origin, created_at) " +
        `VALUES (${plan}, '2025-03-01', 1080, 'manual', '2025-03-01T00:00:00Z')`,
    );

    await byId("nav-portfolio").click();
    await byId("portfolio-tab-assets").waitForDisplayed({ timeout: 30_000 });
    await byId("portfolio-tab-assets").click();
    await clickCentered($("//button[normalize-space(.)='Pension details']"));
    await byId("pension-redemption-simulator").waitForDisplayed({ timeout: 20_000 });

    expect(await byId("pension-income").isExisting()).toBe(false);
    expect(await byId("pension-submit").isEnabled()).toBe(false);
    await snap("pension-simulator-empty");
  });

  it("VGBL passa a pedir o rendimento", async () => {
    await pickOption(byId("pension-kind"), "VGBL");
    await byId("pension-income").waitForDisplayed({ timeout: 10_000 });
    await pickOption(byId("pension-kind"), "PGBL");
    await browser.waitUntil(async () => !(await byId("pension-income").isExisting()), {
      timeout: 10_000,
      timeoutMsg: "voltar para PGBL deveria esconder o campo de rendimento",
    });
  });

  it("PGBL: 10.000 resgatados após 5 anos (faixa de 4 a 6 anos) pagam 25% sobre o total", async () => {
    await estimate({ redeemed: "10000", years: "5" });
    const result = compact(await textOf(byId("pension-result")));
    expect(result).toContain("Rate:25.00%");
    expect(result).toContain("R$10.000,00(thewholeredemption)");
    expect(result).toContain("Estimatedtax:R$2.500,00");
    expect(result).toContain("R$7.500,00");
    expect(result).toContain("15.00%");
    await snap("pension-simulator-pgbl");
  });

  it("exatamente 2 anos ainda está na faixa de 35% (limite superior inclusivo)", async () => {
    await estimate({ redeemed: "10000", years: "2" });
    await waitForText(
      byId("pension-result"),
      (text) => compact(text).includes("Rate:35.00%"),
      "2 anos exatos deveriam cair em 35%, não em 30%",
    );
  });

  it("VGBL: só o rendimento é tributado (35% sobre 2.000 num resgate de 10.000 após 1 ano)", async () => {
    await pickOption(byId("pension-kind"), "VGBL");
    await byId("pension-income").waitForDisplayed({ timeout: 10_000 });
    await estimate({ redeemed: "10000", income: "2000", years: "1" });
    // O resultado do caso anterior (também 35%) ainda pode estar na tela: espera o do VGBL aparecer.
    await waitForText(
      byId("pension-result"),
      (text) => compact(text).includes("(theincomeonly)"),
      "o resultado do VGBL deveria aparecer",
    );
    const result = compact(await textOf(byId("pension-result")));
    expect(result).toContain("Rate:35.00%");
    expect(result).toContain("R$2.000,00(theincomeonly)");
    expect(result).toContain("Estimatedtax:R$700,00");
    expect(result).toContain("R$9.300,00");
    await snap("pension-simulator-vgbl-result");
  });

  it("VGBL com rendimento maior que o resgate avisa na própria tela e não deixa estimar", async () => {
    await fillForm({ redeemed: "1000", income: "1500", years: "1" });
    // A tela barra antes de chamar o backend: aviso em linguagem de usuário, sem o texto cru do erro.
    const warning = byId("pension-income-error");
    await warning.waitForDisplayed({ timeout: 10_000 });
    expect(await textOf(warning)).toBe("The income part can't be greater than the amount redeemed.");
    expect(await byId("pension-submit").isEnabled()).toBe(false);
    // O resultado do caso anterior não pode ficar na tela ao lado de campos que já não o produzem.
    expect(await byId("pension-result").isExisting()).toBe(false);
    await snap("pension-simulator-income-too-high");

    // Corrigindo o rendimento, o aviso some e o botão volta.
    await byId("pension-income").setValue("500");
    await browser.waitUntil(async () => !(await byId("pension-income-error").isExisting()), {
      timeout: 10_000,
      timeoutMsg: "o aviso deveria sumir com o rendimento válido",
    });
    expect(await byId("pension-submit").isEnabled()).toBe(true);
  });

  // Limite de dedução do PGBL: 12% da base (seed da migration), valores digitados.
  it("limite do PGBL: base 100.000 e aportes 8.000 dão teto 12.000 e folga de 4.000, sem excesso", async () => {
    await byId("pension-limit-base").setValue("100000");
    await byId("pension-limit-contributions").setValue("8000");
    await clickCentered(byId("pension-limit-submit"));
    await byId("pension-limit-result").waitForDisplayed({ timeout: 20_000 });
    const result = compact(await textOf(byId("pension-limit-result")));
    expect(result).toContain("Limit:R$12.000,00(12.00%ofR$100.000,00)");
    expect(result).toContain("Deductible:R$8.000,00");
    expect(result).toContain("roomleft:R$4.000,00");
    expect(await byId("pension-limit-excess").isExisting()).toBe(false);
    await (await byId("pension-limit-result")).scrollIntoView({ block: "center" });
    await snap("pension-deduction-limit");
  });

  it("limite do PGBL: aportes de 15.000 passam 3.000 do teto e o excesso aparece", async () => {
    await byId("pension-limit-contributions").setValue("15000");
    // Mexer no campo apaga o resultado anterior; ele só volta com o cálculo novo.
    await browser.waitUntil(async () => !(await byId("pension-limit-result").isExisting()), {
      timeout: 10_000,
      timeoutMsg: "mexer num campo deveria apagar o resultado anterior",
    });
    await clickCentered(byId("pension-limit-submit"));
    const excess = byId("pension-limit-excess");
    await excess.waitForDisplayed({ timeout: 20_000 });
    expect(compact(await textOf(excess))).toContain("R$3.000,00isnotdeductible");
    expect(compact(await textOf(byId("pension-limit-result")))).toContain("roomleft:R$0,00");
    await (await byId("pension-limit-result")).scrollIntoView({ block: "center" });
    await snap("pension-deduction-limit-excess");
  });

  it("limite do PGBL: sem a base, o botão fica desabilitado", async () => {
    await byId("pension-limit-base").setValue("0");
    expect(await byId("pension-limit-submit").isEnabled()).toBe(false);
  });

  // Declaração (Bens e Direitos): custo e saldo em 31/12 e a ficha digitada. Dados semeados no primeiro caso.
  it("declaração: começa no ano anterior, sem ficha, e mostra custo e saldo de 2025", async () => {
    expect(await byId("pension-declaration-year").getValue()).toBe(String(new Date().getFullYear() - 1));
    await byId("pension-declaration-year").setValue("2025");
    await waitForText(
      byId("pension-declaration-position"),
      (text) => compact(text).includes("Coston31/12/2025:R$700,00"),
      "o custo de 2025 deveria ser 700 (1.000 aportados menos 300 resgatados)",
    );
    const position = compact(await textOf(byId("pension-declaration-position")));
    expect(position).toContain("Balanceon31/12/2025:R$1.080,00(statementof2025-03-01)");
    expect(position).toContain("Groupandcodenotfilledin");
  });

  it("declaração: a ficha digitada é gravada com zeros à esquerda e aparece no informe", async () => {
    await byId("pension-dirpf-group").setValue("9");
    await byId("pension-dirpf-code").setValue("6");
    await clickCentered(byId("pension-dirpf-save"));
    await waitForText(
      byId("pension-declaration-ficha"),
      (text) => compact(text) === "Group09·Code06",
      "a ficha digitada deveria aparecer completada com zeros",
    );
    expect(sql("SELECT dirpf_group || '|' || dirpf_code FROM assets WHERE ticker = 'PREV1'")).toBe("09|06");
    // Os campos passam a mostrar o que ficou gravado, e não o "9" e o "6" digitados.
    await browser.waitUntil(async () => (await byId("pension-dirpf-group").getValue()) === "09", {
      timeout: 10_000,
      timeoutMsg: "o campo do grupo deveria mostrar o valor gravado (09)",
    });
    expect(await byId("pension-dirpf-code").getValue()).toBe("06");
    await (await byId("pension-declaration")).scrollIntoView({ block: "center" });
    await snap("pension-declaration");
  });

  it("declaração: um ano anterior ao primeiro aporte mostra que o plano não era mantido", async () => {
    await byId("pension-declaration-year").setValue("2023");
    await waitForText(
      byId("pension-declaration-position"),
      (text) => compact(text).includes("Notheldon31/12/2023"),
      "em 2023 o plano ainda não tinha aportes",
    );
  });
});
