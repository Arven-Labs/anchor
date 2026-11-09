use super::valuation_outcome::ValuationOutcome;
use crate::error::AppError;

#[derive(serde::Deserialize)]
pub struct DcfInputs {
    pub ebit: f64,
    pub tax_rate: f64,
    pub depreciation_amortization: f64,
    pub capex: f64,
    pub nwc_change: f64,
    pub total_debt: f64,
    pub cash: f64,
    pub shares_outstanding: f64,
    pub beta: f64,
    pub risk_free_rate: f64,
    pub market_risk_premium: f64,
    pub kd: f64,
    pub perpetuity_growth: f64,
}

// FCFF         = EBIT × (1 − IR) + D&A − Capex − ΔNWC
// Ke (CAPM)    = Rf + Beta × Prêmio_Risco_Mercado
// E (equity)   = Preço_Atual × Nº_Ações
// WACC         = [E / (E + Dívida)] × Ke + [Dívida / (E + Dívida)] × Kd × (1 − IR)
// Valor_Firma  = FCFF × (1 + g) / (WACC − g)
// Valor_Equity = Valor_Firma − Dívida_Total + Caixa
// Preco_Justo  = Valor_Equity / Nº_Ações
pub fn calculate(inputs: &DcfInputs, current_price: f64) -> Result<ValuationOutcome, AppError> {
    let fcff = inputs.ebit * (1.0 - inputs.tax_rate) + inputs.depreciation_amortization
        - inputs.capex
        - inputs.nwc_change;
    let ke = inputs.risk_free_rate + inputs.beta * inputs.market_risk_premium;
    let equity = current_price * inputs.shares_outstanding;
    let wacc = (equity / (equity + inputs.total_debt)) * ke
        + (inputs.total_debt / (equity + inputs.total_debt)) * inputs.kd * (1.0 - inputs.tax_rate);

    if wacc - inputs.perpetuity_growth <= 0.0 {
        return Err(AppError::InvalidGuard(
            "WACC must be greater than the perpetuity growth rate".to_string(),
        ));
    }

    let firm_value = fcff * (1.0 + inputs.perpetuity_growth) / (wacc - inputs.perpetuity_growth);
    let equity_value = firm_value - inputs.total_debt + inputs.cash;
    let fair_price = equity_value / inputs.shares_outstanding;

    ValuationOutcome::from_fair_price(fair_price, current_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_negative_fair_price_instead_of_calling_it_cheap() {
        let mut inputs = base_inputs();
        inputs.ebit = -1_000.0;

        assert!(matches!(
            calculate(&inputs, 10.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    #[test]
    fn rejects_non_finite_fair_price() {
        for ebit in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut inputs = base_inputs();
            inputs.ebit = ebit;

            assert!(
                matches!(calculate(&inputs, 10.0), Err(AppError::InvalidGuard(_))),
                "ebit {ebit}"
            );
        }
    }

    #[test]
    fn rejects_zero_fair_price_when_there_is_no_cash_flow_debt_or_cash() {
        let mut inputs = base_inputs();
        inputs.ebit = 0.0;
        inputs.depreciation_amortization = 0.0;
        inputs.capex = 0.0;
        inputs.nwc_change = 0.0;
        inputs.total_debt = 0.0;
        inputs.cash = 0.0;

        assert!(matches!(
            calculate(&inputs, 10.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    fn base_inputs() -> DcfInputs {
        DcfInputs {
            ebit: 1_000.0,
            tax_rate: 0.34,
            depreciation_amortization: 200.0,
            capex: 300.0,
            nwc_change: 50.0,
            total_debt: 2_000.0,
            cash: 500.0,
            shares_outstanding: 100.0,
            beta: 1.0,
            risk_free_rate: 0.06,
            market_risk_premium: 0.05,
            kd: 0.08,
            perpetuity_growth: 0.03,
        }
    }

    #[test]
    fn cheap_when_fair_price_above_current_price() {
        let outcome = calculate(&base_inputs(), 10.0).unwrap();

        assert!(outcome.fair_price > 0.0);
        assert!(outcome.safety_margin > 0.0);
        assert_eq!(outcome.verdict.as_str(), "CHEAP");
    }

    #[test]
    fn expensive_when_fair_price_below_current_price() {
        let outcome = calculate(&base_inputs(), 200.0).unwrap();

        assert!(outcome.safety_margin < 0.0);
        assert_eq!(outcome.verdict.as_str(), "EXPENSIVE");
    }

    #[test]
    fn rejects_when_wacc_does_not_exceed_growth() {
        let mut inputs = base_inputs();
        // Beta absurdamente baixo derruba o Ke, e por consequência o WACC, abaixo do g.
        inputs.beta = -10.0;
        inputs.perpetuity_growth = 0.05;

        assert!(matches!(
            calculate(&inputs, 10.0),
            Err(AppError::InvalidGuard(_))
        ));
    }
}
