import { $, expect } from "@wdio/globals";
import { byId, byIdPrefix, enterWorkspace, waitForText } from "./helpers.mjs";

describe("Anchor — Loans, de ponta a ponta", () => {
  it("cria um Workspace e entra nele", async () => {
    await enterWorkspace("E2E Family");
  });

  it("registra um empréstimo e uma baixa com recebido diferente do baixado", async () => {
    await byId("nav-finances").click();
    await byId("finances-tab-loans").click();

    // Devedor: ao criar, ele já vem selecionado no formulário do empréstimo.
    await byId("loan-new-borrower-name").setValue("Mom");
    await byId("loan-add-borrower").click();
    const newLoan = await byId("loan-new");
    await newLoan.waitForEnabled({ timeout: 20_000 });
    await newLoan.click();

    await waitForText(
      byId("loan-borrower-select"),
      (text) => text.includes("Mom"),
      "o devedor recém-criado deveria vir selecionado no empréstimo",
    );
    await byId("loan-principal").setValue("1000");
    await byId("loan-create").click();

    // Sem taxa: saldo a receber igual ao principal, sem extra.
    const receivable = await byIdPrefix("loan-receivable-");
    await receivable.waitForDisplayed({ timeout: 20_000 });
    await expect(byIdPrefix("loan-receivable-")).toHaveText("1000.00");
    await expect(byIdPrefix("loan-extra-")).toHaveText("0.00");

    // Baixa: recebeu 1.050 e baixa 1.000 do saldo.
    await byIdPrefix("loan-repay-").click();
    await byId("repay-received").setValue("1050");
    await byId("repay-written-off").setValue("1000");
    await byId("repay-submit").click();

    await expect(byIdPrefix("loan-receivable-")).toHaveText("0.00");
    await expect(byIdPrefix("loan-extra-")).toHaveText("50.00");
  });

  it("mantém os dados depois de trocar de aba e voltar", async () => {
    await byId("nav-home").click();
    await byId("nav-finances").click();
    await byId("finances-tab-loans").click();

    await expect(byIdPrefix("loan-receivable-")).toHaveText("0.00");
    await expect(byIdPrefix("loan-extra-")).toHaveText("50.00");
    await expect($("body")).toBeDisplayed();
  });
});
