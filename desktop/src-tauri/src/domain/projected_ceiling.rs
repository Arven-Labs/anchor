use super::valuation_outcome::ValuationOutcome;
use crate::error::AppError;

#[derive(serde::Deserialize)]
pub struct ProjectedCeilingInputs {
    pub current_dividend: f64,
    pub expected_growth: f64,
    pub projection_years: i32,
    pub desired_yield: f64,
    pub ke: f64,
}

// Dividendo_Projetado_N = D0 × (1 + g)^N
// Preco_Teto_Futuro_N   = Dividendo_Projetado_N / Yield_Desejado
// Preco_Teto_Projetivo  = Preco_Teto_Futuro_N / (1 + Ke)^N
pub fn calculate(
    inputs: &ProjectedCeilingInputs,
    current_price: f64,
) -> Result<ValuationOutcome, AppError> {
    if inputs.desired_yield <= 0.0 {
        return Err(AppError::InvalidGuard(
            "desired yield must be greater than zero".to_string(),
        ));
    }

    let projected_dividend =
        inputs.current_dividend * (1.0 + inputs.expected_growth).powi(inputs.projection_years);
    let future_ceiling_price = projected_dividend / inputs.desired_yield;
    let fair_price = future_ceiling_price / (1.0 + inputs.ke).powi(inputs.projection_years);

    ValuationOutcome::from_fair_price(fair_price, current_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_positive_fair_price() {
        for current_dividend in [-3.0, 0.0] {
            let inputs = ProjectedCeilingInputs {
                current_dividend,
                expected_growth: 0.05,
                projection_years: 5,
                desired_yield: 0.06,
                ke: 0.10,
            };

            assert!(
                matches!(calculate(&inputs, 30.0), Err(AppError::InvalidGuard(_))),
                "dividendo atual {current_dividend}"
            );
        }
    }

    #[test]
    fn cheap_when_fair_price_above_current_price() {
        let inputs = ProjectedCeilingInputs {
            current_dividend: 3.0,
            expected_growth: 0.05,
            projection_years: 5,
            desired_yield: 0.06,
            ke: 0.10,
        };

        let outcome = calculate(&inputs, 30.0).unwrap();

        assert!(outcome.fair_price > 0.0);
        assert!(outcome.safety_margin > 0.0);
        assert_eq!(outcome.verdict.as_str(), "CHEAP");
    }

    #[test]
    fn expensive_when_fair_price_below_current_price() {
        let inputs = ProjectedCeilingInputs {
            current_dividend: 3.0,
            expected_growth: 0.05,
            projection_years: 5,
            desired_yield: 0.06,
            ke: 0.10,
        };

        let outcome = calculate(&inputs, 100.0).unwrap();

        assert!(outcome.safety_margin < 0.0);
        assert_eq!(outcome.verdict.as_str(), "EXPENSIVE");
    }

    #[test]
    fn rejects_non_positive_desired_yield() {
        let inputs = ProjectedCeilingInputs {
            current_dividend: 3.0,
            expected_growth: 0.05,
            projection_years: 5,
            desired_yield: 0.0,
            ke: 0.10,
        };

        assert!(matches!(
            calculate(&inputs, 30.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    #[test]
    fn zero_projection_years_collapses_to_bazin() {
        let inputs = ProjectedCeilingInputs {
            current_dividend: 3.0,
            expected_growth: 0.05,
            projection_years: 0,
            desired_yield: 0.06,
            ke: 0.10,
        };

        let outcome = calculate(&inputs, 40.0).unwrap();

        // Com N=0 não há projeção nem desconto: vira o Bazin puro (3.0 / 0.06 = 50.0).
        assert!((outcome.fair_price - 50.0).abs() < 1e-9);
    }
}
