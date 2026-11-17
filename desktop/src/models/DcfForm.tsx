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

type CalculateDcfRequest = {
  ticker: string;
  reference_year: number;
  current_price: number;
  ebit: number;
  tax_rate: number;
  depreciation_amortization: number;
  capex: number;
  nwc_change: number;
  total_debt: number;
  cash: number;
  shares_outstanding: number;
  beta: number;
  risk_free_rate: number;
  market_risk_premium: number;
  kd: number;
  perpetuity_growth: number;
};

type DcfInputsModel = {
  id: number;
  valuation_id: number;
  ebit: number;
  tax_rate: number;
  depreciation_amortization: number;
  capex: number;
  nwc_change: number;
  total_debt: number;
  cash: number;
  shares_outstanding: number;
  beta: number;
  risk_free_rate: number;
  market_risk_premium: number;
  kd: number;
  perpetuity_growth: number;
};

function SectionHeading({ children }: { children: string }) {
  return (
    <h2 className="mt-2 text-sm font-semibold text-muted-foreground sm:col-span-2">
      {children}
    </h2>
  );
}

function DcfForm({ ticker: initialTicker, assetClass }: { ticker?: string; assetClass?: string } = {}) {
  const [ticker, setTicker] = useState(initialTicker ?? "");
  const [referenceYear, setReferenceYear] = useState(
    String(new Date().getFullYear()),
  );
  const [currentPrice, setCurrentPrice] = useState("");

  const [ebit, setEbit] = useState("");
  const [taxRate, setTaxRate] = useState("");
  const [depreciationAmortization, setDepreciationAmortization] = useState("");
  const [capex, setCapex] = useState("");
  const [nwcChange, setNwcChange] = useState("");

  const [totalDebt, setTotalDebt] = useState("");
  const [cash, setCash] = useState("");
  const [sharesOutstanding, setSharesOutstanding] = useState("");

  const [beta, setBeta] = useState("");
  const [riskFreeRate, setRiskFreeRate] = useState("");
  const [marketRiskPremium, setMarketRiskPremium] = useState("");
  const [kd, setKd] = useState("");
  const [perpetuityGrowth, setPerpetuityGrowth] = useState("");

  const tickerCollector = useTickerCollector(assetClass);
  const [tickerError, setTickerError] = useState<string | null>(null);

  const { calculateMutation, saveMutation } = useValuationActions<
    CalculateDcfRequest,
    DcfInputsModel
  >("dcf");
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
      const dcf = data.dcfFundamentals;
      setEbit(String(dcf.ebit));
      if (dcf.tax_rate !== null) setTaxRate(String(dcf.tax_rate));
      if (dcf.depreciation_amortization !== null) {
        setDepreciationAmortization(String(dcf.depreciation_amortization));
      }
      if (dcf.capex !== null) setCapex(String(dcf.capex));
      setNwcChange(String(dcf.nwc_change));
      setTotalDebt(String(dcf.total_debt));
      setCash(String(dcf.cash));
      setSharesOutstanding(String(dcf.shares_outstanding));
      filled++;
    }
    if (filled === 0) {
      setTickerError(`No data found for ${ticker.toUpperCase()}.`);
    }
  }

  function buildRequest(): CalculateDcfRequest {
    return {
      ticker: ticker.toUpperCase(),
      reference_year: Number(referenceYear),
      current_price: Number(currentPrice),
      ebit: Number(ebit),
      tax_rate: Number(taxRate) / 100,
      depreciation_amortization: Number(depreciationAmortization),
      capex: Number(capex),
      nwc_change: Number(nwcChange),
      total_debt: Number(totalDebt),
      cash: Number(cash),
      shares_outstanding: Number(sharesOutstanding),
      beta: Number(beta),
      risk_free_rate: Number(riskFreeRate) / 100,
      market_risk_premium: Number(marketRiskPremium) / 100,
      kd: Number(kd) / 100,
      perpetuity_growth: Number(perpetuityGrowth) / 100,
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
        <CardTitle>Fair Price (DCF / FCFF)</CardTitle>
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

          <SectionHeading>Operational</SectionHeading>

          <Field label="EBIT (R$ millions)">
            <Input
              required
              type="number"
              step="0.01"
              value={ebit}
              onChange={(e) => setEbit(e.currentTarget.value)}
            />
          </Field>

          <Field label="Effective tax rate (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={taxRate}
              onChange={(e) => setTaxRate(e.currentTarget.value)}
            />
          </Field>

          <Field label="D&A — depreciation/amortization (R$ millions)">
            <Input
              required
              type="number"
              step="0.01"
              value={depreciationAmortization}
              onChange={(e) => setDepreciationAmortization(e.currentTarget.value)}
            />
          </Field>

          <Field label="Capex (R$ millions)">
            <Input
              required
              type="number"
              step="0.01"
              value={capex}
              onChange={(e) => setCapex(e.currentTarget.value)}
            />
          </Field>

          <Field label="ΔNWC — change in net working capital (R$ millions)">
            <Input
              required
              type="number"
              step="0.01"
              value={nwcChange}
              onChange={(e) => setNwcChange(e.currentTarget.value)}
            />
          </Field>

          <SectionHeading>Capital structure</SectionHeading>

          <Field label="Total debt (R$ millions)">
            <Input
              required
              type="number"
              step="0.01"
              value={totalDebt}
              onChange={(e) => setTotalDebt(e.currentTarget.value)}
            />
          </Field>

          <Field label="Cash (R$ millions)">
            <Input
              required
              type="number"
              step="0.01"
              value={cash}
              onChange={(e) => setCash(e.currentTarget.value)}
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

          <SectionHeading>Cost of capital</SectionHeading>

          <Field label="Beta">
            <Input
              required
              type="number"
              step="0.01"
              value={beta}
              onChange={(e) => setBeta(e.currentTarget.value)}
            />
          </Field>

          <Field label="Risk-free rate — Rf (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={riskFreeRate}
              onChange={(e) => setRiskFreeRate(e.currentTarget.value)}
            />
            <RiskFreeHint onUse={(ratePct) => setRiskFreeRate(String(ratePct))} />
          </Field>

          <Field label="Market risk premium (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={marketRiskPremium}
              onChange={(e) => setMarketRiskPremium(e.currentTarget.value)}
            />
          </Field>

          <Field label="Cost of debt — Kd (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={kd}
              onChange={(e) => setKd(e.currentTarget.value)}
            />
          </Field>

          <Field label="Perpetuity growth — g (%)">
            <Input
              required
              type="number"
              step="0.1"
              value={perpetuityGrowth}
              onChange={(e) => setPerpetuityGrowth(e.currentTarget.value)}
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

export default DcfForm;
