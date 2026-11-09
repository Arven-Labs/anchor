use super::valuation_outcome::ValuationOutcome;
use crate::error::AppError;

#[derive(serde::Deserialize)]
pub struct RimInputs {
    pub book_value_per_share: f64,
    pub roe_current: f64,
    pub payout: f64,
    pub ke: f64,
    pub fade_years: i32,
}

// Generaliza o modelo `banks` (P/B via ROE-Gordon, estágio único) permitindo o
// ROE convergir (fade linear) do patamar atual até o próprio Ke ao longo de N
// anos, em vez de assumir ROE constante pra sempre — a ideia de "fade to cost
// of equity" é a leitura padrão de que a vantagem competitiva de um banco
// erode até ele passar a criar exatamente zero lucro econômico no limite.
//
// Para t = 1..N:
//   ROE_t            = ROE_atual + (Ke − ROE_atual) × (t/N)
//   LucroResidual_t   = (ROE_t − Ke) × VPA_(t-1)
//   VPA_t             = VPA_(t-1) × (1 + ROE_t × (1 − Payout))
// Preco_Justo         = VPA0 + Σ VP(LucroResidual_t)
//
// Não há valor terminal a somar: em t=N o ROE já é exatamente Ke, então o
// lucro residual dali em diante é zero por construção — sem perpetuidade,
// sem guarda de crescimento terminal.
//
// Quando ROE_atual == Ke, o fade não produz nenhum lucro residual em nenhum
// ano (a série inteira já nasce em zero) e o preço justo cai exatamente no
// valor patrimonial — mesmo resultado do modelo `banks` nesse caso particular
// (ver teste `matches_book_value_when_roe_equals_ke`).
pub fn calculate(inputs: &RimInputs, current_price: f64) -> Result<ValuationOutcome, AppError> {
    if inputs.fade_years < 1 {
        return Err(AppError::InvalidGuard(
            "fade_years must be at least 1".to_string(),
        ));
    }

    let n = inputs.fade_years;
    let mut book_value = inputs.book_value_per_share;
    let mut stage_value = 0.0;

    for t in 1..=n {
        let weight = f64::from(t) / f64::from(n);
        let roe_t = inputs.roe_current + (inputs.ke - inputs.roe_current) * weight;
        let residual_income = (roe_t - inputs.ke) * book_value;
        stage_value += residual_income / (1.0 + inputs.ke).powi(t);
        book_value *= 1.0 + roe_t * (1.0 - inputs.payout);
    }

    let fair_price = inputs.book_value_per_share + stage_value;

    ValuationOutcome::from_fair_price(fair_price, current_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_fair_price() {
        for book_value_per_share in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let inputs = RimInputs {
                book_value_per_share,
                roe_current: 0.15,
                payout: 0.4,
                ke: 0.12,
                fade_years: 5,
            };

            assert!(
                matches!(calculate(&inputs, 40.0), Err(AppError::InvalidGuard(_))),
                "VPA {book_value_per_share}"
            );
        }
    }

    #[test]
    fn rejects_zero_fair_price_when_book_value_is_zero() {
        let inputs = RimInputs {
            book_value_per_share: 0.0,
            roe_current: 0.15,
            payout: 0.4,
            ke: 0.12,
            fade_years: 5,
        };

        assert!(matches!(
            calculate(&inputs, 40.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    #[test]
    fn rejects_non_positive_fair_price() {
        let mut inputs = base_inputs();
        inputs.book_value_per_share = -20.0;

        assert!(matches!(
            calculate(&inputs, 20.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    fn base_inputs() -> RimInputs {
        RimInputs {
            book_value_per_share: 20.0,
            roe_current: 0.18,
            payout: 0.4,
            ke: 0.12,
            fade_years: 5,
        }
    }

    #[test]
    fn cheap_when_fair_price_above_current_price() {
        let outcome = calculate(&base_inputs(), 20.0).unwrap();

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
    fn rejects_non_positive_fade_years() {
        let mut inputs = base_inputs();
        inputs.fade_years = 0;

        assert!(matches!(
            calculate(&inputs, 20.0),
            Err(AppError::InvalidGuard(_))
        ));
    }

    // Quando o ROE atual já é igual a Ke, o fade não gera lucro residual em
    // nenhum ano (a série inteira nasce em zero) — o preço justo precisa cair
    // exatamente no valor patrimonial, o mesmo resultado do modelo `banks`
    // nesse caso particular (ROE = Ke ⇒ P/B_Justo = 1).
    #[test]
    fn matches_book_value_when_roe_equals_ke() {
        let inputs = RimInputs {
            book_value_per_share: 15.0,
            roe_current: 0.12,
            payout: 0.5,
            ke: 0.12,
            fade_years: 7,
        };

        let outcome = calculate(&inputs, 20.0).unwrap();

        assert!((outcome.fair_price - 15.0).abs() < 1e-9);
    }
}
