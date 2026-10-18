import { useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Cell, Legend, Pie, PieChart, ResponsiveContainer, Tooltip } from "recharts";
import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import { SortableContext, arrayMove, rectSortingStrategy, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Eye, EyeOff, GripVertical, LayoutGrid } from "lucide-react";
import type { AppError } from "../types";
import type { Asset, AssetFavorite } from "../portfolio/types";
import { ASSET_CLASS_LABELS, type AssetClass } from "../portfolio/types";
import { BUCKET_LABELS, type LossBalance } from "../portfolio/taxLosses";
import type { StockQuote } from "../collector/types";
import { latestForTicker } from "../collector/latestForTicker";
import type { WorkspaceSummary, MonthlyReturn, HomeWidgetLayout, WidgetKey } from "./types";
import FxStalenessNotice from "../finances/FxStalenessNotice";
import KeyRatesTable from "../rates/KeyRatesTable";
import { formatDate as formatRateDate, formatRate, useInterestRates } from "../rates/useInterestRates";
import { Button } from "@/components/ui/button";
import { formatMoney } from "@/lib/utils";
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

const GRID_COLS = 12;
const ROW_HEIGHT = 32;
const GRID_GAP = 16;

// Mesma paleta/convenção de `portfolio/SummarySection.tsx` — duplicada
// localmente em vez de extraída pra um módulo compartilhado, seguindo a
// convenção já estabelecida no projeto (ver `finances/LiabilitySection.tsx`,
// Sessão 82).
const CLASS_COLORS: Partial<Record<AssetClass, string>> = {
  acao_br: "#3987e5",
  fii: "#d95926",
  etf_br: "#199e70",
  cripto: "#c98500",
  bdr: "#d55181",
  metal: "#008300",
  acao_internacional: "#9085e9",
  reit: "#e66767",
};
const OTHER_COLOR = "#9fb1c2"; // --muted-foreground

const WIDGET_TITLES: Record<WidgetKey, string> = {
  kpi_total_value: "Total portfolio value",
  kpi_profit: "Profit",
  kpi_dividends_12m: "Dividends received (12M)",
  kpi_return: "Return",
  allocation_donut: "Allocation by class",
  portfolios_table: "Portfolios",
  market_panel: "★ Market panel",
  interest_rates: "Interest rates",
  losses_to_offset: "Losses to offset",
};

// Fase 10.8 fatia 2 — mesma disposição visual da fatia 1 (Sessão 84), agora
// como ponto de partida editável em vez de fixa: 4 KPIs numa faixa,
// donut+portfolios lado a lado, market panel embaixo ocupando a largura
// toda. Usado quando `get_home_layout` volta vazio (workspace nunca
// personalizou) e pra completar widgets que a personalização salva ainda
// não conhece (ex. um widget novo de uma fatia futura).
const DEFAULT_LAYOUT: HomeWidgetLayout[] = [
  { widget_key: "kpi_total_value", position: 0, w: 3, h: 4, visible: true },
  { widget_key: "kpi_profit", position: 1, w: 3, h: 4, visible: true },
  { widget_key: "kpi_dividends_12m", position: 2, w: 3, h: 4, visible: true },
  { widget_key: "kpi_return", position: 3, w: 3, h: 4, visible: true },
  { widget_key: "allocation_donut", position: 4, w: 6, h: 9, visible: true },
  { widget_key: "portfolios_table", position: 5, w: 6, h: 9, visible: true },
  { widget_key: "market_panel", position: 6, w: 12, h: 8, visible: true },
  // Fase 17 — entra pra quem já personalizou via `mergeWithDefaults`.
  { widget_key: "interest_rates", position: 7, w: 6, h: 10, visible: true },
  // Fase 24 — saldo de prejuízo a compensar por grupo; entra pra quem já personalizou via `mergeWithDefaults`.
  { widget_key: "losses_to_offset", position: 8, w: 6, h: 8, visible: true },
];

function mergeWithDefaults(saved: HomeWidgetLayout[]): HomeWidgetLayout[] {
  if (saved.length === 0) return DEFAULT_LAYOUT.map((w) => ({ ...w }));
  const known = new Set(saved.map((w) => w.widget_key));
  const missing = DEFAULT_LAYOUT.filter((w) => !known.has(w.widget_key));
  if (missing.length === 0) return saved;
  const maxPosition = saved.reduce((max, w) => Math.max(max, w.position), -1);
  return [...saved, ...missing.map((w, i) => ({ ...w, position: maxPosition + 1 + i }))];
}

function formatCurrency(value: number): string {
  return value.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function formatPct(value: number): string {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

function plColorClass(value: number | null): string {
  if (value == null || value === 0) return "text-muted-foreground";
  return value > 0 ? "text-primary" : "text-destructive";
}

function KpiBody({
  value,
  valueClassName,
  subtext,
}: {
  value: string;
  valueClassName?: string;
  subtext?: string;
}) {
  return (
    <div>
      <p className={`text-2xl font-semibold ${valueClassName ?? ""}`}>{value}</p>
      {subtext && <p className="mt-1 text-xs text-muted-foreground">{subtext}</p>}
    </div>
  );
}

// Mesmo rebaseamento de `SummarySection.tsx::trailingTwelveMonthPct` — a
// série de `get_workspace_profitability` já vem encadeada
// (`r_cumulative_pct`, `domain::twr::chain`), então isolar "últimos 12
// meses" é só dividir o índice do mês mais recente pelo de 12 meses atrás.
function trailingTwelveMonthPct(monthly: MonthlyReturn[]): number | null {
  if (monthly.length === 0) return null;
  if (monthly.length <= 12) return monthly[monthly.length - 1].r_cumulative_pct;
  const end = monthly[monthly.length - 1];
  const baseline = monthly[monthly.length - 13];
  const endIndex = 1 + end.r_cumulative_pct / 100;
  const baseIndex = 1 + baseline.r_cumulative_pct / 100;
  return (endIndex / baseIndex - 1) * 100;
}

function AllocationTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { payload: { asset_class: string; market_value: number } }[];
}) {
  if (!active || !payload?.length) return null;
  const row = payload[0].payload;
  return (
    <div
      className="rounded-lg border border-border px-3 py-2 text-sm"
      style={{ background: "#111820" }}
    >
      <p className="font-medium">{ASSET_CLASS_LABELS[row.asset_class as AssetClass] ?? row.asset_class}</p>
      <p className="text-muted-foreground">{formatCurrency(row.market_value)}</p>
    </div>
  );
}

function WidgetShell({
  title,
  editing,
  onHide,
  dragHandleProps,
  children,
}: {
  title: string;
  editing: boolean;
  onHide: () => void;
  dragHandleProps?: Record<string, unknown>;
  children: ReactNode;
}) {
  return (
    <Card className="flex h-full flex-col overflow-hidden">
      <CardHeader className="flex shrink-0 flex-row items-center justify-between gap-2 space-y-0">
        <div className="flex min-w-0 items-center gap-1.5">
          {editing && (
            <button
              type="button"
              className="touch-none cursor-grab text-muted-foreground active:cursor-grabbing"
              aria-label="Drag to reorder"
              {...dragHandleProps}
            >
              <GripVertical className="size-3.5" />
            </button>
          )}
          <CardTitle className="truncate">{title}</CardTitle>
        </div>
        {editing && (
          <Button variant="ghost" size="icon-xs" className="shrink-0" onClick={onHide} title="Hide widget">
            <EyeOff className="size-3.5" />
          </Button>
        )}
      </CardHeader>
      <CardContent className="flex-1 overflow-auto">{children}</CardContent>
    </Card>
  );
}

// Corner de resize customizado (sem lib) — arrasta em unidades de grid
// calculando a largura de coluna a partir do próprio retângulo renderizado
// do widget (que já ocupa exatamente `w` colunas), evitando depender de uma
// ref pro container inteiro.
function ResizeHandle({
  w,
  onResize,
}: {
  w: number;
  onResize: (deltaCols: number, deltaRows: number) => void;
}) {
  const handleRef = useRef<HTMLDivElement>(null);

  function onPointerDown(e: React.PointerEvent<HTMLDivElement>) {
    e.preventDefault();
    e.stopPropagation();
    const wrapper = handleRef.current?.parentElement;
    if (!wrapper) return;
    const rect = wrapper.getBoundingClientRect();
    const colPx = (rect.width - (w - 1) * GRID_GAP) / w;
    const startX = e.clientX;
    const startY = e.clientY;
    let lastDeltaCols = 0;
    let lastDeltaRows = 0;

    function onMove(ev: PointerEvent) {
      const deltaCols = Math.round((ev.clientX - startX) / (colPx + GRID_GAP));
      const deltaRows = Math.round((ev.clientY - startY) / (ROW_HEIGHT + GRID_GAP));
      if (deltaCols === lastDeltaCols && deltaRows === lastDeltaRows) return;
      onResize(deltaCols - lastDeltaCols, deltaRows - lastDeltaRows);
      lastDeltaCols = deltaCols;
      lastDeltaRows = deltaRows;
    }
    function onUp() {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    }
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  return (
    <div
      ref={handleRef}
      onPointerDown={onPointerDown}
      title="Resize"
      className="absolute bottom-0.5 right-0.5 size-3.5 cursor-nwse-resize touch-none opacity-50 hover:opacity-100"
      style={{ clipPath: "polygon(100% 0%, 0% 100%, 100% 100%)", background: "var(--muted-foreground)" }}
    />
  );
}

function SortableWidget({
  widget,
  title,
  editing,
  onHide,
  onResize,
  children,
}: {
  widget: HomeWidgetLayout;
  title: string;
  editing: boolean;
  onHide: () => void;
  onResize: (deltaCols: number, deltaRows: number) => void;
  children: ReactNode;
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: widget.widget_key,
    disabled: !editing,
  });

  const style: CSSProperties = {
    gridColumn: `span ${widget.w}`,
    gridRow: `span ${widget.h}`,
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.4 : 1,
    position: "relative",
  };

  return (
    <div ref={setNodeRef} style={style}>
      <WidgetShell
        title={title}
        editing={editing}
        onHide={onHide}
        dragHandleProps={editing ? { ...attributes, ...listeners } : undefined}
      >
        {children}
      </WidgetShell>
      {editing && <ResizeHandle w={widget.w} onResize={onResize} />}
    </div>
  );
}

// Home do Workspace, fatia 2 (personalização de layout, ver PHASE.md item
// 10.8) — agrega todas as carteiras do Workspace, separada do "Resumo" de
// cada Carteira (`portfolio/SummarySection.tsx`, que continua existindo do
// jeito que está). Painel de mercado reaproveita o mecanismo de Favoritos
// (Fase 10.4) como está — sem coluna de ações aqui, gerenciar continua em
// Watchlists. Layout (posição/tamanho/visibilidade de cada widget) agora é
// editável via "Customize layout" e persiste em `home_widget_layout`,
// escopado por workspace (`commands::home_layout`).
function HomeSection({
  workspaceId,
  onOpenRates,
}: {
  workspaceId: number;
  onOpenRates: () => void;
}) {
  const queryClient = useQueryClient();
  const ratesQuery = useInterestRates();
  const summaryQuery = useQuery<WorkspaceSummary, AppError>({
    queryKey: ["workspace-summary", workspaceId],
    queryFn: () => invoke("get_workspace_summary", { workspaceId }),
  });
  const profitabilityQuery = useQuery<MonthlyReturn[], AppError>({
    queryKey: ["workspace-profitability", workspaceId],
    queryFn: () => invoke("get_workspace_profitability", { workspaceId }),
  });
  const assetsQuery = useQuery<Asset[], AppError>({
    queryKey: ["assets"],
    queryFn: () => invoke("list_assets"),
  });
  const quotesQuery = useQuery<StockQuote[], AppError>({
    queryKey: ["stock-quotes"],
    queryFn: () => invoke("list_stock_quotes"),
  });
  const favoritesQuery = useQuery<AssetFavorite[], AppError>({
    queryKey: ["asset-favorites", workspaceId],
    queryFn: () => invoke("list_favorite_assets", { workspaceId }),
  });
  const layoutQuery = useQuery<HomeWidgetLayout[], AppError>({
    queryKey: ["home-layout", workspaceId],
    queryFn: () => invoke("get_home_layout", { workspaceId }),
  });

  const [editing, setEditing] = useState(false);
  const [draftLayout, setDraftLayout] = useState<HomeWidgetLayout[] | null>(null);

  const saveMutation = useMutation({
    mutationFn: (widgets: HomeWidgetLayout[]) =>
      invoke("save_home_layout", {
        workspaceId,
        widgets: widgets.map((w) => ({
          widget_key: w.widget_key,
          position: w.position,
          w: w.w,
          h: w.h,
          visible: w.visible,
        })),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["home-layout", workspaceId] });
      setEditing(false);
      setDraftLayout(null);
    },
  });

  const savedLayout = useMemo(
    () => mergeWithDefaults((layoutQuery.data ?? []).map((w) => ({ ...w, widget_key: w.widget_key as WidgetKey }))),
    [layoutQuery.data],
  );

  // O saldo de prejuízo vem da mesma apuração da aba Taxes (mesma chave de cache), e só roda com o widget visível.
  const lossesVisible = (draftLayout ?? savedLayout).find((w) => w.widget_key === "losses_to_offset")?.visible ?? true;
  const lossesQuery = useQuery<{ assessment: { loss_balances: LossBalance[] } }, AppError>({
    queryKey: ["tax-assessment", workspaceId],
    queryFn: () => invoke("get_tax_assessment", { workspaceId }),
    enabled: lossesVisible,
  });
  const lossBalances = lossesQuery.data?.assessment.loss_balances ?? [];

  const summary = summaryQuery.data;
  const monthly = profitabilityQuery.data ?? [];
  const assets = assetsQuery.data ?? [];
  const quotes = quotesQuery.data ?? [];
  const favorites = favoritesQuery.data ?? [];

  const totalReturnPct = monthly.length > 0 ? monthly[monthly.length - 1].r_cumulative_pct : null;
  const twelveMonthReturnPct = useMemo(() => trailingTwelveMonthPct(monthly), [monthly]);

  function assetFor(assetId: number): Asset | undefined {
    return assets.find((a) => a.id === assetId);
  }

  function currentPriceFor(assetId: number): number | null {
    const asset = assetFor(assetId);
    if (!asset) return null;
    return latestForTicker(quotes, asset.ticker)?.price ?? null;
  }

  const WIDGET_RENDERERS: Record<WidgetKey, () => ReactNode> = {
    kpi_total_value: () => (
      <KpiBody
        value={summary ? formatMoney(summary.total_market_value, summary.base_currency) : "—"}
        subtext={
          summary ? `Invested: ${formatMoney(summary.total_cost_basis, summary.base_currency)}` : undefined
        }
      />
    ),
    kpi_profit: () => (
      <KpiBody
        value={summary ? formatMoney(summary.unrealized_pl, summary.base_currency) : "—"}
        valueClassName={summary ? plColorClass(summary.unrealized_pl) : undefined}
        subtext={
          summary?.unrealized_pl_pct != null ? formatPct(summary.unrealized_pl_pct * 100) : undefined
        }
      />
    ),
    kpi_dividends_12m: () => (
      <KpiBody value={summary ? formatMoney(summary.dividends_received_12m, summary.base_currency) : "—"} />
    ),
    kpi_return: () => (
      <KpiBody
        value={twelveMonthReturnPct != null ? formatPct(twelveMonthReturnPct) : "—"}
        valueClassName={plColorClass(twelveMonthReturnPct)}
        subtext={totalReturnPct != null ? `Total: ${formatPct(totalReturnPct)}` : "No return history yet"}
      />
    ),
    allocation_donut: () =>
      !summary || summary.allocation_by_class.length === 0 ? (
        <p className="text-muted-foreground">No priced positions yet in this Workspace.</p>
      ) : (
        <ResponsiveContainer width="100%" height="100%" minHeight={220}>
          <PieChart>
            <Pie
              data={summary.allocation_by_class}
              dataKey="market_value"
              nameKey="asset_class"
              innerRadius={64}
              outerRadius={104}
              paddingAngle={2}
              label={(props: { percent?: number }) => `${((props.percent ?? 0) * 100).toFixed(0)}%`}
            >
              {summary.allocation_by_class.map((entry) => (
                <Cell
                  key={entry.asset_class}
                  fill={CLASS_COLORS[entry.asset_class as AssetClass] ?? OTHER_COLOR}
                  stroke="#111820"
                  strokeWidth={2}
                />
              ))}
            </Pie>
            <Tooltip content={<AllocationTooltip />} />
            <Legend
              formatter={(_value, entry) => {
                const payload = (entry as { payload?: { asset_class: string } }).payload;
                const assetClass = payload?.asset_class ?? "";
                return ASSET_CLASS_LABELS[assetClass as AssetClass] ?? assetClass;
              }}
            />
          </PieChart>
        </ResponsiveContainer>
      ),
    portfolios_table: () => (
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Portfolio</TableHead>
            <TableHead>Market value</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {(!summary || summary.portfolios.length === 0) && (
            <TableRow>
              <TableCell colSpan={2} className="text-center text-muted-foreground">
                No portfolios in this Workspace yet.
              </TableCell>
            </TableRow>
          )}
          {summary?.portfolios.map((p) => (
            <TableRow key={p.portfolio_id}>
              <TableCell>{p.name}</TableCell>
              <TableCell>{formatCurrency(p.market_value)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    ),
    market_panel: () => (
      <>
        <p className="mb-4 text-sm text-muted-foreground">
          Favorited assets (manage the star on the "Assets"/"Watchlists" tabs) — useful to keep an
          eye on something without owning it, e.g. a market index.
        </p>
        {favoritesQuery.isError && <p className="mb-3 text-red-600">{favoritesQuery.error.message}</p>}
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Ticker</TableHead>
              <TableHead>Name</TableHead>
              <TableHead>Current price</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {favorites.length === 0 && (
              <TableRow>
                <TableCell colSpan={3} className="text-center text-muted-foreground">
                  No favorited assets yet.
                </TableCell>
              </TableRow>
            )}
            {favorites.map((favorite) => {
              const asset = assetFor(favorite.asset_id);
              const price = currentPriceFor(favorite.asset_id);
              return (
                <TableRow key={favorite.id}>
                  <TableCell>{asset?.ticker ?? `#${favorite.asset_id}`}</TableCell>
                  <TableCell>{asset?.name ?? "—"}</TableCell>
                  <TableCell>{price !== null ? price.toFixed(2) : "—"}</TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </>
    ),
    interest_rates: () => (
      <div className="flex flex-col gap-3">
        {ratesQuery.isLoading && <p className="text-sm">Loading…</p>}
        {ratesQuery.isError && (
          <p className="text-sm text-red-600">{ratesQuery.error.message}</p>
        )}
        {ratesQuery.data && (
          <>
            <div>
              <p className="text-sm text-muted-foreground">Selic target</p>
              {ratesQuery.data.selic ? (
                <>
                  <p className="text-2xl font-semibold">
                    {formatRate(ratesQuery.data.selic.current_pct)}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    since {formatRateDate(ratesQuery.data.selic.since)}
                  </p>
                </>
              ) : (
                <p className="text-sm text-muted-foreground">
                  Unavailable right now — the Central Bank's service didn't answer.
                </p>
              )}
            </div>
            {ratesQuery.data.di ? (
              <div>
                <p className="mb-1 text-sm text-muted-foreground">DI futures</p>
                <KeyRatesTable di={ratesQuery.data.di} />
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">
                {ratesQuery.data.di_error ? "DI curve unavailable right now." : "No recent DI curve."}
              </p>
            )}
          </>
        )}
        <Button variant="outline" size="sm" className="w-fit" onClick={onOpenRates}>
          Details
        </Button>
      </div>
    ),
    losses_to_offset: () => (
      <div className="flex flex-col gap-3" data-testid="losses-widget">
        {lossesQuery.isLoading && <p className="text-sm">Loading…</p>}
        {lossesQuery.isError && <p className="text-sm text-red-600">{lossesQuery.error.message}</p>}
        {lossesQuery.data && lossBalances.length === 0 && (
          <p className="text-sm text-muted-foreground">No losses to offset.</p>
        )}
        {lossBalances.map((l) => (
          <div key={l.bucket}>
            <p className="text-sm text-muted-foreground">{BUCKET_LABELS[l.bucket]}</p>
            <p className="text-2xl font-semibold" data-testid={`losses-widget-${l.bucket}`}>
              {formatMoney(l.balance, "BRL")}
            </p>
            <p className="text-xs text-muted-foreground">
              Saves up to {formatMoney(l.tax_saved, "BRL")} at {(l.rate * 100).toFixed(0)}%
            </p>
          </div>
        ))}
        {lossBalances.length > 1 && (
          <p className="text-sm text-muted-foreground" data-testid="losses-widget-total">
            Total potential savings: {formatMoney(lossBalances.reduce((sum, l) => sum + l.tax_saved, 0), "BRL")}
          </p>
        )}
        {lossBalances.length > 0 && (
          <p className="text-xs text-muted-foreground">Details in Portfolio → Taxes.</p>
        )}
      </div>
    ),
  };

  function startEditing() {
    setDraftLayout(savedLayout.map((w) => ({ ...w })));
    setEditing(true);
  }

  function cancelEditing() {
    setEditing(false);
    setDraftLayout(null);
  }

  function hideWidget(key: WidgetKey) {
    setDraftLayout((prev) =>
      (prev ?? []).map((w) => (w.widget_key === key ? { ...w, visible: false } : w)),
    );
  }

  function showWidget(key: WidgetKey) {
    setDraftLayout((prev) => {
      const list = prev ?? [];
      const maxPosition = list.filter((w) => w.visible).reduce((max, w) => Math.max(max, w.position), -1);
      return list.map((w) => (w.widget_key === key ? { ...w, visible: true, position: maxPosition + 1 } : w));
    });
  }

  function resizeWidget(key: WidgetKey, deltaCols: number, deltaRows: number) {
    setDraftLayout((prev) =>
      (prev ?? []).map((w) =>
        w.widget_key === key
          ? {
              ...w,
              w: Math.min(GRID_COLS, Math.max(2, w.w + deltaCols)),
              h: Math.max(2, w.h + deltaRows),
            }
          : w,
      ),
    );
  }

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    setDraftLayout((prev) => {
      if (!prev) return prev;
      const visible = prev.filter((w) => w.visible).sort((a, b) => a.position - b.position);
      const oldIndex = visible.findIndex((w) => w.widget_key === active.id);
      const newIndex = visible.findIndex((w) => w.widget_key === over.id);
      if (oldIndex === -1 || newIndex === -1) return prev;
      const reordered = arrayMove(visible, oldIndex, newIndex);
      const positionByKey = new Map(reordered.map((w, i) => [w.widget_key, i]));
      return prev.map((w) =>
        positionByKey.has(w.widget_key) ? { ...w, position: positionByKey.get(w.widget_key)! } : w,
      );
    });
  }

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  if (summaryQuery.isError) {
    return <p className="text-red-600">{summaryQuery.error.message}</p>;
  }
  if (summaryQuery.isLoading || !summary) {
    return <p className="text-muted-foreground">Loading...</p>;
  }

  const effectiveLayout = editing ? (draftLayout ?? savedLayout) : savedLayout;
  const visibleWidgets = effectiveLayout.filter((w) => w.visible).sort((a, b) => a.position - b.position);
  const hiddenWidgets = editing ? effectiveLayout.filter((w) => !w.visible) : [];

  return (
    <div className="flex flex-col gap-4">
      <FxStalenessNotice workspaceId={workspaceId} />
      <div className="flex items-center justify-between gap-2">
        <div />
        {editing ? (
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={cancelEditing} disabled={saveMutation.isPending}>
              Cancel
            </Button>
            <Button
              size="sm"
              onClick={() => draftLayout && saveMutation.mutate(draftLayout)}
              disabled={saveMutation.isPending}
            >
              Save layout
            </Button>
          </div>
        ) : (
          <Button variant="outline" size="sm" onClick={startEditing}>
            <LayoutGrid className="size-3.5" />
            Customize layout
          </Button>
        )}
      </div>

      {saveMutation.isError && (
        <p className="text-red-600">{saveMutation.error.message}</p>
      )}

      {editing && hiddenWidgets.length > 0 && (
        <div className="flex flex-wrap items-center gap-2 rounded-lg border border-dashed border-border p-3">
          <span className="text-sm text-muted-foreground">Hidden widgets:</span>
          {hiddenWidgets.map((w) => (
            <Button
              key={w.widget_key}
              variant="secondary"
              size="sm"
              onClick={() => showWidget(w.widget_key)}
            >
              <Eye className="size-3.5" />
              {WIDGET_TITLES[w.widget_key]}
            </Button>
          ))}
        </div>
      )}

      {summary.positions_missing_fx > 0 && (
        <p className="text-sm text-muted-foreground" data-testid="home-missing-fx">
          {summary.positions_missing_fx} position{summary.positions_missing_fx === 1 ? "" : "s"} without an
          exchange rate to {summary.base_currency} — the totals above don't include {summary.positions_missing_fx === 1 ? "it" : "them"}.
        </p>
      )}

      {summary.positions_missing_price > 0 && (
        <p className="text-sm text-muted-foreground">
          {summary.positions_missing_price} position{summary.positions_missing_price === 1 ? "" : "s"} without a
          registered price/valuation — the totals above don't include {summary.positions_missing_price === 1 ? "it" : "them"}.
        </p>
      )}

      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={visibleWidgets.map((w) => w.widget_key)} strategy={rectSortingStrategy}>
          <div
            className="grid"
            style={{
              gridTemplateColumns: `repeat(${GRID_COLS}, minmax(0, 1fr))`,
              gridAutoRows: `${ROW_HEIGHT}px`,
              gap: GRID_GAP,
            }}
          >
            {visibleWidgets.map((w) => (
              <SortableWidget
                key={w.widget_key}
                widget={w}
                title={WIDGET_TITLES[w.widget_key]}
                editing={editing}
                onHide={() => hideWidget(w.widget_key)}
                onResize={(deltaCols, deltaRows) => resizeWidget(w.widget_key, deltaCols, deltaRows)}
              >
                {WIDGET_RENDERERS[w.widget_key]()}
              </SortableWidget>
            ))}
          </div>
        </SortableContext>
      </DndContext>
    </div>
  );
}

export default HomeSection;
