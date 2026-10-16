import { useState, type FormEvent } from "react";
import { SearchIcon } from "lucide-react";
import {
  ASSET_CLASSES_WITH_AUTO_QUOTE,
  type AssetClass,
} from "../portfolio/types";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// Rótulos curtos só pro gatilho do seletor (o dropdown continua com o nome
// completo) — precisa caber ao lado do campo de busca na navbar.
const SHORT_CLASS_LABELS: Record<AssetClass, string> = {
  acao_br: "B3",
  fii: "FII",
  etf_br: "ETF",
  cripto: "Crypto",
  bdr: "BDR",
  metal: "Metal",
  acao_internacional: "Intl",
  reit: "REIT",
  etf_us: "ETF US",
  tesouro_direto: "Tesouro",
  renda_fixa: "Renda fixa",
  imovel: "Imóvel",
  empresa_nao_listada: "Empresa",
};

const CLASS_LABELS: Record<AssetClass, string> = {
  acao_br: "Stock (B3)",
  fii: "FII (B3)",
  etf_br: "ETF (B3)",
  cripto: "Crypto",
  bdr: "BDR (B3)",
  metal: "Metal",
  acao_internacional: "International stock",
  reit: "REIT",
  etf_us: "ETF (US)",
  tesouro_direto: "Tesouro Direto",
  renda_fixa: "Fixed income",
  imovel: "Real estate",
  empresa_nao_listada: "Private company",
};

const PLACEHOLDER_BY_CLASS: Partial<Record<AssetClass, string>> = {
  fii: "HGLG11",
  etf_br: "BOVA11",
  cripto: "ETH",
  bdr: "AAPL34",
  metal: "XAU",
  acao_internacional: "AAPL",
  reit: "O",
  etf_us: "SPY",
};

function GlobalSearch({
  assetClass,
  onSearch,
}: {
  assetClass: AssetClass;
  onSearch: (assetClass: AssetClass, ticker: string) => void;
}) {
  const [tickerInput, setTickerInput] = useState("");
  const [selectedClass, setSelectedClass] = useState<AssetClass>(assetClass);

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const normalized = tickerInput.trim().toUpperCase();
    if (!normalized) return;
    onSearch(selectedClass, normalized);
  }

  return (
    <form onSubmit={handleSubmit} className="flex w-full min-w-0 items-center">
      <div className="flex h-9 min-w-0 flex-1 items-center rounded-lg border border-input bg-input/30 focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50">
        <Select
          value={selectedClass}
          onValueChange={(value) => setSelectedClass(value as AssetClass)}
        >
          <SelectTrigger
            data-testid="global-search-class"
            size="sm"
            className="h-9 w-24 shrink-0 rounded-r-none border-none bg-transparent text-muted-foreground shadow-none focus-visible:ring-0"
          >
            <SelectValue>{SHORT_CLASS_LABELS[selectedClass]}</SelectValue>
          </SelectTrigger>
          <SelectContent align="start">
            {ASSET_CLASSES_WITH_AUTO_QUOTE.map((key) => (
              <SelectItem key={key} value={key}>
                {CLASS_LABELS[key]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <div className="h-4 w-px shrink-0 bg-border" />
        <Input
          data-testid="global-search-ticker"
          value={tickerInput}
          onChange={(e) => setTickerInput(e.currentTarget.value)}
          placeholder={PLACEHOLDER_BY_CLASS[selectedClass] ?? "PETR4"}
          className="h-9 w-full min-w-0 flex-1 rounded-l-none border-none bg-transparent shadow-none focus-visible:ring-0"
        />
      </div>
      <Button
        type="submit"
        variant="ghost"
        size="icon-sm"
        className="ml-1 shrink-0"
        data-testid="global-search-submit"
      >
        <SearchIcon />
        <span className="sr-only">Search</span>
      </Button>
    </form>
  );
}

export default GlobalSearch;
