import { SearchIcon } from "lucide-react";
import type { AssetClass } from "../portfolio/types";
import StockAnalysisSection from "./StockAnalysisSection";
import FiiLookupSection from "./FiiLookupSection";
import EtfLookupSection from "./EtfLookupSection";
import CryptoLookupSection from "./CryptoLookupSection";
import BdrLookupSection from "./BdrLookupSection";
import MetalLookupSection from "./MetalLookupSection";
import UsStockLookupSection from "./UsStockLookupSection";
import ReitLookupSection from "./ReitLookupSection";
import EtfUsLookupSection from "./EtfUsLookupSection";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

/// Fase 10, item 8, Sessão 43 — "Pesquisa" virou a tela unificada de
/// análise pra qualquer classe de ativo com dado automático, em vez de uma
/// aba por classe. Pedido explícito do dono do projeto depois de eu sugerir
/// telas separadas: "queria deixar o app completo mas enxuto, pq se toda
/// feature for adicionar uma aba nova...". Também rejeitou a ideia de
/// detectar a classe tentando um fetch atrás do outro — prefere escolher a
/// classe explicitamente antes de buscar. Este componente é só a moldura
/// (busca + seletor de classe); todo o conteúdo específico de cada classe
/// mora em `StockAnalysisSection.tsx`/`FiiLookupSection.tsx`/
/// `EtfLookupSection.tsx` (este último desde a Sessão 50, sem fundamentos/
/// DCF/valuation — nenhum desses conceitos existe pra um fundo de índice)/
/// `CryptoLookupSection.tsx` (Sessão 51 — absorveu a antiga aba solta
/// "Crypto Score", removida do nav principal)/`BdrLookupSection.tsx`
/// (Sessão 53 — irmã quase idêntica de `EtfLookupSection.tsx`, mesma
/// ausência de fundamentos/DCF, bolsai não tem pra BDR)/`MetalLookupSection.tsx`
/// (Sessão 55 — só ouro por ora, fonte Yahoo sem `.SA`, sem
/// fundamentos/dividendos, que não fazem sentido pra metal)/
/// `UsStockLookupSection.tsx` (Fatia 1 — ação americana, fonte Yahoo sem
/// `.SA`, sem fundamentos/DCF nesta fatia — SEC EDGAR fica pra depois)/
/// `ReitLookupSection.tsx` (REIT — indicadores imobiliários via SEC EDGAR,
/// sem os 8 modelos de valuation)/`EtfUsLookupSection.tsx` (ETF americano —
/// irmã de `EtfLookupSection.tsx` com a mesma fonte Yahoo sem `.SA` de
/// `UsStockLookupSection.tsx`/`ReitLookupSection.tsx`, sem fundamentos), que
/// só recebem o ticker já commitado — cada uma cuida do próprio fetch/estado,
/// sem nada compartilhado entre elas além do visual (`shared.tsx`).
///
/// Sessão seguinte à Fase 16 — a busca (classe + ticker) saiu daqui e virou
/// `nav/GlobalSearch.tsx`, fixa na navbar em `App.tsx`, pedido explícito do
/// dono do projeto ("queria deixar o campo de pesquisa no topo, do lado da
/// navbar"). Este componente agora só recebe `assetClass`/`activeTicker` já
/// commitados e desenha o resultado.
function StockLookupPanel({
  workspaceId,
  assetClass,
  activeTicker,
}: {
  workspaceId: number;
  assetClass: AssetClass;
  activeTicker: string | null;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Research</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-6">
        {!activeTicker && (
          <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed border-border py-10 text-center text-muted-foreground">
            <SearchIcon className="size-5" />
            <p className="text-sm">
              Use a busca no topo da tela pra analisar um ativo.
            </p>
          </div>
        )}

        {activeTicker &&
          (assetClass === "fii" ? (
            <FiiLookupSection key={activeTicker} ticker={activeTicker} />
          ) : assetClass === "etf_br" ? (
            <EtfLookupSection key={activeTicker} ticker={activeTicker} />
          ) : assetClass === "cripto" ? (
            <CryptoLookupSection key={activeTicker} ticker={activeTicker} />
          ) : assetClass === "bdr" ? (
            <BdrLookupSection key={activeTicker} ticker={activeTicker} />
          ) : assetClass === "metal" ? (
            <MetalLookupSection key={activeTicker} ticker={activeTicker} />
          ) : assetClass === "reit" ? (
            <ReitLookupSection key={activeTicker} ticker={activeTicker} />
          ) : assetClass === "etf_us" ? (
            <EtfUsLookupSection key={activeTicker} ticker={activeTicker} />
          ) : assetClass === "acao_internacional" ? (
            <UsStockLookupSection
              key={activeTicker}
              ticker={activeTicker}
              workspaceId={workspaceId}
            />
          ) : (
            <StockAnalysisSection
              key={activeTicker}
              ticker={activeTicker}
              workspaceId={workspaceId}
            />
          ))}
      </CardContent>
    </Card>
  );
}

export default StockLookupPanel;
