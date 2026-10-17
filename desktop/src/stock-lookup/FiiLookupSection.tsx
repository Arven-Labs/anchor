import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import type { FiiCnpjSuggestion, FiiCvmMonthly, FiiCvmProperty } from "../portfolio/types";
import { latestForTicker } from "../collector/latestForTicker";
import type {
  StockDividendPayment,
  StockDividendsAvg,
  StockPriceHistory,
  StockQuote,
} from "../collector/types";
import DividendHistoryChart from "./DividendHistoryChart";
import PriceHistoryChart from "./PriceHistoryChart";
import {
  AddToAssetsButton,
  CompanyLogo,
  StatTile,
  formatCurrency,
  formatFractionAsPercent,
  formatRatio,
  type StockNote,
} from "./shared";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

type CollectorSummary = { success: boolean; output: string };

function formatArea(value: number | null): string {
  return value === null ? "—" : `${value.toLocaleString("pt-BR")} m²`;
}

/// Fase 10, item 8, Sessão 43 — análise de FII, irmã de
/// `StockAnalysisSection.tsx` na tela de Pesquisa unificada (mesmo `Card`/
/// busca, seletor de classe em `StockLookupPanel.tsx` decide qual das duas
/// renderiza). Indicadores próprios de FII (vacância/inadimplência por
/// imóvel, patrimônio líquido, VP da cota) direto da CVM — nada de
/// LPA/VPA/ROE, que não fazem sentido pra um fundo. CNPJ é resolvido
/// automaticamente (cache em `fii_cnpj_cache`, ver `main.py`) sem exigir
/// nenhuma ação do usuário — diferente do cadastro de Ativo
/// (`AssetSection.tsx`), aqui não há nada pra persistir/confirmar, é só
/// pesquisa.
function FiiLookupSection({ ticker }: { ticker: string }) {
  const [noteDraft, setNoteDraft] = useState("");
  const autoFetchedTickerRef = useRef<string | null>(null);
  const autoFetchedCvmCnpjRef = useRef<string | null>(null);

  const queryClient = useQueryClient();

  const quoteQuery = useQuery<StockQuote | null, AppError>({
    queryKey: ["fii-lookup-quote", ticker],
    queryFn: async () => {
      const quotes = await invoke<StockQuote[]>("list_stock_quotes");
      return latestForTicker(quotes, ticker);
    },
  });

  const collectorMutation = useMutation<CollectorSummary, AppError, string>({
    mutationFn: (t) =>
      invoke<CollectorSummary>("run_stock_collector", { ticker: t, assetClass: null }),
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["fii-lookup-quote", ticker] });
      queryClient.invalidateQueries({ queryKey: ["fii-lookup-price-history", ticker] });
    },
  });

  const priceHistoryQuery = useQuery<StockPriceHistory[], AppError>({
    queryKey: ["fii-lookup-price-history", ticker],
    queryFn: () => invoke("list_stock_price_history", { ticker }),
  });

  // Mesmo padrão cache-aware do resto do app: cotação ausente dispara o
  // coletor Yahoo no máximo uma vez por ticker buscado.
  useEffect(() => {
    if (
      quoteQuery.isSuccess &&
      quoteQuery.data === null &&
      autoFetchedTickerRef.current !== ticker
    ) {
      autoFetchedTickerRef.current = ticker;
      collectorMutation.mutate(ticker);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ticker, quoteQuery.isSuccess, quoteQuery.data]);

  const dividendsQuery = useQuery<StockDividendPayment[], AppError>({
    queryKey: ["fii-lookup-dividends", ticker],
    queryFn: async () => {
      const payments = await invoke<StockDividendPayment[]>("list_stock_dividend_payments");
      return payments.filter((p) => p.ticker === ticker);
    },
  });

  // Mesma tabela/comando que a ação usa pro tile "Avg dividend (5y)" — só
  // Yahoo, genérico por ticker, funciona igual pra FII.
  const dividendsAvgQuery = useQuery<StockDividendsAvg | null, AppError>({
    queryKey: ["fii-lookup-dividends-avg", ticker],
    queryFn: async () => {
      const rows = await invoke<StockDividendsAvg[]>("list_stock_dividends_avg");
      return latestForTicker(rows, ticker);
    },
  });

  // CNPJ resolvido sozinho (cache-aware do lado do coletor — ver
  // `main.py::main_fii_resolve_cnpj` — então pesquisar o mesmo ticker de
  // novo nunca chama a bolsai outra vez).
  const cnpjQuery = useQuery<FiiCnpjSuggestion | null, AppError>({
    queryKey: ["fii-lookup-cnpj", ticker],
    queryFn: () => invoke("resolve_fii_cnpj", { ticker }),
  });
  const cnpj = cnpjQuery.data?.cnpj ?? null;

  const monthlyQuery = useQuery<FiiCvmMonthly[], AppError>({
    queryKey: ["fii-cvm-monthly", cnpj],
    queryFn: () => invoke("list_fii_cvm_monthly", { cnpj }),
    enabled: cnpj !== null,
  });

  const propertiesQuery = useQuery<FiiCvmProperty[], AppError>({
    queryKey: ["fii-cvm-properties", cnpj],
    queryFn: () => invoke("list_fii_cvm_properties", { cnpj }),
    enabled: cnpj !== null,
  });

  const cvmCollectorMutation = useMutation<CollectorSummary, AppError, string>({
    mutationFn: (c) => invoke("run_fii_cvm_collector", { cnpjs: [c] }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["fii-cvm-monthly", cnpj] });
      queryClient.invalidateQueries({ queryKey: ["fii-cvm-properties", cnpj] });
    },
  });

  // Mesmo padrão cache-aware da cotação acima: assim que o CNPJ resolve,
  // se ainda não tem indicador mensal salvo, busca sozinho — sem isso, DY
  // do mês/rentabilidade/P-VP ficavam sempre em branco até o usuário achar
  // e clicar "Atualizar dados CVM" manualmente (achado testando com o
  // dono do projeto, Sessão 46). Guardado por CNPJ (não por ticker), já
  // que é isso que identifica o dado — um novo `run_fii_cvm_collector`
  // não duplica nada (`INSERT OR IGNORE` do lado do coletor).
  useEffect(() => {
    if (
      cnpj !== null &&
      monthlyQuery.isSuccess &&
      monthlyQuery.data.length === 0 &&
      autoFetchedCvmCnpjRef.current !== cnpj
    ) {
      autoFetchedCvmCnpjRef.current = cnpj;
      cvmCollectorMutation.mutate(cnpj);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cnpj, monthlyQuery.isSuccess, monthlyQuery.data]);

  const notesQuery = useQuery<StockNote[], AppError>({
    queryKey: ["fii-lookup-notes", ticker],
    queryFn: () => invoke("list_stock_notes"),
  });
  const note = (notesQuery.data ?? []).find((n) => n.ticker === ticker) ?? null;

  useEffect(() => {
    setNoteDraft(note?.note ?? "");
  }, [note]);

  const saveNoteMutation = useMutation<StockNote, AppError, void>({
    mutationFn: () => invoke<StockNote>("save_stock_note", { request: { ticker, note: noteDraft } }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["fii-lookup-notes", ticker] });
    },
  });

  const monthly = monthlyQuery.data ?? [];
  const latestMonthly = monthly[monthly.length - 1] ?? null;
  const properties = propertiesQuery.data ?? [];

  const price = quoteQuery.data?.price ?? null;
  const pvp =
    price != null && latestMonthly?.valor_patrimonial_cota
      ? price / latestMonthly.valor_patrimonial_cota
      : null;

  return (
    <div className="flex flex-col gap-6">
      {quoteQuery.isError && <p className="text-red-600">{quoteQuery.error.message}</p>}
      {collectorMutation.isError && (
        <p className="text-red-600">{collectorMutation.error.message}</p>
      )}
      {cnpjQuery.isError && <p className="text-red-600">{cnpjQuery.error.message}</p>}
      {(monthlyQuery.isError || propertiesQuery.isError) && (
        <p className="text-red-600">
          {monthlyQuery.error?.message ?? propertiesQuery.error?.message}
        </p>
      )}

      {quoteQuery.isLoading && <p className="text-muted-foreground">Loading {ticker}...</p>}

      {quoteQuery.data === null && collectorMutation.isPending && (
        <p className="text-muted-foreground">Fetching {ticker} for the first time...</p>
      )}

      {quoteQuery.data === null && !collectorMutation.isPending && !quoteQuery.isLoading && (
        <p className="text-muted-foreground">No data found for {ticker}.</p>
      )}

      {quoteQuery.data !== null && quoteQuery.data !== undefined && (
        <>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <CompanyLogo ticker={ticker} />
              <h2 className="text-xl font-semibold">{ticker}</h2>
            </div>
            <div className="flex items-center gap-3">
              <AddToAssetsButton
                ticker={ticker}
                assetClass="fii"
                name={quoteQuery.data?.name ?? ticker}
                currency={quoteQuery.data?.currency ?? "BRL"}
                exchange={quoteQuery.data?.exchange ?? null}
                cnpj={cnpj}
              />
              <Button
                type="button"
                variant="outline"
                onClick={() => collectorMutation.mutate(ticker)}
                disabled={collectorMutation.isPending}
              >
                {collectorMutation.isPending ? "Refreshing..." : "Refresh data"}
              </Button>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <StatTile label="Price" value={formatCurrency(price)} />
            <StatTile label="P/VP" value={formatRatio(pvp)} />
            <StatTile
              label="Dividend yield (month)"
              value={formatFractionAsPercent(latestMonthly?.dividend_yield_mes ?? null)}
            />
            <StatTile
              label="Effective return (month)"
              value={formatFractionAsPercent(latestMonthly?.rentabilidade_efetiva_mes ?? null)}
            />
            <StatTile
              label="Avg dividend/share (5y)"
              value={formatCurrency(dividendsAvgQuery.data?.avg_dividend_5y)}
            />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Price history</h3>
            <PriceHistoryChart
              history={priceHistoryQuery.data ?? []}
              currencyPrefix={quoteQuery.data?.currency === "USD" ? "US$" : "R$"}
            />
          </div>

          <div>
            <div className="mb-3 flex items-center justify-between">
              <h3 className="text-sm font-semibold text-muted-foreground">
                CVM indicators {latestMonthly && `(reference: ${latestMonthly.reference_date})`}
              </h3>
              {cnpj && (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={cvmCollectorMutation.isPending}
                  onClick={() => cvmCollectorMutation.mutate(cnpj)}
                >
                  {cvmCollectorMutation.isPending ? "Updating..." : "Update CVM data"}
                </Button>
              )}
            </div>

            {cnpjQuery.isLoading && <p className="text-muted-foreground">Resolving CNPJ...</p>}
            {cnpjQuery.isSuccess && cnpj === null && (
              <p className="text-muted-foreground">
                Couldn't find this fund's CNPJ automatically — without a CNPJ there's no way to
                pull CVM indicators. Register the asset in the Portfolio tab and paste the CNPJ
                manually there to unlock this section.
              </p>
            )}
            {cvmCollectorMutation.isError && (
              <p className="text-red-600">{cvmCollectorMutation.error.message}</p>
            )}
            {cnpj && !latestMonthly && cvmCollectorMutation.isPending && (
              <p className="text-muted-foreground">Fetching CVM data for the first time...</p>
            )}
            {cnpj &&
              !latestMonthly &&
              !monthlyQuery.isLoading &&
              !cvmCollectorMutation.isPending && (
                <p className="text-muted-foreground">
                  No CVM data yet — click "Update CVM data" above.
                </p>
              )}

            {latestMonthly && (
              <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
                <StatTile
                  label="Net assets"
                  value={formatCurrency(latestMonthly.patrimonio_liquido)}
                />
                <StatTile
                  label="Book value/share"
                  value={formatCurrency(latestMonthly.valor_patrimonial_cota)}
                />
                <StatTile label="Shareholders" value={String(latestMonthly.numero_cotistas ?? "—")} />
              </div>
            )}
          </div>

          {properties.length > 0 && (
            <div>
              <h3 className="mb-3 text-sm font-semibold text-muted-foreground">
                Properties (reference: {properties[0].reference_date})
              </h3>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Property</TableHead>
                    <TableHead>Address</TableHead>
                    <TableHead>Area</TableHead>
                    <TableHead>Vacancy</TableHead>
                    <TableHead>Delinquency</TableHead>
                    <TableHead>% Revenue</TableHead>
                    <TableHead>% Leased</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {properties.map((property) => (
                    <TableRow key={property.id}>
                      <TableCell>{property.nome_imovel}</TableCell>
                      <TableCell>{property.endereco ?? "—"}</TableCell>
                      <TableCell>{formatArea(property.area_m2)}</TableCell>
                      <TableCell>{formatFractionAsPercent(property.percentual_vacancia)}</TableCell>
                      <TableCell>
                        {formatFractionAsPercent(property.percentual_inadimplencia)}
                      </TableCell>
                      <TableCell>
                        {formatFractionAsPercent(property.percentual_receitas_fii)}
                      </TableCell>
                      <TableCell>{formatFractionAsPercent(property.percentual_locado)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">
              Dividend history
            </h3>
            <DividendHistoryChart payments={dividendsQuery.data ?? []} />
          </div>

          <div>
            <h3 className="mb-3 text-sm font-semibold text-muted-foreground">Notes</h3>
            <Textarea
              value={noteDraft}
              onChange={(e) => setNoteDraft(e.currentTarget.value)}
              rows={4}
            />
            <Button
              type="button"
              className="mt-2"
              onClick={() => saveNoteMutation.mutate()}
              disabled={saveNoteMutation.isPending}
            >
              {saveNoteMutation.isPending ? "Saving..." : "Save notes"}
            </Button>
            {saveNoteMutation.isError && (
              <p className="mt-2 text-red-600">{saveNoteMutation.error.message}</p>
            )}
          </div>
        </>
      )}
    </div>
  );
}

export default FiiLookupSection;
