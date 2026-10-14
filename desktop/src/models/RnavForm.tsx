import { useState, type FormEvent } from "react";
import { RefreshCw } from "lucide-react";
import { useTickerCollector } from "../collector/useTickerCollector";
import { useValuationActions } from "./useValuationActions";
import ValuationResult from "../components/ValuationResult";
import Field from "../components/Field";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

type CalculateRnavRequest = {
  ticker: string;
  reference_year: number;
  current_price: number;
  landbank: number;
  inventory_at_market_value: number;
  net_cash: number;
  shares_outstanding: number;
};

type RnavInputsModel = {
  id: number;
  valuation_id: number;
  landbank: number;
  inventory_at_market_value: number;
  net_cash: number;
  shares_outstanding: number;
};

function RnavForm({ ticker: initialTicker, assetClass }: { ticker?: string; assetClass?: string } = {}) {
  const [ticker, setTicker] = useState(initialTicker ?? "");
  const [referenceYear, setReferenceYear] = useState(
    String(new Date().getFullYear()),
  );
  const [currentPrice, setCurrentPrice] = useState("");
  const [landbank, setLandbank] = useState("");
  const [inventoryAtMarketValue, setInventoryAtMarketValue] = useState("");
  const [netCash, setNetCash] = useState("");
  const [sharesOutstanding, setSharesOutstanding] = useState("");

  const tickerCollector = useTickerCollector(assetClass);
  const [tickerError, setTickerError] = useState<string | null>(null);

  const { calculateMutation, saveMutation } = useValuationActions<
    CalculateRnavRequest,
    RnavInputsModel
  >("rnav");
  const [lastAction, setLastAction] = useState<"calculate" | "save" | null>(null);

  async function handleFetch() {
    if (!ticker.trim()) {
      setTickerError("Ticker is required to fetch data.");
      return;
    }
    setTickerError(null);
    const data = await tickerCollector.mutateAsync(ticker).catch(() => null);
    if (!data) return;
    let filled = 0;
    if (data.quote) {
      setCurrentPrice(String(data.quote.price));
      filled++;
    }
    if (data.dcfFundamentals) {
      setSharesOutstanding(String(data.dcfFundamentals.shares_outstanding));
      setNetCash(
        String(data.dcfFundamentals.cash - data.dcfFundamentals.total_debt),
      );
      if (data.dcfFundamentals.inventory !== null) {
        setInventoryAtMarketValue(String(data.dcfFundamentals.inventory));
      }
      filled++;
    }
    if (filled === 0) {
      setTickerError(`No data found for ${ticker.toUpperCase()}.`);
    }
  }

  function buildRequest(): CalculateRnavRequest {
    return {
      ticker: ticker.toUpperCase(),
      reference_year: Number(referenceYear),
      current_price: Number(currentPrice),
      landbank: Number(landbank),
      inventory_at_market_value: Number(inventoryAtMarketValue),
      net_cash: Number(netCash),
      shares_outstanding: Number(sharesOutstanding),
    };
  }

  function handleCalculate(event: FormEvent) {
    event.preventDefault();
    setLastAction("calculate");
    calculateMutation.mutate(buildRequest());
  }

  function handleSave() {
    setLastAction("save");
    saveMutation.mutate(buildRequest());
  }

  const activeMutation = lastAction === "save" ? saveMutation : calculateMutation;
  const resultValuation =
    lastAction === "save" ? (saveMutation.data?.valuation ?? null) : (calculateMutation.data ?? null);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Fair Price (RNAV)</CardTitle>
      </CardHeader>
      <CardContent>
        <form
          onSubmit={handleCalculate}
          className="grid grid-cols-1 gap-4 sm:grid-cols-2"
        >
          <Field label="Ticker">
            <div className="flex gap-2">
              <Input
                required
                value={ticker}
                onChange={(e) => setTicker(e.currentTarget.value)}
                placeholder="CYRE3"
                className="flex-1"
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                onClick={handleFetch}
                disabled={tickerCollector.isPending}
                aria-label="Fetch data for this ticker"
                title="Fetch data for this ticker"
              >
                <RefreshCw
                  className={tickerCollector.isPending ? "animate-spin" : ""}
                />
              </Button>
            </div>
            {tickerError && <p className="text-red-600">{tickerError}</p>}
            {tickerCollector.isError && (
              <p className="text-red-600">{tickerCollector.error.message}</p>
            )}
          </Field>

          <Field label="Reference year">
            <Input
              required
              type="number"
              value={referenceYear}
              onChange={(e) => setReferenceYear(e.currentTarget.value)}
            />
          </Field>

          <Field label="Current price (R$)">
            <Input
              required
              type="number"
              step="0.01"
              value={currentPrice}
              onChange={(e) => setCurrentPrice(e.currentTarget.value)}
            />
          </Field>

          <Field label="Landbank at market value (R$ millions)">
            <Input
              required
              type="number"
              step="0.01"
              value={landbank}
              onChange={(e) => setLandbank(e.currentTarget.value)}
            />
            <p className="text-xs text-muted-foreground">
              Not in structured balance-sheet data — enter manually from the
              investor-relations release.
            </p>
          </Field>

          <Field label="Inventory at market value (R$ millions)">
            <Input
              required
              type="number"
              step="0.01"
              value={inventoryAtMarketValue}
              onChange={(e) => setInventoryAtMarketValue(e.currentTarget.value)}
            />
            <p className="text-xs text-muted-foreground">
              Auto-filled with CVM book value — adjust to market value if
              known.
            </p>
          </Field>

          <Field label="Net cash — cash minus debt, can be negative (R$ millions)">
            <Input
              required
              type="number"
              step="0.01"
              value={netCash}
              onChange={(e) => setNetCash(e.currentTarget.value)}
            />
          </Field>

          <Field label="Shares outstanding (millions)">
            <Input
              required
              type="number"
              step="0.01"
              value={sharesOutstanding}
              onChange={(e) => setSharesOutstanding(e.currentTarget.value)}
            />
          </Field>

          <div className="flex gap-3 sm:col-span-2">
            <Button type="submit" disabled={calculateMutation.isPending} className="flex-1">
              {calculateMutation.isPending ? "Calculating..." : "Calculate"}
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={handleSave}
              disabled={saveMutation.isPending}
              className="flex-1"
            >
              {saveMutation.isPending ? "Saving..." : "Save"}
            </Button>
          </div>
        </form>

        <ValuationResult
          isError={activeMutation.isError}
          error={activeMutation.error ?? null}
          isSuccess={activeMutation.isSuccess}
          valuation={resultValuation}
        />
        {lastAction === "save" && saveMutation.isSuccess && (
          <p className="mt-2 text-sm text-green-700">Saved.</p>
        )}

      </CardContent>
    </Card>
  );
}

export default RnavForm;
