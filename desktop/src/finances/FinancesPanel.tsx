import { useState } from "react";
import BankAccountSection from "./BankAccountSection";
import GeneralTransactionSection from "./GeneralTransactionSection";
import LiabilitySection from "./LiabilitySection";
import LoanSection from "./LoanSection";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

type FinancesSection = "accounts" | "liabilities" | "loans" | "transactions";

const FINANCES_SECTIONS: Record<FinancesSection, string> = {
  accounts: "Accounts",
  liabilities: "Liabilities",
  loans: "Loans",
  transactions: "Transactions",
};

// Fase 12 — núcleo de Finanças Gerais (BankAccount + GeneralTransaction
// manual) + Liability (dívida com parcelas geradas automaticamente, Price/
// SAC). Módulo irmão de Portfolio (Fase 10), não sub-aba dele — o rascunho
// trata "Finanças Gerais" como parte separada da carteira de ativos (ver
// PHASE.md, Fase 12). Categorias não têm aba própria — pedido explícito do
// dono do projeto (Sessão 71): gerenciadas via popup
// (`CategoryManagerDialog`) a partir da aba Transactions, junto do botão
// "New transaction". Sem Category com limite de orçamento, Attachment+Vault,
// IA de extração, reconciliação, retorno alavancado ou "Patrimônio Líquido"
// cruzando com o Portfolio nesta fatia.
function FinancesPanel({ workspaceId }: { workspaceId: number }) {
  const [section, setSection] = useState<FinancesSection>("accounts");

  return (
    <Tabs value={section} onValueChange={(value) => setSection(value as FinancesSection)}>
      <TabsList className="mb-4">
        {Object.entries(FINANCES_SECTIONS).map(([key, label]) => (
          <TabsTrigger key={key} value={key}>
            {label}
          </TabsTrigger>
        ))}
      </TabsList>

      <TabsContent value="accounts">
        <BankAccountSection workspaceId={workspaceId} />
      </TabsContent>
      <TabsContent value="liabilities">
        <LiabilitySection workspaceId={workspaceId} />
      </TabsContent>
      <TabsContent value="loans">
        <LoanSection workspaceId={workspaceId} />
      </TabsContent>
      <TabsContent value="transactions">
        <GeneralTransactionSection workspaceId={workspaceId} />
      </TabsContent>
    </Tabs>
  );
}

export default FinancesPanel;
