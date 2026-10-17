import { $ } from "@wdio/globals";
import {
  byId,
  byIdPrefix,
  clickCentered,
  enterWorkspace,
  pickOption,
  sql,
  textOf,
  waitForText,
} from "./helpers.mjs";

// Posições em opções (Fase 18, 3ª fatia). Tudo local, sem rede: a marcação a mercado
// baixa ~80 MB da B3 e fica de fora de propósito. A carteira é semeada no banco
// (mesmo caminho dos specs de impostos).
describe("Anchor — posições em opções na Carteira", () => {
  it("registra uma opção lançada e a encerra sem valor, realizando o prêmio", async () => {
    await enterWorkspace("Options E2E");
    const workspace = sql("SELECT id FROM workspace WHERE name = 'Options E2E'");
    sql(
      `INSERT INTO portfolio (workspace_id, name, created_at) VALUES (${workspace}, 'Main', '2026-01-01T00:00:00Z')`,
    );

    await byId("nav-portfolio").click();
    await byId("portfolio-tab-options").click();

    // Call lançada: 100 opções a R$ 1,50 (vencimento e data de abertura já vêm preenchidos).
    await byId("op-series").setValue("petrk600");
    await byId("op-underlying").setValue("petr4");
    await pickOption(byId("op-side"), "Sold (written)");
    await byId("op-strike").setValue("60");
    await byId("op-premium").setValue("1.5");
    await byId("op-submit").click();

    const openTable = await byId("op-open-table");
    await openTable.waitForDisplayed({ timeout: 20_000 });
    const row = await textOf(openTable);
    expect(row).toContain("PETRK600"); // maiúsculas, como o backend grava
    expect(row).toContain("Sold call");
    expect(await textOf(byId("op-open-count"))).toBe("1");

    // Encerra como "expirou sem valor": a lançada ganha o prêmio todo, 100 × 1,50 = R$ 150,00.
    await clickCentered(byIdPrefix("close-position-"));
    await pickOption(byId("close-kind"), "Expired worthless");
    await byId("close-submit").click();

    await waitForText(
      byId("op-realized-total"),
      (text) => text.includes("150,00") && text.includes("+"),
      "o resultado realizado deveria ser +R$ 150,00",
    );
    expect(await textOf(byId("op-open-count"))).toBe("0");
    expect(sql("SELECT close_kind || ':' || close_price FROM option_positions")).toBe("expired:0.0");
    expect(await textOf($("body"))).toContain("No open option positions");
  });

  it("leva o resultado realizado da opção para a apuração de imposto do mês", async () => {
    // A call lançada foi aberta e encerrada hoje: day trade, +R$ 150,00 na gaveta de day
    // trade a 20 % = R$ 30,00, menos o IRRF de 1 % do ganho (R$ 1,50) = R$ 28,50 a pagar,
    // sem a isenção dos R$ 20 mil (que é só de ações).
    const period = new Date().toISOString().slice(0, 7);
    await clickCentered(byId("portfolio-tab-taxes"));

    await waitForText(
      byId(`tax-due-${period}`),
      (text) => text.includes("28,50"),
      "aberta e encerrada no mesmo dia, deveria pagar 20 % de R$ 150,00 menos 1 % de IRRF = R$ 28,50",
    );
    await clickCentered(byId(`tax-details-${period}`));
    const note = await byId("options-tax-note");
    await note.waitForDisplayed({ timeout: 10_000 });
    expect(await textOf($("body"))).toContain("PETRK600");

    // Antedatando a abertura, vira swing trade: gaveta comum a 15 % = R$ 22,50.
    sql("UPDATE option_positions SET open_date = '2026-01-02'");
    await clickCentered(byId("portfolio-tab-options"));
    await clickCentered(byId("portfolio-tab-taxes"));
    await waitForText(
      byId(`tax-due-${period}`),
      (text) => text.includes("22,50"),
      "aberta antes do encerramento, deveria pagar 15 % de R$ 150,00 = R$ 22,50",
    );

    await clickCentered(byId("portfolio-tab-options")); // o caso seguinte usa o formulário
  });

  it("conta a posição aberta no patrimônio a custo e tira ao desligar a flag", async () => {
    // Call lançada de 100 opções a R$ 1,00, sem preço atualizado: vale −R$ 100,00 (passivo, ao prêmio).
    await byId("op-series").setValue("petrw550");
    await byId("op-underlying").setValue("petr4");
    await pickOption(byId("op-side"), "Sold (written)");
    await byId("op-strike").setValue("55");
    await byId("op-premium").setValue("1");
    await byId("op-submit").click();

    await waitForText(
      byId("op-net-worth-impact"),
      (text) => text.includes("100,00") && text.includes("−"),
      "a lançada deveria pesar −R$ 100,00 no patrimônio",
    );
    expect(await textOf(byId("op-at-cost-note"))).toContain("1 at cost");

    // O patrimônio de Finanças enxerga o mesmo valor.
    await clickCentered(byId("nav-finances"));
    await clickCentered(byId("finances-tab-liabilities"));
    await waitForText(
      byId("nw-options"),
      (text) => text === "-100.00",
      "o cartão Options de Finanças deveria mostrar -100.00",
    );

    // Desligando a flag a posição sai do patrimônio.
    await clickCentered(byId("nav-portfolio"));
    await clickCentered(byId("portfolio-tab-options"));
    await clickCentered(byIdPrefix("op-nw-toggle-"));
    await waitForText(
      byId("op-net-worth-impact"),
      (text) => text === "—",
      "sem posição contada o impacto no patrimônio deveria ficar em —",
    );
    expect(sql("SELECT include_in_net_worth FROM option_positions WHERE series_ticker = 'PETRW550'")).toBe("0");
    await clickCentered(byId("nav-finances"));
    await clickCentered(byId("finances-tab-liabilities"));
    await waitForText(byId("nw-options"), (text) => text === "0.00", "Options deveria voltar a 0.00");

    await clickCentered(byId("nav-portfolio"));
    await clickCentered(byId("portfolio-tab-options")); // o caso seguinte usa o formulário
  });

  it("mostra o erro do backend quando o formulário é inválido", async () => {
    await byId("op-series").setValue("PETRW550");
    await byId("op-underlying").setValue("PETR4");
    await byId("op-strike").setValue("55");
    await byId("op-premium").setValue("1");
    await byId("op-quantity").setValue("0"); // o campo aceita, o backend recusa
    await byId("op-submit").click();

    await waitForText(
      byId("option-positions"),
      (text) => text.includes("quantity must be greater than zero"),
      "o erro de quantidade deveria aparecer na tela",
    );
    expect(Number(sql("SELECT count(*) FROM option_positions"))).toBe(2);
  });
});
