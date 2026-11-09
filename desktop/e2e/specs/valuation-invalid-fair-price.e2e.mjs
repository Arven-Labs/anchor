import { $, expect } from "@wdio/globals";
import { byId, clickCentered, enterWorkspace, pickOption, snap, sql } from "./helpers.mjs";

// P10 — preço justo <= 0 não pode virar "Barata". A guarda mora no Rust
// (`ValuationOutcome::from_fair_price`); este spec prova que cada um dos 8
// formulários mostra o erro em vermelho e não mostra veredito. O `Label` do
// formulário não é ligado ao `Input` (sem `htmlFor`), então os campos saem pelo
// texto do rótulo, sem `data-testid` e sem tocar no código de produção.
//
// Cada caso: rótulo do modelo no seletor, campos (rótulo -> valor digitado) e
// uma entrada que força justo <= 0. Os campos que já vêm preenchidos (ticker,
// ano, yield 6, N 5) só são digitados de novo quando o caso os cita.
const CASES = [
  {
    model: "Bazin",
    fields: {
      "Current price (R$)": "25",
      "Average dividend per share (R$, last 5 years)": "-3",
      "Desired yield (%)": "6",
    },
  },
  {
    // Justo negativo é impossível no Graham (raiz quadrada): quem barra é o guard de entrada.
    model: "Graham",
    fields: {
      "Current price (R$)": "25",
      "EPS — earnings per share (R$)": "-2",
      "Book value per share (R$)": "20",
    },
  },
  {
    model: "Gordon / DDM",
    fields: {
      "Current price (R$)": "25",
      "Current dividend — D0 (R$/share)": "-2",
      "Expected dividend growth — g (%)": "3",
      "Required return — Ke (%)": "10",
    },
  },
  {
    // Mesma base do teste Rust do DCF: justo -214,28, que antes saía como "Barata".
    model: "DCF / FCFF",
    fields: {
      "Current price (R$)": "10",
      "EBIT (R$ millions)": "-1000",
      "Effective tax rate (%)": "34",
      "D&A — depreciation/amortization (R$ millions)": "200",
      "Capex (R$ millions)": "300",
      "ΔNWC — change in net working capital (R$ millions)": "50",
      "Total debt (R$ millions)": "2000",
      "Cash (R$ millions)": "500",
      "Shares outstanding (millions)": "100",
      "Beta": "1",
      "Risk-free rate — Rf (%)": "6",
      "Market risk premium (%)": "5",
      "Cost of debt — Kd (%)": "8",
      "Perpetuity growth — g (%)": "3",
    },
  },
  {
    model: "Banks (P/B)",
    fields: {
      "Current price (R$)": "25",
      "Book value per share (R$)": "-20",
      "ROE (%)": "15",
      "Payout (%)": "40",
      "Required return — Ke (%)": "12",
    },
  },
  {
    model: "RIM (Bancos)",
    fields: {
      "Current price (R$)": "25",
      "Book value per share (R$)": "-20",
      "Current ROE (%)": "15",
      "Payout (%)": "40",
      "Required return — Ke (%)": "12",
      "Fade years — N (ROE converges to Ke)": "5",
    },
  },
  {
    model: "RNAV",
    fields: {
      "Current price (R$)": "25",
      "Landbank at market value (R$ millions)": "-5000",
      "Inventory at market value (R$ millions)": "1000",
      "Net cash — cash minus debt, can be negative (R$ millions)": "500",
      "Shares outstanding (millions)": "100",
    },
  },
  {
    model: "Projected Ceiling",
    fields: {
      "Current price (R$)": "25",
      "Current dividend — D0 (R$/share)": "-2",
      "Expected growth — g (%)": "3",
      "Projection years — N": "5",
      "Desired yield (%)": "6",
      "Discount rate — Ke (%)": "10",
    },
  },
];

// O texto do erro vem de `AppError.message`; os dois guards (o de entrada e o
// central do justo) dizem "greater than zero".
const ERROR_TEXT = "greater than zero";

const modelSelect = () =>
  $('//label[normalize-space(.)="Valuation model"]/following-sibling::*//button[@role="combobox"]');
const input = (label) => $(`//label[normalize-space(.)="${label}"]/following-sibling::input`);
const calculateButton = () => $('//button[normalize-space(.)="Calculate"]');
const redError = () =>
  $(`//p[contains(@class,"text-red-600") and contains(normalize-space(.),"${ERROR_TEXT}")]`);
const verdict = () => $('//p[starts-with(normalize-space(.),"Verdict:")]');

async function fill(label, value) {
  const field = await input(label);
  await field.scrollIntoView({ block: "center" });
  await field.setValue(value);
}

describe("Anchor — preço justo <= 0 nos 8 modelos de valuation", () => {
  it("abre a análise de TEST3 e o popup New Valuation", async () => {
    await enterWorkspace("Invalid Fair Price E2E");
    sql(
      "INSERT INTO stock_quotes (ticker, price, source, fetched_at, name, exchange, currency) " +
        "VALUES ('TEST3', 25.0, 'e2e', '2026-11-01T00:00:00Z', 'Test SA', 'SAO', 'BRL')",
    );

    await byId("nav-lookup").click();
    await pickOption(byId("global-search-class"), "Stock (B3)");
    await byId("global-search-ticker").setValue("TEST3");
    await byId("global-search-submit").click();

    const open = await $('//button[normalize-space(.)="New Valuation"]');
    await open.waitForDisplayed({ timeout: 30_000 });
    await clickCentered(open);
    await (await modelSelect()).waitForDisplayed({ timeout: 10_000 });
  });

  for (const { model, fields } of CASES) {
    it(`${model}: recusa o cálculo em vermelho e não mostra veredito`, async () => {
      await pickOption(modelSelect(), model);

      for (const [label, value] of Object.entries(fields)) {
        await fill(label, value);
      }
      await clickCentered(calculateButton());

      const error = await redError();
      await error.waitForDisplayed({ timeout: 20_000 });
      await error.scrollIntoView({ block: "center" });
      await snap(`valuation-invalid-${model}`);

      expect(await verdict().isExisting()).toBe(false);
    });
  }
});
