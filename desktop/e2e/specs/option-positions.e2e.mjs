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
    expect(Number(sql("SELECT count(*) FROM option_positions"))).toBe(1);
  });
});
