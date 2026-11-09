use super::valuation_outcome::ValuationOutcome;
use crate::error::AppError;

#[derive(serde::Deserialize)]
pub struct BazinInputs {
    pub average_dividend: f64,
    pub desired_yield: f64,
}

// Preco_Teto = Dividendo_Médio / Yield_Desejado
pub fn calculate(inputs: &BazinInputs, current_price: f64) -> Result<ValuationOutcome, AppError> {
    if inputs.desired_yield <= 0.0 {
        return Err(AppError::InvalidGuard(
            "desired yield must be greater than zero".to_string(),
        ));
    }

    let fair_price = inputs.average_dividend / inputs.desired_yield;
    ValuationOutcome::from_fair_price(fair_price, current_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_fair_price() {
        for average_dividend in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let inputs = BazinInputs {
                average_dividend,
                desired_yield: 0.06,
            };

            assert!(
                matches!(calculate(&inputs, 40.0), Err(AppError::InvalidGuard(_))),
                "dividendo médio {average_dividend}"
            );
        }
    }

    #[test]
    fn rejects_non_positive_fair_price() {
        for average_dividend in [-3.0, 0.0] {
            let inputs = BazinInputs {
                average_dividend,
                desired_yield: 0.06,
            };

            assert!(
                matches!(calculate(&inputs, 40.0), Err(AppError::InvalidGuard(_))),
                "dividendo médio {average_dividend}"
            );
        }
    }

    #[test]
    fn cheap_when_fair_price_above_current_price() {
        let inputs = BazinInputs {
            average_dividend: 3.0,
            desired_yield: 0.06,
        };

        let outcome = calculate(&inputs, 40.0).unwrap();

        assert!((outcome.fair_price - 50.0).abs() < 1e-9);
        assert!((outcome.safety_margin - 0.2).abs() < 1e-9);
        assert_eq!(outcome.verdict.as_str(), "CHEAP");
    }

    #[test]
    fn expensive_when_fair_price_below_current_price() {
        let inputs = BazinInputs {
            average_dividend: 3.0,
            desired_yield: 0.06,
        };

        let outcome = calculate(&inputs, 60.0).unwrap();

        assert!(outcome.safety_margin < 0.0);
        assert_eq!(outcome.verdict.as_str(), "EXPENSIVE");
    }

    #[test]
    fn rejects_non_positive_desired_yield() {
        let inputs = BazinInputs {
            average_dividend: 3.0,
            desired_yield: 0.0,
        };

        assert!(matches!(
            calculate(&inputs, 40.0),
            Err(AppError::InvalidGuard(_))
        ));
    }
}
