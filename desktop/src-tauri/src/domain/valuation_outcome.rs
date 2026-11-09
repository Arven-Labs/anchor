use crate::error::AppError;

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

impl ValuationOutcome {
    /// Fecha o cálculo de qualquer modelo de valuation a partir do preço justo.
    ///
    /// A margem de segurança divide pelo justo: com justo < 0 o sinal inverte e
    /// um valuation negativo viraria "Barata"; com 0 ou NaN a margem não
    /// existe. Por isso o guard mora aqui, na mesma função que calcula a
    /// margem, e nenhum modelo consegue esquecer de chamá-lo.
    pub fn from_fair_price(fair_price: f64, current_price: f64) -> Result<Self, AppError> {
        if !(fair_price > 0.0) {
            return Err(AppError::InvalidGuard(
                "fair price must be greater than zero".to_string(),
            ));
        }

        let safety_margin = (fair_price - current_price) / fair_price;
        let verdict = if safety_margin > 0.0 {
            Verdict::Cheap
        } else {
            Verdict::Expensive
        };

        Ok(Self {
            fair_price,
            safety_margin,
            verdict,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cheap_when_current_price_is_below_fair_price() {
        let outcome = ValuationOutcome::from_fair_price(50.0, 40.0).unwrap();

        assert!((outcome.fair_price - 50.0).abs() < 1e-9);
        assert!((outcome.safety_margin - 0.2).abs() < 1e-9);
        assert_eq!(outcome.verdict.as_str(), "CHEAP");
    }

    #[test]
    fn expensive_when_current_price_is_above_fair_price() {
        let outcome = ValuationOutcome::from_fair_price(50.0, 60.0).unwrap();

        assert!((outcome.safety_margin + 0.2).abs() < 1e-9);
        assert_eq!(outcome.verdict.as_str(), "EXPENSIVE");
    }

    #[test]
    fn expensive_when_margin_is_exactly_zero() {
        let outcome = ValuationOutcome::from_fair_price(50.0, 50.0).unwrap();

        assert_eq!(outcome.safety_margin, 0.0);
        assert_eq!(outcome.verdict.as_str(), "EXPENSIVE");
    }

    #[test]
    fn rejects_fair_price_that_is_not_finite_and_positive() {
        for rejected in [0.0, -0.0, -214.28, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                matches!(
                    ValuationOutcome::from_fair_price(rejected, 10.0),
                    Err(AppError::InvalidGuard(_))
                ),
                "{rejected}"
            );
        }
    }
}
