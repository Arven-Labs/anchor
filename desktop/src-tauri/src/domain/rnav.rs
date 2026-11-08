use super::valuation_outcome::ValuationOutcome;
use crate::error::AppError;

#[derive(serde::Deserialize)]
pub struct RnavInputs {
    pub landbank: f64,
    pub inventory_at_market_value: f64,
    pub net_cash: f64,
    pub shares_outstanding: f64,
}

// RNAV_Total = Landbank + Estoque + Caixa_Líquido
// RNAV/Ação  = RNAV_Total / Nº_Ações
pub fn calculate(inputs: &RnavInputs, current_price: f64) -> Result<ValuationOutcome, AppError> {
    if inputs.shares_outstanding <= 0.0 {
        return Err(AppError::InvalidGuard(
            "shares outstanding must be greater than zero".to_string(),
        ));
    }

    let rnav_total = inputs.landbank + inputs.inventory_at_market_value + inputs.net_cash;
    let fair_price = rnav_total / inputs.shares_outstanding;

    ValuationOutcome::from_fair_price(fair_price, current_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_positive_fair_price() {
        // 100 + 50 - 400 = -250 no total: mais dívida líquida que ativos.
        for net_cash in [-400.0, -150.0] {
            let inputs = RnavInputs {
                landbank: 100.0,
                inventory_at_market_value: 50.0,
                net_cash,
                shares_outstanding: 100.0,
            };

            assert!(
                matches!(calculate(&inputs, 8.0), Err(AppError::InvalidGuard(_))),
                "caixa líquido {net_cash}"
            );
        }
    }

    #[test]
    fn cheap_when_fair_price_above_current_price() {
        let inputs = RnavInputs {
            landbank: 500.0,
            inventory_at_market_value: 300.0,
            net_cash: 200.0,
            shares_outstanding: 100.0,
        };

        let outcome = calculate(&inputs, 8.0).unwrap();

        assert!((outcome.fair_price - 10.0).abs() < 1e-9);
        assert!(outcome.safety_margin > 0.0);
        assert_eq!(outcome.verdict.as_str(), "CHEAP");
    }

    #[test]
    fn expensive_when_fair_price_below_current_price() {
        let inputs = RnavInputs {
            landbank: 500.0,
            inventory_at_market_value: 300.0,
            net_cash: 200.0,
            shares_outstanding: 100.0,
        };

        let outcome = calculate(&inputs, 15.0).unwrap();

        assert!(outcome.safety_margin < 0.0);
        assert_eq!(outcome.verdict.as_str(), "EXPENSIVE");
    }

    #[test]
    fn handles_negative_net_cash() {
        let inputs = RnavInputs {
            landbank: 500.0,
            inventory_at_market_value: 300.0,
            net_cash: -400.0,
            shares_outstanding: 100.0,
        };

        let outcome = calculate(&inputs, 3.0).unwrap();

        assert!((outcome.fair_price - 4.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_non_positive_shares_outstanding() {
        let inputs = RnavInputs {
            landbank: 500.0,
            inventory_at_market_value: 300.0,
            net_cash: 200.0,
            shares_outstanding: 0.0,
        };

        assert!(matches!(
            calculate(&inputs, 8.0),
            Err(AppError::InvalidGuard(_))
        ));
    }
}
