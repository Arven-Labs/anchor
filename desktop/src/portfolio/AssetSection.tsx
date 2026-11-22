import { Fragment, useEffect, useRef, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import {
  ASSET_CLASSES,
  ASSET_CLASSES_WITH_AUTO_QUOTE,
  ASSET_CLASS_LABELS,
  BDR_UNDERLYING_CLASSES,
  type Asset,
  type AssetClass,
  type AssetFavorite,
  type ExposureType,
  type FiiCnpjSuggestion,
} from "./types";
import { latestForTicker } from "../collector/latestForTicker";
import type { StockQuote } from "../collector/types";
import FiiCvmDetails from "./FiiCvmDetails";
import ManualAssetDetails from "./ManualAssetDetails";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

type CreateAssetRequest = {
  ticker: string;
  name: string;
  asset_class: string;
  currency: string;
  exchange: string | null;
  exposure_type: string;
  exposure_value: string;
  cnpj: string | null;
  equity_shares_owned: number | null;
  equity_total_shares: number | null;
  equity_company_valuation: number | null;
  underlying_asset_class: string | null;
};

type CollectorSummary = { success: boolean; output: string };
type ToggleFavoriteRequest = { workspace_id: number; asset_id: number };
type UpdateAssetUnderlyingClassRequest = {
  asset_id: number;
  underlying_asset_class: string | null;
};

// Classes cadastradas à mão, com histórico de avaliações/saldos e anexos (`ManualAssetDetails.tsx`), e o rótulo do botão.
const MANUAL_DETAILS_LABELS: Record<string, string> = {
  imovel: "Property details",
  empresa_nao_listada: "Company details",
  previdencia: "Pension details",
};

function AssetSection({ workspaceId }: { workspaceId: number }) {
  const [ticker, setTicker] = useState("");
  const [name, setName] = useState("");
  const [assetClass, setAssetClass] = useState<AssetClass>("acao_br");
  const [currency, setCurrency] = useState("BRL");
  const [exchange, setExchange] = useState("");
  const [exposureType, setExposureType] = useState<ExposureType>("pais");
  const [exposureValue, setExposureValue] = useState("BR");
  const [cnpj, setCnpj] = useState("");
  const [equitySharesOwned, setEquitySharesOwned] = useState("");
  const [equityTotalShares, setEquityTotalShares] = useState("");
  const [equityCompanyValuation, setEquityCompanyValuation] = useState("");
  const [underlyingAssetClass, setUnderlyingAssetClass] = useState("");
  const [expandedAssetId, setExpandedAssetId] = useState<number | null>(null);
  const [manualAssetDialogAssetId, setManualAssetDialogAssetId] = useState<number | null>(null);
  const [isAddAssetDialogOpen, setIsAddAssetDialogOpen] = useState(false);

  const [tickerQuery, setTickerQuery] = useState("");
  const [activeTicker, setActiveTicker] = useState<string | null>(null);
  const autoFetchedTickerRef = useRef<string | null>(null);
  const prefilledTickerRef = useRef<string | null>(null);
  const cnpjResolvedTickerRef = useRef<string | null>(null);

  const isAutoQuoteClass = (ASSET_CLASSES_WITH_AUTO_QUOTE as string[]).includes(assetClass);
  const isFii = assetClass === "fii";
  const isCripto = assetClass === "cripto";
  const isBdr = assetClass === "bdr";
  const isMetal = assetClass === "metal";
  const isUsStock = assetClass === "acao_internacional";
  const isReit = assetClass === "reit";
  const isEtfUs = assetClass === "etf_us";
  const isEmpresaNaoListada = assetClass === "empresa_nao_listada";

  const queryClient = useQueryClient();

  const assetsQuery = useQuery<Asset[], AppError>({
    queryKey: ["assets"],
    queryFn: () => invoke("list_assets"),
  });

  // Fase 10.4 — favorito rápido (estrela), separado das watchlists nomeadas
  // (aba própria "Watchlists"): mesma query cacheada sob a chave
  // ["asset-favorites", workspaceId] usada lá, pra ficar em sincronia.
  const favoritesQuery = useQuery<AssetFavorite[], AppError>({
    queryKey: ["asset-favorites", workspaceId],
    queryFn: () => invoke("list_favorite_assets", { workspaceId }),
  });
  const favoriteAssetIds = new Set((favoritesQuery.data ?? []).map((f) => f.asset_id));

  const toggleFavoriteMutation = useMutation<boolean, AppError, number>({
    mutationFn: (assetId) =>
      invoke("toggle_favorite", {
        request: { workspace_id: workspaceId, asset_id: assetId } satisfies ToggleFavoriteRequest,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["asset-favorites", workspaceId] });
    },
  });

  // Fase 10, item 8, Sessão 86 — edição do subjacente de um BDR já
  // cadastrado, direto na tabela (sem popup, é um campo só). Mesmo padrão
  // de invalidação do toggleFavoriteMutation acima.
  const updateUnderlyingClassMutation = useMutation<
    Asset,
    AppError,
    UpdateAssetUnderlyingClassRequest
  >({
    mutationFn: (request) => invoke("update_asset_underlying_class", { request }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["assets"] });
    },
  });

  // Só busca cotação pras classes de ASSET_CLASSES_WITH_AUTO_QUOTE (Ação
  // BR/FII, mesmo endpoint Yahoo) — as outras ainda não têm coletor
  // integrado (ver Fase 10, item 8), então continuam com cadastro manual.
  const lookupQuery = useQuery<StockQuote | null, AppError>({
    queryKey: ["asset-section-stock-quote", activeTicker],
    enabled: isAutoQuoteClass && activeTicker !== null,
    queryFn: async () => {
      const quotes = await invoke<StockQuote[]>("list_stock_quotes");
      return latestForTicker(quotes, activeTicker as string);
    },
  });

  const collectorMutation = useMutation<CollectorSummary, AppError, string>({
    mutationFn: (t) =>
      invoke<CollectorSummary>("run_stock_collector", {
        ticker: t,
        assetClass: isCripto
          ? "cripto"
          : isMetal
            ? "metal"
            : isUsStock
              ? "acao_internacional"
              : isReit
                ? "reit"
                : isEtfUs
                  ? "etf_us"
                  : null,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["asset-section-stock-quote", activeTicker] });
    },
  });

  // Fase 10, item 8, Sessão 41 — só pra FII: tenta sugerir o CNPJ do fundo
  // assim que a cotação chega (mesmo gatilho do prefill abaixo). A bolsai
  // só é chamada aqui, uma vez por busca — depois de confirmado e salvo,
  // toda leitura recorrente (FiiCvmDetails) usa só a CVM.
  const resolveCnpjMutation = useMutation<FiiCnpjSuggestion | null, AppError, string>({
    mutationFn: (t) => invoke("resolve_fii_cnpj", { ticker: t }),
  });

  // Mesmo padrão do Stock Lookup (Fase 9): um ticker sem dado no banco
  // dispara o coletor no máximo uma vez por busca, guardado pelo ref.
  useEffect(() => {
    if (
      isAutoQuoteClass &&
      activeTicker &&
      lookupQuery.isSuccess &&
      lookupQuery.data === null &&
      autoFetchedTickerRef.current !== activeTicker
    ) {
      autoFetchedTickerRef.current = activeTicker;
      collectorMutation.mutate(activeTicker);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isAutoQuoteClass, activeTicker, lookupQuery.isSuccess, lookupQuery.data]);

  // Pré-preenche o preview editável assim que a cotação chega — só na
  // primeira vez por ticker, pra não sobrescrever edições manuais do
  // usuário em buscas repetidas (ex: "Refresh" implícito de reabrir a tela).
  useEffect(() => {
    const quote = lookupQuery.data;
    if (isAutoQuoteClass && activeTicker && quote && prefilledTickerRef.current !== activeTicker) {
      prefilledTickerRef.current = activeTicker;
      setTicker(activeTicker);
      setName(quote.name ?? "");
      setCurrency(quote.currency ?? "BRL");
      setExchange(quote.exchange ?? "");
      if (isCripto) {
        setExposureType("categoria_especial");
        setExposureValue("crypto");
      } else if (isBdr) {
        // BDR representa empresa estrangeira — o Yahoo não devolve o país
        // real por trás do ticker, então "US" é só um chute razoável (a
        // maioria dos BDRs na B3 é de empresa americana), editável, igual
        // o "BR" das outras classes já é (decisão do dono do projeto,
        // Sessão 53).
        setExposureType("pais");
        setExposureValue("US");
      } else if (isMetal) {
        setExposureType("categoria_especial");
        setExposureValue("gold_metal");
      } else if (isUsStock || isReit || isEtfUs) {
        // Mesmo padrão do BDR: "US" é o chute óbvio pra ação/REIT/ETF
        // americano (NYSE/NASDAQ), editável.
        setExposureType("pais");
        setExposureValue("US");
      } else {
        setExposureType("pais");
        setExposureValue("BR");
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    isAutoQuoteClass,
    isCripto,
    isBdr,
    isMetal,
    isUsStock,
    isReit,
    isEtfUs,
    activeTicker,
    lookupQuery.data,
  ]);

  // Sugestão de CNPJ (só FII) — dispara junto com o prefill acima, uma vez
  // por ticker. `cnpj` fica vazio se não achar sugestão confiável (o
  // usuário cola manualmente, ou deixa em branco e resolve depois pelo
  // painel de detalhes CVM via `update_asset_cnpj`).
  useEffect(() => {
    const quote = lookupQuery.data;
    if (isFii && activeTicker && quote && cnpjResolvedTickerRef.current !== activeTicker) {
      cnpjResolvedTickerRef.current = activeTicker;
      resolveCnpjMutation.mutate(activeTicker, {
        onSuccess: (suggestion) => setCnpj(suggestion?.cnpj ?? ""),
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isFii, activeTicker, lookupQuery.data]);

  // Trocar de classe pra fora da busca automática limpa a busca em
  // andamento, pra não deixar um preview de ticker antigo aparecendo se
  // voltar depois.
  useEffect(() => {
    if (!isAutoQuoteClass) {
      setActiveTicker(null);
      setTickerQuery("");
      autoFetchedTickerRef.current = null;
      prefilledTickerRef.current = null;
    }
    if (!isFii) {
      setCnpj("");
      cnpjResolvedTickerRef.current = null;
    }
    if (!isBdr) {
      setUnderlyingAssetClass("");
    }
  }, [isAutoQuoteClass, isFii, isBdr]);

  const createMutation = useMutation<Asset, AppError, CreateAssetRequest>({
    mutationFn: (request) => invoke("create_asset", { request }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["assets"] });
      setTicker("");
      setName("");
      setExchange("");
      setCnpj("");
      setEquitySharesOwned("");
      setEquityTotalShares("");
      setEquityCompanyValuation("");
      setUnderlyingAssetClass("");
      setTickerQuery("");
      setActiveTicker(null);
      autoFetchedTickerRef.current = null;
      prefilledTickerRef.current = null;
      cnpjResolvedTickerRef.current = null;
    },
  });

  function handleTickerSearch(event: FormEvent) {
    event.preventDefault();
    const normalized = tickerQuery.trim().toUpperCase();
    if (!normalized) return;
    setActiveTicker(normalized);
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    createMutation.mutate({
      ticker: ticker.toUpperCase(),
      name,
      asset_class: assetClass,
      currency: currency.toUpperCase(),
      exchange: exchange.trim() === "" ? null : exchange,
      exposure_type: exposureType,
      exposure_value: exposureValue,
      cnpj: isFii && cnpj.trim() !== "" ? cnpj.trim() : null,
      equity_shares_owned:
        isEmpresaNaoListada && equitySharesOwned.trim() !== "" ? Number(equitySharesOwned) : null,
      equity_total_shares:
        isEmpresaNaoListada && equityTotalShares.trim() !== "" ? Number(equityTotalShares) : null,
      equity_company_valuation:
        isEmpresaNaoListada && equityCompanyValuation.trim() !== ""
          ? Number(equityCompanyValuation)
          : null,
      underlying_asset_class:
        isBdr && underlyingAssetClass !== "" ? underlyingAssetClass : null,
    });
  }

  const assets = assetsQuery.data ?? [];
  const showCreateForm = !isAutoQuoteClass || (activeTicker !== null && lookupQuery.data != null);

  return (
    <>
    <Card>
      <CardHeader>
        <CardTitle>Assets</CardTitle>
      </CardHeader>
      <CardContent>
        <p className="mb-4 text-sm text-muted-foreground">
          Catalog of tradeable/registrable assets — shared across every Portfolio in the
          Workspace. Stock (B3), FII (B3), ETF (B3), Crypto, BDR (B3), Metal, International stock
          and REIT fetch data automatically by ticker; the other classes (Tesouro Direto, Fixed
          income, Real estate, Private company) still use manual registration for now. Real
          estate and Private company also get a valuation history and document attachments — see
          the details popup below.
        </p>

        <div className="mb-4 flex justify-end">
          <Button type="button" onClick={() => setIsAddAssetDialogOpen(true)}>
            Add asset
          </Button>
        </div>

        {assetsQuery.isError && (
          <p className="mb-3 text-red-600">{assetsQuery.error.message}</p>
        )}

        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Ticker</TableHead>
              <TableHead>Name</TableHead>
              <TableHead>Class</TableHead>
              <TableHead>Currency</TableHead>
              <TableHead>Exchange</TableHead>
              <TableHead>Exposure</TableHead>
              <TableHead>Underlying</TableHead>
              <TableHead>★</TableHead>
              <TableHead></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {assets.length === 0 && (
              <TableRow>
                <TableCell colSpan={9} className="text-center text-muted-foreground">
                  No assets registered yet.
                </TableCell>
              </TableRow>
            )}
            {assets.map((asset) => {
              const isFavorite = favoriteAssetIds.has(asset.id);
              const isExpanded = expandedAssetId === asset.id;
              return (
                <Fragment key={asset.id}>
                  <TableRow>
                    <TableCell>{asset.ticker}</TableCell>
                    <TableCell>{asset.name}</TableCell>
                    <TableCell>
                      {ASSET_CLASS_LABELS[asset.asset_class as AssetClass] ?? asset.asset_class}
                    </TableCell>
                    <TableCell>{asset.currency}</TableCell>
                    <TableCell>{asset.exchange ?? "—"}</TableCell>
                    <TableCell>
                      {asset.exposure_type === "pais" ? "🌍 " : "🏷️ "}
                      {asset.exposure_value}
                    </TableCell>
                    <TableCell>
                      {asset.asset_class === "bdr" ? (
                        <Select
                          value={asset.underlying_asset_class ?? "unspecified"}
                          onValueChange={(value) =>
                            updateUnderlyingClassMutation.mutate({
                              asset_id: asset.id,
                              underlying_asset_class: value === "unspecified" ? null : value,
                            })
                          }
                        >
                          <SelectTrigger className="w-[9rem]" size="sm">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value="unspecified">Not specified</SelectItem>
                            {BDR_UNDERLYING_CLASSES.map((key) => (
                              <SelectItem key={key} value={key}>
                                {ASSET_CLASS_LABELS[key]}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      ) : (
                        "—"
                      )}
                    </TableCell>
                    <TableCell>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        disabled={toggleFavoriteMutation.isPending}
                        onClick={() => toggleFavoriteMutation.mutate(asset.id)}
                        aria-label={isFavorite ? "Remove from favorites" : "Add to favorites"}
                      >
                        {isFavorite ? "★" : "☆"}
                      </Button>
                    </TableCell>
                    <TableCell>
                      {asset.asset_class === "fii" && (
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => setExpandedAssetId(isExpanded ? null : asset.id)}
                        >
                          {isExpanded ? "Hide CVM" : "CVM data"}
                        </Button>
                      )}
                      {asset.asset_class in MANUAL_DETAILS_LABELS && (
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => setManualAssetDialogAssetId(asset.id)}
                        >
                          {MANUAL_DETAILS_LABELS[asset.asset_class]}
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                  {isExpanded && asset.asset_class === "fii" && (
                    <TableRow>
                      <TableCell colSpan={9} className="bg-muted/30">
                        <FiiCvmDetails asset={asset} />
                      </TableCell>
                    </TableRow>
                  )}
                </Fragment>
              );
            })}
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <Dialog open={isAddAssetDialogOpen} onOpenChange={setIsAddAssetDialogOpen}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Add asset</DialogTitle>
        </DialogHeader>

        <div className="max-w-xs">
          <Field label="Asset class">
            <Select value={assetClass} onValueChange={(value) => setAssetClass(value as AssetClass)}>
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {ASSET_CLASSES.map((key) => (
                  <SelectItem key={key} value={key}>
                    {ASSET_CLASS_LABELS[key]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
        </div>

        {isAutoQuoteClass && (
          <form onSubmit={handleTickerSearch} className="flex items-end gap-3">
            <Field
              label={
                isCripto || isMetal || isUsStock || isReit || isEtfUs
                  ? "Search ticker"
                  : "Search ticker (B3)"
              }
              className="flex-1"
            >
              <Input
                required
                placeholder={
                  assetClass === "fii"
                    ? "HGLG11"
                    : assetClass === "etf_br"
                      ? "BOVA11"
                      : isCripto
                        ? "ETH"
                        : isBdr
                          ? "AAPL34"
                          : isMetal
                            ? "XAU"
                            : isUsStock
                              ? "AAPL"
                              : isReit
                                ? "O"
                                : isEtfUs
                                  ? "SPY"
                                  : "PETR4"
                }
                value={tickerQuery}
                onChange={(e) => setTickerQuery(e.currentTarget.value)}
              />
            </Field>
            <Button type="submit">Search</Button>
          </form>
        )}

        {isAutoQuoteClass && lookupQuery.isError && (
          <p className="text-red-600">{lookupQuery.error.message}</p>
        )}
        {isAutoQuoteClass && collectorMutation.isError && (
          <p className="text-red-600">{collectorMutation.error.message}</p>
        )}
        {isAutoQuoteClass && activeTicker && lookupQuery.isLoading && (
          <p className="text-muted-foreground">Loading {activeTicker}...</p>
        )}
        {isAutoQuoteClass && activeTicker && lookupQuery.data === null && collectorMutation.isPending && (
          <p className="text-muted-foreground">Fetching {activeTicker} for the first time...</p>
        )}
        {isAutoQuoteClass &&
          activeTicker &&
          lookupQuery.isSuccess &&
          lookupQuery.data === null &&
          !collectorMutation.isPending && (
            <p className="text-muted-foreground">No data found for {activeTicker}.</p>
          )}

        {showCreateForm && (
          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
              <Field label="Ticker / identifier">
                <Input
                  required
                  disabled={isAutoQuoteClass}
                  placeholder="e.g.: PETR4, Tesouro IPCA+ 2035, Apt 302 - Rua X"
                  value={ticker}
                  onChange={(e) => setTicker(e.currentTarget.value)}
                />
              </Field>
              <Field label="Name" className="sm:col-span-2">
                <Input
                  required
                  placeholder="e.g.: Petrobras PN"
                  value={name}
                  onChange={(e) => setName(e.currentTarget.value)}
                />
              </Field>
            </div>

            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <Field label="Currency">
                <Input
                  required
                  placeholder="BRL, USD..."
                  value={currency}
                  onChange={(e) => setCurrency(e.currentTarget.value)}
                />
              </Field>
              <Field label="Exchange/listing (optional)">
                <Input
                  placeholder="e.g.: B3, NASDAQ"
                  value={exchange}
                  onChange={(e) => setExchange(e.currentTarget.value)}
                />
              </Field>
            </div>

            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <Field label="Exposure type">
                <Select
                  value={exposureType}
                  onValueChange={(value) => setExposureType(value as ExposureType)}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="pais">Country</SelectItem>
                    <SelectItem value="categoria_especial">Special category</SelectItem>
                  </SelectContent>
                </Select>
              </Field>
              <Field
                label={
                  exposureType === "pais"
                    ? "Exposure country (e.g.: BR, US)"
                    : "Category (e.g.: crypto, gold_metal)"
                }
              >
                <Input
                  required
                  value={exposureValue}
                  onChange={(e) => setExposureValue(e.currentTarget.value)}
                />
              </Field>
            </div>

            {isBdr && (
              <Field label="Underlying asset type (optional — what the BDR represents behind the receipt)">
                <Select
                  value={underlyingAssetClass === "" ? "unspecified" : underlyingAssetClass}
                  onValueChange={(value) =>
                    setUnderlyingAssetClass(value === "unspecified" ? "" : value)
                  }
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="unspecified">Not specified</SelectItem>
                    {BDR_UNDERLYING_CLASSES.map((key) => (
                      <SelectItem key={key} value={key}>
                        {ASSET_CLASS_LABELS[key]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            )}

            {isFii && (
              <Field label="Fund CNPJ (to pull indicators from CVM — vacancy, delinquency, net assets)">
                <Input
                  placeholder="00.000.000/0001-00"
                  value={cnpj}
                  onChange={(e) => setCnpj(e.currentTarget.value)}
                />
                {resolveCnpjMutation.isPending && (
                  <p className="mt-1 text-sm text-muted-foreground">Looking up CNPJ...</p>
                )}
                {resolveCnpjMutation.isSuccess && resolveCnpjMutation.data === null && (
                  <p className="mt-1 text-sm text-muted-foreground">
                    Couldn't find the CNPJ automatically — paste it manually (fund's website or
                    CVM) or leave it blank and resolve it later from the CVM details panel.
                  </p>
                )}
              </Field>
            )}

            {isEmpresaNaoListada && (
              <div className="flex flex-col gap-2">
                <p className="text-sm text-muted-foreground">
                  Equity stake (optional at registration — editable later from the details popup).
                  Percentage is always calculated from shares owned / total company shares, never
                  typed in directly.
                </p>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                  <Field label="Shares owned">
                    <Input
                      type="number"
                      step="any"
                      min="0"
                      value={equitySharesOwned}
                      onChange={(e) => setEquitySharesOwned(e.currentTarget.value)}
                    />
                  </Field>
                  <Field label="Total company shares">
                    <Input
                      type="number"
                      step="any"
                      min="0"
                      value={equityTotalShares}
                      onChange={(e) => setEquityTotalShares(e.currentTarget.value)}
                    />
                  </Field>
                  <Field label={`Company valuation (${currency || "BRL"})`}>
                    <Input
                      type="number"
                      step="0.01"
                      min="0"
                      value={equityCompanyValuation}
                      onChange={(e) => setEquityCompanyValuation(e.currentTarget.value)}
                    />
                  </Field>
                </div>
                {equitySharesOwned.trim() !== "" &&
                  equityTotalShares.trim() !== "" &&
                  Number(equityTotalShares) > 0 && (
                    <p className="text-sm text-muted-foreground">
                      Your stake: {((Number(equitySharesOwned) / Number(equityTotalShares)) * 100).toFixed(2)}%
                      {equityCompanyValuation.trim() !== "" &&
                        ` (~${(
                          (Number(equitySharesOwned) / Number(equityTotalShares)) *
                          Number(equityCompanyValuation)
                        ).toLocaleString("pt-BR", {
                          style: "currency",
                          currency: currency || "BRL",
                        })})`}
                    </p>
                  )}
              </div>
            )}

            {createMutation.isError && (
              <p className="text-red-600">{createMutation.error.message}</p>
            )}

            <Button type="submit" disabled={createMutation.isPending} className="w-fit">
              {createMutation.isPending ? "Adding..." : "Add asset"}
            </Button>
          </form>
        )}
      </DialogContent>
    </Dialog>

    <Dialog
      open={manualAssetDialogAssetId !== null}
      onOpenChange={(open) => !open && setManualAssetDialogAssetId(null)}
    >
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
        {manualAssetDialogAssetId !== null &&
          (() => {
            const asset = assets.find((a) => a.id === manualAssetDialogAssetId);
            if (!asset) return null;
            return (
              <>
                <DialogHeader>
                  <DialogTitle>
                    {MANUAL_DETAILS_LABELS[asset.asset_class]} —{" "}
                    {asset.name}
                  </DialogTitle>
                </DialogHeader>
                <ManualAssetDetails asset={asset} workspaceId={workspaceId} />
              </>
            );
          })()}
      </DialogContent>
    </Dialog>
    </>
  );
}

export default AssetSection;
