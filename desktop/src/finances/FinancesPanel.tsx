import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import BankAccountSection from "./BankAccountSection";
import GeneralTransactionSection from "./GeneralTransactionSection";
import LiabilitySection from "./LiabilitySection";
import LoanSection from "./LoanSection";
import StatementImportPanel from "./StatementImportPanel";
import { STATEMENT_PENDING_KEY, type StatementPendingItem } from "./StatementReviewDialog";
import PendingNotice from "../pending/PendingNotice";
import { useOpenPending } from "../pending/PendingNav";
import type { AppError } from "../types";
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
  const openPending = useOpenPending();
  const pendingQuery = useQuery<StatementPendingItem[], AppError>({
    queryKey: [STATEMENT_PENDING_KEY, workspaceId],
    queryFn: () => invoke("list_statement_pending", { workspaceId }),
  });

  return (
    <Tabs value={section} onValueChange={(value) => setSection(value as FinancesSection)}>
      <TabsList className="mb-4">
        {Object.entries(FINANCES_SECTIONS).map(([key, label]) => (
          <TabsTrigger key={key} value={key} data-testid={`finances-tab-${key}`}>
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
        <div className="flex flex-col gap-6">
          <StatementImportPanel workspaceId={workspaceId} onStaged={() => openPending({ kind: "statement" })} />
          <PendingNotice count={pendingQuery.data?.length ?? 0} testId="pending-notice-statement" />
          <GeneralTransactionSection workspaceId={workspaceId} />
        </div>
      </TabsContent>
    </Tabs>
  );
}

export default FinancesPanel;
