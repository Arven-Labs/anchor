use super::valuation_outcome::ValuationOutcome;
use crate::error::AppError;

#[derive(serde::Deserialize)]
pub struct BanksInputs {
    pub book_value_per_share: f64,
    pub roe: f64,
    pub payout: f64,
    pub ke: f64,
}

// g_sustentável = ROE × (1 − Payout)
// P/B_Justo     = (ROE − g_sustentável) / (Ke − g_sustentável)
// Preco_Justo   = P/B_Justo × VPA
pub fn calculate(inputs: &BanksInputs, current_price: f64) -> Result<ValuationOutcome, AppError> {
    let sustainable_growth = inputs.roe * (1.0 - inputs.payout);

    if inputs.ke <= sustainable_growth {
        return Err(AppError::InvalidGuard(
            "Ke must be greater than the sustainable growth rate".to_string(),
        ));
    }

    let fair_pb = (inputs.roe - sustainable_growth) / (inputs.ke - sustainable_growth);
    let fair_price = fair_pb * inputs.book_value_per_share;

    ValuationOutcome::from_fair_price(fair_price, current_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_fair_price() {
        for book_value_per_share in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let inputs = BanksInputs {
                book_value_per_share,
                roe: 0.15,
                payout: 0.4,
                ke: 0.12,
            };

            assert!(
                matches!(calculate(&inputs, 40.0), Err(AppError::InvalidGuard(_))),
                "VPA {book_value_per_share}"
            );
        }
    }

    #[test]
    fn rejects_zero_fair_price_when_roe_is_zero() {
        let inputs = BanksInputs {
            book_value_per_share: 20.0,
            roe: 0.0,
            payout: 0.4,
            ke: 0.12,
        };

        assert!(matches!(
            calculate(&inputs, 40.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    #[test]
    fn rejects_non_positive_fair_price() {
        let inputs = BanksInputs {
            book_value_per_share: -20.0,
            roe: 0.18,
            payout: 0.5,
            ke: 0.12,
        };

        assert!(matches!(
            calculate(&inputs, 25.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    #[test]
    fn cheap_when_fair_price_above_current_price() {
        let inputs = BanksInputs {
            book_value_per_share: 20.0,
            roe: 0.18,
            payout: 0.5,
            ke: 0.12,
        };

        let outcome = calculate(&inputs, 25.0).unwrap();

        assert!(outcome.fair_price > 0.0);
        assert!(outcome.safety_margin > 0.0);
        assert_eq!(outcome.verdict.as_str(), "CHEAP");
    }

    #[test]
    fn expensive_when_fair_price_below_current_price() {
        let inputs = BanksInputs {
            book_value_per_share: 20.0,
            roe: 0.18,
            payout: 0.5,
            ke: 0.12,
        };

        let outcome = calculate(&inputs, 90.0).unwrap();

        assert!(outcome.safety_margin < 0.0);
        assert_eq!(outcome.verdict.as_str(), "EXPENSIVE");
    }

    #[test]
    fn rejects_ke_equal_to_sustainable_growth() {
        let inputs = BanksInputs {
            book_value_per_share: 20.0,
            // g_sustentável = 0.18 * (1 - 0.5) = 0.09 = ke
            roe: 0.18,
            payout: 0.5,
            ke: 0.09,
        };

        assert!(matches!(
            calculate(&inputs, 25.0),
            Err(AppError::InvalidGuard(_))
        ));
    }
}
