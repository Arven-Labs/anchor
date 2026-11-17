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

type CalculateRimRequest = {
  ticker: string;
  reference_year: number;
  current_price: number;
  book_value_per_share: number;
  roe_current: number;
  payout: number;
  ke: number;
  fade_years: number;
};

type RimInputsModel = {
  id: number;
  valuation_id: number;
  book_value_per_share: number;
  roe_current: number;
  payout: number;
  ke: number;
  fade_years: number;
};

function RimForm({ ticker: initialTicker, assetClass }: { ticker?: string; assetClass?: string } = {}) {
  const [ticker, setTicker] = useState(initialTicker ?? "");
  const [referenceYear, setReferenceYear] = useState(
    String(new Date().getFullYear()),
  );
  const [currentPrice, setCurrentPrice] = useState("");
  const [bookValuePerShare, setBookValuePerShare] = useState("");
  const [roeCurrent, setRoeCurrent] = useState("");
  const [payout, setPayout] = useState("");
  const [ke, setKe] = useState("");
  const [fadeYears, setFadeYears] = useState("5");

  const tickerCollector = useTickerCollector(assetClass);
  const [tickerError, setTickerError] = useState<string | null>(null);

  const { calculateMutation, saveMutation } = useValuationActions<
    CalculateRimRequest,
    RimInputsModel
  >("rim");
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
    if (data.fundamentals) {
      setBookValuePerShare(String(data.fundamentals.vpa));
      setRoeCurrent(String(data.fundamentals.roe));
      if (data.fundamentals.payout !== null) {
        setPayout(String(data.fundamentals.payout));
      }
      filled++;
    }
    if (filled === 0) {
      setTickerError(`No data found for ${ticker.toUpperCase()}.`);
    }
  }

  function buildRequest(): CalculateRimRequest {
    return {
      ticker: ticker.toUpperCase(),
      reference_year: Number(referenceYear),
      current_price: Number(currentPrice),
      book_value_per_share: Number(bookValuePerShare),
      roe_current: Number(roeCurrent) / 100,
      payout: Number(payout) / 100,
      ke: Number(ke) / 100,
      fade_years: Number(fadeYears),
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
        <CardTitle>Fair Price (RIM — Lucro Residual, Bancos)</CardTitle>
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
                placeholder="ITUB4"
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

          <Field label="Book value per share (R$)">
            <Input
              required
              type="number"
              step="0.01"
              value={bookValuePerShare}
              onChange={(e) => setBookValuePerShare(e.currentTarget.value)}
            />
          </Field>

          <Field label="Current ROE (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={roeCurrent}
              onChange={(e) => setRoeCurrent(e.currentTarget.value)}
            />
          </Field>

          <Field label="Payout (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={payout}
              onChange={(e) => setPayout(e.currentTarget.value)}
            />
          </Field>

          <Field label="Required return — Ke (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={ke}
              onChange={(e) => setKe(e.currentTarget.value)}
            />
            <RiskFreeHint ke={ke} />
            <p className="text-xs text-muted-foreground">
              Use a nominal Ke — the model discounts nominal book value/ROE
              flows with no inflation adjustment, so a real Ke would inflate
              the fair price.
            </p>
          </Field>

          <Field label="Fade years — N (ROE converges to Ke)">
            <Input
              required
              type="number"
              step="1"
              min="1"
              value={fadeYears}
              onChange={(e) => setFadeYears(e.currentTarget.value)}
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

export default RimForm;
