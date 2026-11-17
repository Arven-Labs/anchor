import { useState, type FormEvent } from "react";
import { RefreshCw } from "lucide-react";
import { useTickerCollector } from "../collector/useTickerCollector";
import { useValuationActions } from "./useValuationActions";
import ValuationResult from "../components/ValuationResult";
import Field from "../components/Field";
import RiskFreeHint from "./RiskFreeHint";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

type CalculateProjectedCeilingRequest = {
  ticker: string;
  reference_year: number;
  current_price: number;
  current_dividend: number;
  expected_growth: number;
  projection_years: number;
  desired_yield: number;
  ke: number;
};

type ProjectedCeilingInputsModel = {
  id: number;
  valuation_id: number;
  current_dividend: number;
  expected_growth: number;
  projection_years: number;
  desired_yield: number;
  ke: number;
};

function ProjectedCeilingForm({ ticker: initialTicker, assetClass }: { ticker?: string; assetClass?: string } = {}) {
  const [ticker, setTicker] = useState(initialTicker ?? "");
  const [referenceYear, setReferenceYear] = useState(
    String(new Date().getFullYear()),
  );
  const [currentPrice, setCurrentPrice] = useState("");
  const [currentDividend, setCurrentDividend] = useState("");
  const [expectedGrowth, setExpectedGrowth] = useState("");
  const [projectionYears, setProjectionYears] = useState("5");
  const [desiredYield, setDesiredYield] = useState("6");
  const [ke, setKe] = useState("");

  const tickerCollector = useTickerCollector(assetClass);
  const [tickerError, setTickerError] = useState<string | null>(null);

  const { calculateMutation, saveMutation } = useValuationActions<
    CalculateProjectedCeilingRequest,
    ProjectedCeilingInputsModel
  >("projected_ceiling");
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
    if (data.dividendsAvg) {
      setCurrentDividend(String(data.dividendsAvg.avg_dividend_5y));
      filled++;
    }
    if (filled === 0) {
      setTickerError(`No data found for ${ticker.toUpperCase()}.`);
    }
  }

  function buildRequest(): CalculateProjectedCeilingRequest {
    return {
      ticker: ticker.toUpperCase(),
      reference_year: Number(referenceYear),
      current_price: Number(currentPrice),
      current_dividend: Number(currentDividend),
      expected_growth: Number(expectedGrowth) / 100,
      projection_years: Number(projectionYears),
      desired_yield: Number(desiredYield) / 100,
      ke: Number(ke) / 100,
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
        <CardTitle>Fair Price (Projected Ceiling)</CardTitle>
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
                placeholder="ITSA4"
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

          <Field label="Current dividend — D0 (R$/share)">
            <Input
              required
              type="number"
              step="0.01"
              value={currentDividend}
              onChange={(e) => setCurrentDividend(e.currentTarget.value)}
            />
            <p className="text-xs text-muted-foreground">
              Pre-filled with the average dividend of the last 5 years —
              adjust if you have the exact value of the last dividend paid.
            </p>
          </Field>

          <Field label="Expected growth — g (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={expectedGrowth}
              onChange={(e) => setExpectedGrowth(e.currentTarget.value)}
            />
          </Field>

          <Field label="Projection years — N">
            <Input
              required
              type="number"
              step="1"
              value={projectionYears}
              onChange={(e) => setProjectionYears(e.currentTarget.value)}
            />
          </Field>

          <Field label="Desired yield (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={desiredYield}
              onChange={(e) => setDesiredYield(e.currentTarget.value)}
            />
          </Field>

          <Field label="Discount rate — Ke (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={ke}
              onChange={(e) => setKe(e.currentTarget.value)}
            />
            <RiskFreeHint ke={ke} />
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

export default ProjectedCeilingForm;
