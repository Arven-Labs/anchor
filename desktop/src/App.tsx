import { useState } from "react";
import HomeSection from "./home/HomeSection";
import StockLookupPanel, { type ResearchView } from "./stock-lookup/StockLookupPanel";
import AlertsPanel from "./alerts/AlertsPanel";
import PortfolioPanel from "./portfolio/PortfolioPanel";
import FinancesPanel from "./finances/FinancesPanel";
import WorkspaceGate from "./workspace/WorkspaceGate";
import SettingsPage from "./settings/SettingsPage";
import GlobalSearch from "./nav/GlobalSearch";
import type { AssetClass } from "./portfolio/types";
import { Button } from "@/components/ui/button";
import {
  HomeIcon,
  SearchIcon,
  WalletIcon,
  LandmarkIcon,
  BellIcon,
  SettingsIcon,
  LogOutIcon,
} from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

// Fase 10, item 8, Sessão 51 — a antiga aba solta "Crypto Score" foi
// absorvida por Research (agora um seletor de classe ali, junto com Ação/
// FII/ETF) — pedido explícito do dono do projeto pra unificar as duas
// telas. `run_crypto_collector`/`crypto_indicators` no backend não mudaram
// de nome, só o lugar de onde a UI os chama (StockLookupPanel.tsx →
// CryptoLookupSection.tsx). Sessão 54: mesma lógica pra "TruthID Sync" —
// virou uma seção dentro de Configurações (`SettingsPage.tsx` →
// `TruthIdSettingsSection.tsx`), não mais aba própria aqui. Sessão 56: a
// aba "Valuation" (seletor de modelo + 8 forms) migrou pra dentro de
// Research/análise de ação (`StockAnalysisSection.tsx` → botão "New
// Valuation" → `models/NewValuationDialog.tsx`) — os modelos servem pra
// "empresas", não só uma tela solta, e a lista "Saved Valuations" foi
// junto (mesmo componente `valuations/SavedValuationsPanel.tsx`, agora
// aberto via popup em vez de toggle de aba).
const SECTIONS = {
  home: { label: "Home", icon: HomeIcon },
  lookup: { label: "Research", icon: SearchIcon },
  portfolio: { label: "Portfolio", icon: WalletIcon },
  finances: { label: "General Finances", icon: LandmarkIcon },
  alerts: { label: "Alerts", icon: BellIcon },
} as const;

type SectionKey = keyof typeof SECTIONS;

// Sem lib de rotas — app desktop de janela única não tem barra de endereço
// pra uma URL de verdade ganhar alguma coisa. Esse estado troca a tela
// inteira (Tabs vs Configurações). A tela de chat saiu na Fase 20.
type AppView = "main" | "settings";

function App() {
  const [workspaceId, setWorkspaceId] = useState<number | null>(null);
  const [view, setView] = useState<AppView>("main");
  const [section, setSection] = useState<SectionKey>("home");
  const [assetClass, setAssetClass] = useState<AssetClass>("acao_br");
  const [activeTicker, setActiveTicker] = useState<string | null>(null);
  const [researchView, setResearchView] = useState<ResearchView>("asset");

  function handleGlobalSearch(nextAssetClass: AssetClass, ticker: string) {
    setAssetClass(nextAssetClass);
    setActiveTicker(ticker);
    setResearchView("asset");
    setSection("lookup");
  }

  function openRates() {
    setResearchView("rates");
    setSection("lookup");
  }

  if (workspaceId === null) {
    return (
      <main className="mx-auto max-w-6xl p-8">
        <WorkspaceGate onEnter={setWorkspaceId} />
      </main>
    );
  }

  if (view === "settings") {
    return (
      <main className="mx-auto max-w-6xl p-8">
        <SettingsPage onBack={() => setView("main")} />
      </main>
    );
  }

  return (
    <>
      <header className="sticky top-0 z-10 border-b border-border/80 bg-background/85 backdrop-blur supports-backdrop-filter:bg-background/60">
        {/* grid-cols 1fr/2fr/1fr — os dois lados dividem o espaço em
            frações IGUAIS entre si (por isso a busca fica de verdade no
            centro da janela, não só no meio do espaço sobrando), e o meio
            ganha o dobro pra crescer/encolher junto com o redimensionamento
            em vez de ficar travado num tamanho fixo. minmax(max-content,1fr)
            nos lados é só uma trava de segurança pra nunca cortar a navbar/
            ícones caso o conteúdo deles precise de mais que a fração exata.
            Nomes das abas ficam escondidos (só ícone + tooltip) até a
            janela passar de 1900px — com nomes visíveis a navbar fica bem
            mais larga que os ícones e puxa a centralização; abaixo desse
            tamanho fica só ícone pros dois lados baterem certo. Pedido
            explícito depois de ida e volta: centralizar sempre > nomes
            sempre visíveis. */}
        <div className="grid grid-cols-[minmax(max-content,1fr)_minmax(0,2fr)_minmax(max-content,1fr)] items-center gap-3 px-3 py-3">
          <Tabs
            value={section}
            onValueChange={(value) => setSection(value as SectionKey)}
            className="min-w-0 justify-self-start"
          >
            <TooltipProvider>
              <TabsList className="h-9 gap-0.5 rounded-xl bg-muted/60 p-1 ring-1 ring-border/60">
                {Object.entries(SECTIONS).map(([key, { label, icon: Icon }]) => (
                  <Tooltip key={key}>
                    <TooltipTrigger asChild>
                      <TabsTrigger
                        value={key}
                        className={
                          "gap-1.5 rounded-lg px-2.5 min-[1900px]:px-3" +
                          (section === key
                            ? " bg-primary/15 text-primary dark:bg-primary/15 dark:text-primary"
                            : "")
                        }
                      >
                        <Icon />
                        <span className="hidden min-[1900px]:inline">{label}</span>
                      </TabsTrigger>
                    </TooltipTrigger>
                    <TooltipContent side="bottom" className="min-[1900px]:hidden">
                      {label}
                    </TooltipContent>
                  </Tooltip>
                ))}
              </TabsList>
            </TooltipProvider>
          </Tabs>

          <div className="w-full max-w-3xl justify-self-center">
            <GlobalSearch assetClass={assetClass} onSearch={handleGlobalSearch} />
          </div>

          <div className="flex items-center gap-1 justify-self-end">
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={() => setView("settings")}
            >
              <SettingsIcon />
              <span className="sr-only">Settings</span>
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={() => setWorkspaceId(null)}
            >
              <LogOutIcon />
              <span className="sr-only">Switch Workspace</span>
            </Button>
          </div>
        </div>
      </header>

      <main className="px-4 py-6">
        <Tabs
          value={section}
          onValueChange={(value) => setSection(value as SectionKey)}
        >
          <TabsContent value="home">
            <HomeSection workspaceId={workspaceId} onOpenRates={openRates} />
          </TabsContent>

          <TabsContent value="lookup">
            <StockLookupPanel
              workspaceId={workspaceId}
              assetClass={assetClass}
              activeTicker={activeTicker}
              view={researchView}
              onOpenRates={openRates}
            />
          </TabsContent>

          <TabsContent value="portfolio">
            <PortfolioPanel workspaceId={workspaceId} />
          </TabsContent>

          <TabsContent value="finances">
            <FinancesPanel workspaceId={workspaceId} />
          </TabsContent>

          <TabsContent value="alerts">
            <AlertsPanel />
          </TabsContent>
        </Tabs>
      </main>
    </>
  );
}

export default App;
