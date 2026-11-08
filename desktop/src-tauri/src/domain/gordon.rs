use crate::error::AppError;

#[derive(serde::Deserialize)]
pub struct GordonInputs {
    pub current_dividend: f64,
    pub expected_growth: f64,
    pub ke: f64,
}

pub enum Verdict {
    Cheap,
    Expensive,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Cheap => "CHEAP",
            Verdict::Expensive => "EXPENSIVE",
        }
    }
}

pub struct ValuationOutcome {
    pub fair_price: f64,
    pub safety_margin: f64,
    pub verdict: Verdict,
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
    super::ensure_positive_fair_price(fair_price)?;
    let safety_margin = (fair_price - current_price) / fair_price;
    let verdict = if safety_margin > 0.0 {
        Verdict::Cheap
    } else {
        Verdict::Expensive
    };

    Ok(ValuationOutcome {
        fair_price,
        safety_margin,
        verdict,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
