import { useState, type FormEvent, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../types";
import Field from "../components/Field";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

// Fase 18, 3ª fatia (Sessão 116) — posições em opções da B3, compradas ou
// lançadas, numa aba da Carteira. Isolada de patrimônio, Resumo, rentabilidade
// e IR: é um registro com resultado. O resultado realizado vem do backend; o a
// mercado só aparece depois de "Refresh market prices", porque a primeira carga
// do dia baixa ~80 MB da B3 (~2 min).

type OptionPosition = {
  id: number;
  portfolio_id: number;
  series_ticker: string;
  underlying_ticker: string;
  option_type: "call" | "put";
  strike: number;
  expiration: string;
  side: "long" | "short";
  quantity: number;
  premium: number;
  open_date: string;
  open_fee: number | null;
  notes: string | null;
  close_kind: "closed" | "expired" | "exercised" | null;
  close_price: number | null;
  close_date: string | null;
  close_fee: number | null;
  is_open: boolean;
  is_expired: boolean;
  days_to_expiry: number;
  realized_pnl: number | null;
};

type OptionMark = {
  position_id: number;
  series_ticker: string;
  last_price: number | null;
  last_trade_date: string | null;
  unrealized_pnl: number | null;
};

type OptionMarks = { stale: boolean; marks: OptionMark[] };

type CloseKind = "closed" | "expired" | "exercised";

const SIDE_LABEL: Record<OptionPosition["side"], string> = { long: "Bought", short: "Sold (written)" };
const CLOSE_LABEL: Record<CloseKind, string> = {
  closed: "Closed (bought back or sold)",
  expired: "Expired worthless",
  exercised: "Exercised",
};
const CLOSE_SHORT: Record<CloseKind, string> = {
  closed: "Closed",
  expired: "Expired",
  exercised: "Exercised",
};

const DEFAULT_QUANTITY = "100"; // 1 lote padrão da B3
const DEFAULT_DAYS_TO_EXPIRY = 30;

function dateIso(offsetDays = 0): string {
  const date = new Date();
  date.setDate(date.getDate() + offsetDays);
  return date.toISOString().slice(0, 10);
}

// Mesma formatação de `TaxesSection.tsx` (duplicada de propósito, convenção do projeto).
function formatBrl(value: number): string {
  return value.toLocaleString("pt-BR", { style: "currency", currency: "BRL" });
}

function formatDate(iso: string): string {
  return new Date(`${iso}T00:00:00`).toLocaleDateString("pt-BR");
}

/** Resultado com sinal explícito, pra o ganho e a perda não dependerem só da cor. */
function Pnl({ value }: { value: number | null | undefined }) {
  if (value == null) return <span className="text-muted-foreground">—</span>;
  const gain = value >= 0;
  return (
    <span className={gain ? "text-green-600 dark:text-green-400" : "text-red-600 dark:text-red-400"}>
      {gain ? "+" : "−"}
      {formatBrl(Math.abs(value))}
    </span>
  );
}

function positionLabel(p: OptionPosition): string {
  return `${p.side === "long" ? "Bought" : "Sold"} ${p.option_type}`;
}

function KpiCard({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <p className="text-sm text-muted-foreground">{title}</p>
      <p className="mt-1 text-2xl font-semibold">{children}</p>
    </div>
  );
}

function OptionPositionsSection({ portfolioId }: { portfolioId: number }) {
  const queryClient = useQueryClient();

  const [series, setSeries] = useState("");
  const [underlying, setUnderlying] = useState("");
  const [optionType, setOptionType] = useState<"call" | "put">("call");
  const [side, setSide] = useState<"long" | "short">("long");
  const [strike, setStrike] = useState("");
  const [expiration, setExpiration] = useState(dateIso(DEFAULT_DAYS_TO_EXPIRY));
  const [quantity, setQuantity] = useState(DEFAULT_QUANTITY);
  const [premium, setPremium] = useState("");
  const [openDate, setOpenDate] = useState(dateIso());
  const [fee, setFee] = useState("");
  const [notes, setNotes] = useState("");

  const [closing, setClosing] = useState<OptionPosition | null>(null);
  const [closeKind, setCloseKind] = useState<CloseKind>("closed");
  const [closePrice, setClosePrice] = useState("");
  const [closeDate, setCloseDate] = useState(dateIso());
  const [closeFee, setCloseFee] = useState("");
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<number | null>(null);
  const [marks, setMarks] = useState<OptionMarks | null>(null);

  const positionsQuery = useQuery<OptionPosition[], AppError>({
    queryKey: ["option-positions", portfolioId],
    queryFn: () => invoke("list_option_positions", { portfolioId }),
  });
  const positions = positionsQuery.data ?? [];
  const open = positions.filter((p) => p.is_open);
  const closed = positions.filter((p) => !p.is_open);

  function refresh() {
    queryClient.invalidateQueries({ queryKey: ["option-positions", portfolioId] });
  }

  const createMutation = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("create_option_position", {
        request: {
          portfolio_id: portfolioId,
          series_ticker: series,
          underlying_ticker: underlying,
          option_type: optionType,
          strike: Number(strike),
          expiration,
          side,
          quantity: Number(quantity),
          premium: Number(premium),
          open_date: openDate,
          open_fee: fee !== "" ? Number(fee) : null,
          notes: notes !== "" ? notes : null,
        },
      }),
    onSuccess: () => {
      refresh();
      setSeries("");
      setStrike("");
      setPremium("");
      setFee("");
      setNotes("");
    },
  });

  const closeMutation = useMutation<unknown, AppError, void>({
    mutationFn: () =>
      invoke("close_option_position", {
        request: {
          id: closing?.id,
          close_kind: closeKind,
          close_price: closeKind === "expired" ? null : Number(closePrice),
          close_date: closeDate,
          close_fee: closeFee !== "" ? Number(closeFee) : null,
        },
      }),
    onSuccess: () => {
      refresh();
      setClosing(null);
      setMarks(null); // as marcações eram de posições que mudaram
    },
  });

  const deleteMutation = useMutation<void, AppError, number>({
    mutationFn: (id) => invoke("delete_option_position", { id }),
    onSuccess: () => {
      refresh();
      setConfirmingDeleteId(null);
    },
  });

  const marksMutation = useMutation<OptionMarks, AppError, void>({
    mutationFn: () => invoke("get_option_marks", { portfolioId }),
    onSuccess: setMarks,
  });

  function handleCreate(event: FormEvent) {
    event.preventDefault();
    createMutation.mutate();
  }

  function startClosing(position: OptionPosition) {
    setClosing(position);
    setCloseKind("closed");
    setClosePrice("");
    setCloseDate(dateIso());
    setCloseFee("");
    closeMutation.reset();
  }

  const markFor = (id: number) => marks?.marks.find((m) => m.position_id === id);
  const marketResult = marks
    ? marks.marks.reduce((sum, m) => sum + (m.unrealized_pnl ?? 0), 0)
    : null;
  const hasAnyMark = marks?.marks.some((m) => m.unrealized_pnl != null) ?? false;
  const realizedTotal = closed.reduce((sum, p) => sum + (p.realized_pnl ?? 0), 0);

  return (
    <div className="flex flex-col gap-6" data-testid="option-positions">
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        <KpiCard title="Open positions">
          <span data-testid="op-open-count">{open.length}</span>
        </KpiCard>
        <KpiCard title="Market result (open)">
          {hasAnyMark && marketResult != null ? <Pnl value={marketResult} /> : "—"}
        </KpiCard>
        <KpiCard title="Realized result">
          <span data-testid="op-realized-total">
            {closed.length > 0 ? <Pnl value={realizedTotal} /> : "—"}
          </span>
        </KpiCard>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>New option position</CardTitle>
          <CardDescription>
            Register an option you bought or wrote on the B3. It is a record with a result: it does
            not touch your net worth, summary, profitability or taxes. For the premium, use the
            price per option.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleCreate} className="flex flex-col gap-4">
            <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
              <Field label="Series ticker">
                <Input
                  required
                  placeholder="PETRK600"
                  value={series}
                  data-testid="op-series"
                  onChange={(e) => setSeries(e.currentTarget.value)}
                />
              </Field>
              <Field label="Underlying ticker">
                <Input
                  required
                  placeholder="PETR4"
                  value={underlying}
                  data-testid="op-underlying"
                  onChange={(e) => setUnderlying(e.currentTarget.value)}
                />
              </Field>
              <Field label="Type">
                <Select value={optionType} onValueChange={(v) => setOptionType(v as "call" | "put")}>
                  <SelectTrigger data-testid="op-type">
                    <SelectValue>{optionType === "call" ? "Call" : "Put"}</SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="call">Call</SelectItem>
                    <SelectItem value="put">Put</SelectItem>
                  </SelectContent>
                </Select>
              </Field>
              <Field label="Position">
                <Select value={side} onValueChange={(v) => setSide(v as "long" | "short")}>
                  <SelectTrigger data-testid="op-side">
                    <SelectValue>{SIDE_LABEL[side]}</SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="long">{SIDE_LABEL.long}</SelectItem>
                    <SelectItem value="short">{SIDE_LABEL.short}</SelectItem>
                  </SelectContent>
                </Select>
              </Field>
            </div>
            <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
              <Field label="Strike">
                <Input
                  required
                  type="number"
                  step="any"
                  value={strike}
                  data-testid="op-strike"
                  onChange={(e) => setStrike(e.currentTarget.value)}
                />
              </Field>
              <Field label="Expiration">
                <Input
                  required
                  type="date"
                  value={expiration}
                  data-testid="op-expiration"
                  onChange={(e) => setExpiration(e.currentTarget.value)}
                />
              </Field>
              <Field label="Quantity (options)">
                <Input
                  required
                  type="number"
                  step="any"
                  value={quantity}
                  data-testid="op-quantity"
                  onChange={(e) => setQuantity(e.currentTarget.value)}
                />
              </Field>
              <Field label="Premium per option">
                <Input
                  required
                  type="number"
                  step="any"
                  value={premium}
                  data-testid="op-premium"
                  onChange={(e) => setPremium(e.currentTarget.value)}
                />
              </Field>
            </div>
            <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
              <Field label="Open date">
                <Input
                  required
                  type="date"
                  value={openDate}
                  data-testid="op-open-date"
                  onChange={(e) => setOpenDate(e.currentTarget.value)}
                />
              </Field>
              <Field label="Fee (optional)">
                <Input
                  type="number"
                  step="any"
                  value={fee}
                  data-testid="op-fee"
                  onChange={(e) => setFee(e.currentTarget.value)}
                />
              </Field>
              <Field label="Notes (optional)" className="col-span-2">
                <Input value={notes} onChange={(e) => setNotes(e.currentTarget.value)} />
              </Field>
            </div>
            {createMutation.isError && (
              <p className="text-sm text-red-600">{createMutation.error.message}</p>
            )}
            <Button
              type="submit"
              className="w-fit"
              data-testid="op-submit"
              disabled={createMutation.isPending}
            >
              {createMutation.isPending ? "Saving..." : "Add position"}
            </Button>
          </form>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Open positions</CardTitle>
          <CardDescription>
            The market result uses the last price traded on the B3, with no exit fee. The first load
            of the day downloads about 80 MB and can take around 2 minutes.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-3">
            <Button
              type="button"
              variant="outline"
              size="sm"
              data-testid="op-refresh-marks"
              disabled={marksMutation.isPending || open.length === 0}
              onClick={() => marksMutation.mutate()}
            >
              {marksMutation.isPending ? "Loading prices (up to ~2 min)..." : "Refresh market prices"}
            </Button>
            {marks?.stale && (
              <span className="text-xs text-muted-foreground">
                The B3 is unreachable; showing older prices.
              </span>
            )}
          </div>
          {marksMutation.isError && (
            <p className="text-sm text-red-600">{marksMutation.error.message}</p>
          )}
          {positionsQuery.isError && (
            <p className="text-sm text-red-600">{positionsQuery.error.message}</p>
          )}
          {open.length === 0 ? (
            <p className="text-sm text-muted-foreground">No open option positions.</p>
          ) : (
            <Table data-testid="op-open-table">
              <TableHeader>
                <TableRow>
                  <TableHead>Series</TableHead>
                  <TableHead>Position</TableHead>
                  <TableHead>Strike</TableHead>
                  <TableHead>Expiry</TableHead>
                  <TableHead>Qty</TableHead>
                  <TableHead>Premium</TableHead>
                  <TableHead>Last price</TableHead>
                  <TableHead>Market result</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {open.map((p) => {
                  const mark = markFor(p.id);
                  return (
                    <TableRow key={p.id}>
                      <TableCell className="font-medium">
                        {p.series_ticker}
                        <span className="block text-xs font-normal text-muted-foreground">
                          on {p.underlying_ticker}
                        </span>
                      </TableCell>
                      <TableCell>{positionLabel(p)}</TableCell>
                      <TableCell>{p.strike.toFixed(2)}</TableCell>
                      <TableCell>
                        {formatDate(p.expiration)}
                        {p.is_expired ? (
                          <span
                            className="ml-2 text-xs font-medium text-red-600 dark:text-red-400"
                            data-testid={`op-expired-${p.id}`}
                          >
                            Expired: close it
                          </span>
                        ) : (
                          <span className="ml-2 text-xs text-muted-foreground">
                            ({p.days_to_expiry}d)
                          </span>
                        )}
                      </TableCell>
                      <TableCell>{p.quantity}</TableCell>
                      <TableCell>{p.premium.toFixed(2)}</TableCell>
                      <TableCell>
                        {mark?.last_price != null ? mark.last_price.toFixed(2) : "—"}
                        {marks && mark?.last_price == null && (
                          <span className="block text-xs text-muted-foreground">never traded</span>
                        )}
                      </TableCell>
                      <TableCell>
                        <Pnl value={mark?.unrealized_pnl} />
                      </TableCell>
                      <TableCell className="whitespace-nowrap text-right">
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          data-testid={`close-position-${p.id}`}
                          onClick={() => startClosing(p)}
                        >
                          Close
                        </Button>
                        {confirmingDeleteId === p.id ? (
                          <Button
                            type="button"
                            variant="destructive"
                            size="sm"
                            className="ml-2"
                            onClick={() => deleteMutation.mutate(p.id)}
                          >
                            Confirm delete
                          </Button>
                        ) : (
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            className="ml-2"
                            data-testid={`delete-position-${p.id}`}
                            onClick={() => setConfirmingDeleteId(p.id)}
                          >
                            Delete
                          </Button>
                        )}
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {closed.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Closed positions</CardTitle>
          </CardHeader>
          <CardContent>
            <Table data-testid="op-closed-table">
              <TableHeader>
                <TableRow>
                  <TableHead>Series</TableHead>
                  <TableHead>Position</TableHead>
                  <TableHead>Strike</TableHead>
                  <TableHead>Closed</TableHead>
                  <TableHead>Qty</TableHead>
                  <TableHead>Premium</TableHead>
                  <TableHead>Close price</TableHead>
                  <TableHead>Realized result</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {closed.map((p) => (
                  <TableRow key={p.id}>
                    <TableCell className="font-medium">{p.series_ticker}</TableCell>
                    <TableCell>{positionLabel(p)}</TableCell>
                    <TableCell>{p.strike.toFixed(2)}</TableCell>
                    <TableCell>
                      {p.close_date ? formatDate(p.close_date) : "—"}
                      {p.close_kind && (
                        <span className="ml-2 text-xs text-muted-foreground">
                          {CLOSE_SHORT[p.close_kind]}
                        </span>
                      )}
                    </TableCell>
                    <TableCell>{p.quantity}</TableCell>
                    <TableCell>{p.premium.toFixed(2)}</TableCell>
                    <TableCell>{p.close_price != null ? p.close_price.toFixed(2) : "—"}</TableCell>
                    <TableCell data-testid={`op-realized-${p.id}`}>
                      <Pnl value={p.realized_pnl} />
                    </TableCell>
                    <TableCell className="text-right">
                      {confirmingDeleteId === p.id ? (
                        <Button
                          type="button"
                          variant="destructive"
                          size="sm"
                          onClick={() => deleteMutation.mutate(p.id)}
                        >
                          Confirm delete
                        </Button>
                      ) : (
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          onClick={() => setConfirmingDeleteId(p.id)}
                        >
                          Delete
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      <Dialog open={closing !== null} onOpenChange={(isOpen) => !isOpen && setClosing(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Close {closing?.series_ticker}</DialogTitle>
          </DialogHeader>
          <form
            className="flex flex-col gap-4"
            onSubmit={(e) => {
              e.preventDefault();
              closeMutation.mutate();
            }}
          >
            <Field label="How it ended">
              <Select value={closeKind} onValueChange={(v) => setCloseKind(v as CloseKind)}>
                <SelectTrigger data-testid="close-kind">
                  <SelectValue>{CLOSE_LABEL[closeKind]}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="closed">{CLOSE_LABEL.closed}</SelectItem>
                  <SelectItem value="expired">{CLOSE_LABEL.expired}</SelectItem>
                  <SelectItem value="exercised">{CLOSE_LABEL.exercised}</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            <Field label={closeKind === "exercised" ? "Intrinsic value per option" : "Close price per option"}>
              <Input
                required={closeKind !== "expired"}
                type="number"
                step="any"
                disabled={closeKind === "expired"}
                value={closeKind === "expired" ? "0" : closePrice}
                data-testid="close-price"
                onChange={(e) => setClosePrice(e.currentTarget.value)}
              />
            </Field>
            {closeKind === "exercised" && (
              <p className="text-xs text-muted-foreground">
                Enter the option's value at exercise (how much it was in the money). The stock
                bought or sold by the exercise is not recorded here.
              </p>
            )}
            <div className="grid grid-cols-2 gap-4">
              <Field label="Close date">
                <Input
                  required
                  type="date"
                  value={closeDate}
                  data-testid="close-date"
                  onChange={(e) => setCloseDate(e.currentTarget.value)}
                />
              </Field>
              <Field label="Fee (optional)">
                <Input
                  type="number"
                  step="any"
                  value={closeFee}
                  data-testid="close-fee"
                  onChange={(e) => setCloseFee(e.currentTarget.value)}
                />
              </Field>
            </div>
            {closeMutation.isError && (
              <p className="text-sm text-red-600">{closeMutation.error.message}</p>
            )}
            <Button
              type="submit"
              data-testid="close-submit"
              disabled={closeMutation.isPending}
            >
              {closeMutation.isPending ? "Closing..." : "Close position"}
            </Button>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}

export default OptionPositionsSection;
