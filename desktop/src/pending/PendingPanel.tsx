import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { Portfolio } from "../portfolio/types";
import PendingItemsSection from "../portfolio/PendingItemsSection";
import StatementPendingSection from "../finances/StatementPendingSection";
import DividendPendingSection from "./DividendPendingSection";
import type { PendingFocus } from "./PendingNav";

// Fase 35, entrega 6 — tela única de pendentes. Reúne as filas de revisão que antes
// moravam em quatro lugares: lançamentos de carteira (por carteira), linhas de extrato
// e rendimentos (por Workspace) e sugestões de provento (por carteira). Cada bloco é o
// mesmo componente de antes, com o mesmo popup de revisão; só o lugar mudou.
// `initialFocus` vem de quem importou e abre a revisão certa já na chegada (o painel
// monta de novo a cada visita à aba, então o estado inicial basta).
function PendingPanel({
  workspaceId,
  initialFocus,
}: {
  workspaceId: number;
  initialFocus: PendingFocus | null;
}) {
  const [portfolioReviewId, setPortfolioReviewId] = useState<number | null>(
    initialFocus?.kind === "portfolio" ? initialFocus.portfolioId : null,
  );
  const [statementReviewOpen, setStatementReviewOpen] = useState(initialFocus?.kind === "statement");

  const portfoliosQuery = useQuery<Portfolio[], AppError>({
    queryKey: ["portfolios", workspaceId],
    queryFn: () => invoke("list_portfolios", { workspaceId }),
  });
  const portfolios = portfoliosQuery.data ?? [];

  return (
    <div className="flex flex-col gap-6" data-testid="pending-panel">
      {portfoliosQuery.isError && <p className="text-sm text-destructive">{portfoliosQuery.error.message}</p>}

      <section className="flex flex-col gap-3">
        <h2 className="text-lg font-semibold">Bank statement and fixed-income yield</h2>
        <StatementPendingSection
          workspaceId={workspaceId}
          reviewOpen={statementReviewOpen}
          onReviewOpenChange={setStatementReviewOpen}
        />
      </section>

      {portfolios.map((portfolio) => (
        <section key={portfolio.id} className="flex flex-col gap-3" data-testid="pending-portfolio">
          <h2 className="text-lg font-semibold">{portfolio.name}</h2>
          <PendingItemsSection
            portfolioId={portfolio.id}
            reviewOpen={portfolioReviewId === portfolio.id}
            onReviewOpenChange={(open) => setPortfolioReviewId(open ? portfolio.id : null)}
          />
          <DividendPendingSection portfolioId={portfolio.id} />
        </section>
      ))}
    </div>
  );
}

export default PendingPanel;
