use crate::error::AppError;

#[derive(serde::Deserialize)]
pub struct GrahamInputs {
    pub eps: f64,
    pub book_value_per_share: f64,
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

// Graham_Number = RAIZ(22.5 × LPA × VPA)
pub fn calculate(inputs: &GrahamInputs, current_price: f64) -> Result<ValuationOutcome, AppError> {
    if inputs.eps <= 0.0 || inputs.book_value_per_share <= 0.0 {
        return Err(AppError::InvalidGuard(
            "EPS and book value per share must both be greater than zero".to_string(),
        ));
    }

    let fair_price = (22.5 * inputs.eps * inputs.book_value_per_share).sqrt();
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
    fn rejects_non_finite_fair_price() {
        let inputs = GrahamInputs {
            eps: f64::NAN,
            book_value_per_share: 20.0,
        };

        assert!(matches!(
            calculate(&inputs, 20.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    #[test]
    fn cheap_when_fair_price_above_current_price() {
        let inputs = GrahamInputs {
            eps: 2.0,
            book_value_per_share: 20.0,
        };

        let outcome = calculate(&inputs, 20.0).unwrap();

        assert!((outcome.fair_price - 30.0).abs() < 1e-9);
        assert!(outcome.safety_margin > 0.0);
        assert_eq!(outcome.verdict.as_str(), "CHEAP");
    }

    #[test]
    fn expensive_when_fair_price_below_current_price() {
        let inputs = GrahamInputs {
            eps: 2.0,
            book_value_per_share: 20.0,
        };

        let outcome = calculate(&inputs, 40.0).unwrap();

        assert!(outcome.safety_margin < 0.0);
        assert_eq!(outcome.verdict.as_str(), "EXPENSIVE");
    }

    #[test]
    fn rejects_non_positive_eps() {
        let inputs = GrahamInputs {
            eps: 0.0,
            book_value_per_share: 20.0,
        };

        assert!(matches!(
            calculate(&inputs, 20.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    #[test]
    fn rejects_non_positive_book_value() {
        let inputs = GrahamInputs {
            eps: 2.0,
            book_value_per_share: -1.0,
        };

        assert!(matches!(
            calculate(&inputs, 20.0),
            Err(AppError::InvalidGuard(_))
        ));
    }
}
