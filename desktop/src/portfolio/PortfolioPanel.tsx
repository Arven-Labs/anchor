import { useEffect, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { Portfolio } from "./types";
import SummarySection from "./SummarySection";
import PositionsSection from "./PositionsSection";
import LedgerSection from "./LedgerSection";
import DividendsSummarySection from "./DividendsSummarySection";
import CustodiaSection from "./CustodiaSection";
import AssetSection from "./AssetSection";
import TransactionSection from "./TransactionSection";
import ProfitabilitySection from "./ProfitabilitySection";
import BacktestSection from "./BacktestSection";
import TaxesSection from "./TaxesSection";
import OptionPositionsSection from "./OptionPositionsSection";
import WatchlistSection from "./WatchlistSection";
import ThesisSection from "./ThesisSection";
import Field from "../components/Field";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

type PortfolioSection =
  | "summary"
  | "positions"
  | "ledger"
  | "dividends_summary"
  | "transactions"
  | "profitability"
  | "backtest"
  | "taxes"
  | "options"
  | "assets"
  | "custodias"
  | "watchlists"
  | "theses";

const PORTFOLIO_SECTIONS: Record<PortfolioSection, string> = {
  summary: "Summary",
  positions: "Positions",
  ledger: "Ledger",
  dividends_summary: "Dividends",
  transactions: "Transactions & Positions",
  profitability: "Profitability",
  backtest: "Backtest",
  taxes: "Taxes",
  options: "Options",
  assets: "Assets",
  custodias: "Custodies",
  watchlists: "Watchlists",
  theses: "Theses",
};

type CreatePortfolioRequest = {
  workspace_id: number;
  name: string;
  description: string | null;
};

type RenamePortfolioRequest = {
  portfolio_id: number;
  name: string;
};

// Fase 10.1 — fundação de Workspace/Portfolio (single-user, sem
// WorkspaceMember/convite ainda, ver PHASE.md item 10.1) + a primeira
// fatia de multi-ativos (10.2, escopo Sessão 29: Ação BR/Stocks intl/
// Tesouro Direto/Renda Fixa) e Gestão de Custódias básica (item 9).
// `workspaceId` chega resolvido pela tela de entrada (Fase 10.6,
// `WorkspaceGate.tsx`) — não é mais buscado aqui.
function PortfolioPanel({ workspaceId }: { workspaceId: number }) {
  const [selectedPortfolioId, setSelectedPortfolioId] = useState<number | null>(null);
  const [section, setSection] = useState<PortfolioSection>("summary");
  const [newPortfolioName, setNewPortfolioName] = useState("");
  const [showNewPortfolioForm, setShowNewPortfolioForm] = useState(false);
  const [renamingPortfolio, setRenamingPortfolio] = useState(false);
  const [renameValue, setRenameValue] = useState("");

  const queryClient = useQueryClient();

  const portfoliosQuery = useQuery<Portfolio[], AppError>({
    queryKey: ["portfolios", workspaceId],
    queryFn: () => invoke("list_portfolios", { workspaceId }),
  });

  const portfolios = portfoliosQuery.data ?? [];

  // Seleciona a primeira Portfolio assim que a lista chega, se nada foi
  // escolhido ainda (Workspace nasce com uma "Carteira Principal" semeada
  // pela migration — quase sempre só existe essa mesmo).
  useEffect(() => {
    if (selectedPortfolioId === null && portfolios.length > 0) {
      setSelectedPortfolioId(portfolios[0].id);
    }
  }, [portfolios, selectedPortfolioId]);

  const createPortfolioMutation = useMutation<Portfolio, AppError, CreatePortfolioRequest>({
    mutationFn: (request) => invoke("create_portfolio", { request }),
    onSuccess: (created) => {
      queryClient.invalidateQueries({ queryKey: ["portfolios", workspaceId] });
      setSelectedPortfolioId(created.id);
      setNewPortfolioName("");
      setShowNewPortfolioForm(false);
    },
  });

  function handleCreatePortfolio(event: FormEvent) {
    event.preventDefault();
    createPortfolioMutation.mutate({
      workspace_id: workspaceId,
      name: newPortfolioName,
      description: null,
    });
  }

  const renamePortfolioMutation = useMutation<Portfolio, AppError, RenamePortfolioRequest>({
    mutationFn: (request) => invoke("rename_portfolio", { request }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["portfolios", workspaceId] });
      setRenamingPortfolio(false);
    },
  });

  function handleRenameSubmit(event: FormEvent) {
    event.preventDefault();
    if (selectedPortfolioId === null) return;
    renamePortfolioMutation.mutate({ portfolio_id: selectedPortfolioId, name: renameValue });
  }

  if (portfoliosQuery.isError) {
    return <p className="text-red-600">{portfoliosQuery.error.message}</p>;
  }
  if (portfoliosQuery.isLoading) {
    return <p className="text-muted-foreground">Loading...</p>;
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-end gap-4">
        <Field label="Portfolio" className="flex-1">
          <Select
            value={selectedPortfolioId !== null ? String(selectedPortfolioId) : ""}
            onValueChange={(value) => setSelectedPortfolioId(Number(value))}
          >
            <SelectTrigger className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {portfolios.map((portfolio) => (
                <SelectItem key={portfolio.id} value={String(portfolio.id)}>
                  {portfolio.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <Button
          type="button"
          variant="outline"
          onClick={() => {
            const current = portfolios.find((p) => p.id === selectedPortfolioId);
            setRenameValue(current?.name ?? "");
            setRenamingPortfolio((v) => !v);
          }}
          disabled={selectedPortfolioId === null}
        >
          Rename
        </Button>
        <Button
          type="button"
          variant="outline"
          onClick={() => setShowNewPortfolioForm((v) => !v)}
        >
          New portfolio
        </Button>
      </div>

      {renamingPortfolio && (
        <form onSubmit={handleRenameSubmit} className="flex items-end gap-4">
          <Field label="New portfolio name" className="flex-1">
            <Input
              required
              value={renameValue}
              onChange={(e) => setRenameValue(e.currentTarget.value)}
            />
          </Field>
          <Button type="submit" disabled={renamePortfolioMutation.isPending}>
            {renamePortfolioMutation.isPending ? "Saving..." : "Save"}
          </Button>
        </form>
      )}

      {showNewPortfolioForm && (
        <form onSubmit={handleCreatePortfolio} className="flex items-end gap-4">
          <Field label="New portfolio's name" className="flex-1">
            <Input
              required
              value={newPortfolioName}
              onChange={(e) => setNewPortfolioName(e.currentTarget.value)}
            />
          </Field>
          <Button type="submit" disabled={createPortfolioMutation.isPending}>
            {createPortfolioMutation.isPending ? "Creating..." : "Create"}
          </Button>
        </form>
      )}

      {selectedPortfolioId !== null && (
        <Tabs value={section} onValueChange={(value) => setSection(value as PortfolioSection)}>
          <TabsList className="mb-4">
            {Object.entries(PORTFOLIO_SECTIONS).map(([key, label]) => (
              <TabsTrigger key={key} value={key} data-testid={`portfolio-tab-${key}`}>
                {label}
              </TabsTrigger>
            ))}
          </TabsList>

          <TabsContent value="summary">
            <SummarySection portfolioId={selectedPortfolioId} />
          </TabsContent>
          <TabsContent value="positions">
            <PositionsSection portfolioId={selectedPortfolioId} />
          </TabsContent>
          <TabsContent value="ledger">
            <LedgerSection portfolioId={selectedPortfolioId} />
          </TabsContent>
          <TabsContent value="dividends_summary">
            <DividendsSummarySection portfolioId={selectedPortfolioId} />
          </TabsContent>
          <TabsContent value="transactions">
            <TransactionSection workspaceId={workspaceId} portfolioId={selectedPortfolioId} />
          </TabsContent>
          <TabsContent value="profitability">
            <ProfitabilitySection portfolioId={selectedPortfolioId} />
          </TabsContent>
          <TabsContent value="backtest">
            <BacktestSection portfolioId={selectedPortfolioId} />
          </TabsContent>
          <TabsContent value="taxes">
            <TaxesSection workspaceId={workspaceId} />
          </TabsContent>
          <TabsContent value="options">
            <OptionPositionsSection portfolioId={selectedPortfolioId} />
          </TabsContent>
          <TabsContent value="assets">
            <AssetSection workspaceId={workspaceId} />
          </TabsContent>
          <TabsContent value="custodias">
            <CustodiaSection workspaceId={workspaceId} />
          </TabsContent>
          <TabsContent value="watchlists">
            <WatchlistSection workspaceId={workspaceId} />
          </TabsContent>
          <TabsContent value="theses">
            <ThesisSection workspaceId={workspaceId} />
          </TabsContent>
        </Tabs>
      )}
    </div>
  );
}

export default PortfolioPanel;
