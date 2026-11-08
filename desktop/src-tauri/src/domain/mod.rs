pub mod alert_check;
pub mod banks;
pub mod backtest;
pub mod bazin;
pub mod benchmark;
pub mod calendar;
pub mod cost_basis;
pub mod crypto_score;
pub mod currency_conversion;
pub mod dcf;
pub mod dividend_suggestion;
pub mod gordon;
pub mod graham;
pub mod liability;
pub mod loan;
pub mod net_worth;
pub mod option_payoff;
pub mod option_position;
pub mod porquinho_yield;
pub mod position_pricing;
pub mod projected_ceiling;
pub mod proventos;
pub mod ptax;
pub mod rates;
pub mod rim;
pub mod rnav;
pub mod tax;
pub mod transaction_ledger;
pub mod twr;

use crate::error::AppError;

/// Recusa preço justo que não seja finito e positivo. A margem de segurança
/// divide por ele: com justo < 0 o sinal inverte e um valuation negativo
/// viraria "Barata"; com 0 ou NaN a margem não existe.
pub(crate) fn ensure_positive_fair_price(fair_price: f64) -> Result<(), AppError> {
    if fair_price.is_finite() && fair_price > 0.0 {
        Ok(())
    } else {
        Err(AppError::InvalidGuard(
            "fair price must be greater than zero".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_finite_positive_fair_price() {
        assert!(ensure_positive_fair_price(0.01).is_ok());
        assert!(ensure_positive_fair_price(50.0).is_ok());

        for rejected in [0.0, -0.0, -214.28, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                matches!(
                    ensure_positive_fair_price(rejected),
                    Err(AppError::InvalidGuard(_))
                ),
                "{rejected}"
            );
        }
    }
}
