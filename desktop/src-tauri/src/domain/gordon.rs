use super::valuation_outcome::ValuationOutcome;
use crate::error::AppError;

#[derive(serde::Deserialize)]
pub struct GordonInputs {
    pub current_dividend: f64,
    pub expected_growth: f64,
    pub ke: f64,
}

// D1 = D0 × (1 + g)
// Preco_Justo = D1 / (Ke − g)
pub fn calculate(inputs: &GordonInputs, current_price: f64) -> Result<ValuationOutcome, AppError> {
    if inputs.ke <= inputs.expected_growth {
        return Err(AppError::InvalidGuard(
            "Ke must be greater than the expected growth rate".to_string(),
        ));
    }

    let d1 = inputs.current_dividend * (1.0 + inputs.expected_growth);
    let fair_price = d1 / (inputs.ke - inputs.expected_growth);
    ValuationOutcome::from_fair_price(fair_price, current_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_fair_price() {
        for current_dividend in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let inputs = GordonInputs {
                current_dividend,
                expected_growth: 0.03,
                ke: 0.10,
            };

            assert!(
                matches!(calculate(&inputs, 40.0), Err(AppError::InvalidGuard(_))),
                "dividendo atual {current_dividend}"
            );
        }
    }

    #[test]
    fn rejects_non_positive_fair_price() {
        for current_dividend in [-2.0, 0.0] {
            let inputs = GordonInputs {
                current_dividend,
                expected_growth: 0.03,
                ke: 0.10,
            };

            assert!(
                matches!(calculate(&inputs, 25.0), Err(AppError::InvalidGuard(_))),
                "dividendo atual {current_dividend}"
            );
        }
    }

    #[test]
    fn cheap_when_fair_price_above_current_price() {
        let inputs = GordonInputs {
            current_dividend: 2.0,
            expected_growth: 0.03,
            ke: 0.10,
        };

        let outcome = calculate(&inputs, 25.0).unwrap();

        assert!((outcome.fair_price - 29.428571428571427).abs() < 1e-9);
        assert!(outcome.safety_margin > 0.0);
        assert_eq!(outcome.verdict.as_str(), "CHEAP");
    }

    #[test]
    fn expensive_when_fair_price_below_current_price() {
        let inputs = GordonInputs {
            current_dividend: 2.0,
            expected_growth: 0.03,
            ke: 0.10,
        };

        let outcome = calculate(&inputs, 40.0).unwrap();

        assert!(outcome.safety_margin < 0.0);
        assert_eq!(outcome.verdict.as_str(), "EXPENSIVE");
    }

    #[test]
    fn rejects_ke_equal_to_growth() {
        let inputs = GordonInputs {
            current_dividend: 2.0,
            expected_growth: 0.05,
            ke: 0.05,
        };

        assert!(matches!(
            calculate(&inputs, 25.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    #[test]
    fn rejects_ke_below_growth() {
        let inputs = GordonInputs {
            current_dividend: 2.0,
            expected_growth: 0.08,
            ke: 0.05,
        };

        assert!(matches!(
            calculate(&inputs, 25.0),
            Err(AppError::InvalidGuard(_))
        ));
    }
}
