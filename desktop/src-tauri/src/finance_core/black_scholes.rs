// Black-Scholes para opções europeias, sem dividendos — porta de
// `lume/api/app/services/black_scholes.py`. Matemática pura, sem rede nem
// estado: a vol implícita e as gregas são recalculadas a cada chamada, porque o
// preço do ativo-objeto muda a todo instante.
//
// Escalas de mercado (não os valores crus por ano/unidade de vol): `delta` e
// `gamma` por R$ 1 do ativo, `vega` por 1 ponto percentual de volatilidade,
// `theta` por dia corrido (÷ 365). Série americana usa a mesma aproximação
// europeia: não há fonte pra validar exercício antecipado.

use std::f64::consts::{PI, SQRT_2};

const MIN_VOLATILITY: f64 = 1e-4;
const MAX_VOLATILITY: f64 = 5.0;
const MAX_NEWTON_ITERATIONS: usize = 50;
const MAX_BISECTION_ITERATIONS: usize = 100;
const PRICE_TOLERANCE: f64 = 1e-6;
const INITIAL_VOLATILITY_GUESS: f64 = 0.3;
/// Abaixo disso a derivada é zero pra efeito prático e o Newton não anda.
const MIN_VEGA_FOR_NEWTON: f64 = 1e-8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
    Call,
    Put,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Greeks {
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
}

fn norm_cdf(x: f64) -> f64 {
    0.5 * (1.0 + libm::erf(x / SQRT_2))
}

fn norm_pdf(x: f64) -> f64 {
    (-x * x / 2.0).exp() / (2.0 * PI).sqrt()
}

fn d1_d2(spot: f64, strike: f64, years: f64, rate: f64, vol: f64) -> (f64, f64) {
    let d1 = ((spot / strike).ln() + (rate + vol * vol / 2.0) * years) / (vol * years.sqrt());
    (d1, d1 - vol * years.sqrt())
}

pub fn price(option_type: OptionType, spot: f64, strike: f64, years: f64, rate: f64, vol: f64) -> f64 {
    let (d1, d2) = d1_d2(spot, strike, years, rate, vol);
    let discounted_strike = strike * (-rate * years).exp();
    match option_type {
        OptionType::Call => spot * norm_cdf(d1) - discounted_strike * norm_cdf(d2),
        OptionType::Put => discounted_strike * norm_cdf(-d2) - spot * norm_cdf(-d1),
    }
}

pub fn greeks(option_type: OptionType, spot: f64, strike: f64, years: f64, rate: f64, vol: f64) -> Greeks {
    let (d1, d2) = d1_d2(spot, strike, years, rate, vol);
    let pdf_d1 = norm_pdf(d1);
    let discounted_strike = strike * (-rate * years).exp();

    let gamma = pdf_d1 / (spot * vol * years.sqrt());
    let vega = spot * pdf_d1 * years.sqrt() / 100.0;
    let decay = -spot * pdf_d1 * vol / (2.0 * years.sqrt());

    let (delta, theta) = match option_type {
        OptionType::Call => (norm_cdf(d1), (decay - rate * discounted_strike * norm_cdf(d2)) / 365.0),
        OptionType::Put => (norm_cdf(d1) - 1.0, (decay + rate * discounted_strike * norm_cdf(-d2)) / 365.0),
    };
    Greeks { delta, gamma, theta, vega }
}

/// Volatilidade (fração, 0,35 = 35 %) que reproduz `market_price`: Newton com a
/// vega como derivada e bisseção se não convergir. `None` quando o preço está
/// abaixo do valor intrínseco (nenhuma vol positiva o reproduz: sinal de preço
/// velho, não de bug) ou quando não há solução no intervalo [0,01 %, 500 %].
pub fn implied_volatility(
    option_type: OptionType,
    market_price: f64,
    spot: f64,
    strike: f64,
    years: f64,
    rate: f64,
) -> Option<f64> {
    let intrinsic = match option_type {
        OptionType::Call => (spot - strike).max(0.0),
        OptionType::Put => (strike - spot).max(0.0),
    };
    if market_price < intrinsic - PRICE_TOLERANCE {
        return None;
    }

    let diff_at = |vol: f64| price(option_type, spot, strike, years, rate, vol) - market_price;

    let mut vol = INITIAL_VOLATILITY_GUESS;
    for _ in 0..MAX_NEWTON_ITERATIONS {
        let diff = diff_at(vol);
        if diff.abs() < PRICE_TOLERANCE {
            return Some(vol);
        }
        let vega_raw = spot * norm_pdf(d1_d2(spot, strike, years, rate, vol).0) * years.sqrt();
        if vega_raw < MIN_VEGA_FOR_NEWTON {
            break;
        }
        vol = (vol - diff / vega_raw).clamp(MIN_VOLATILITY, MAX_VOLATILITY);
    }

    let (mut low, mut high) = (MIN_VOLATILITY, MAX_VOLATILITY);
    let mut diff_low = diff_at(low);
    if diff_low * diff_at(high) > 0.0 {
        return None;
    }
    for _ in 0..MAX_BISECTION_ITERATIONS {
        let mid = (low + high) / 2.0;
        let diff_mid = diff_at(mid);
        if diff_mid.abs() < PRICE_TOLERANCE {
            return Some(mid);
        }
        if diff_low * diff_mid < 0.0 {
            high = mid;
        } else {
            low = mid;
            diff_low = diff_mid;
        }
    }
    Some((low + high) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Caso do Hull: S = K = 100, T = 1, r = 5 %, σ = 20 %.
    const S: f64 = 100.0;
    const K: f64 = 100.0;
    const T: f64 = 1.0;
    const R: f64 = 0.05;
    const SIGMA: f64 = 0.20;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() < tolerance
    }

    #[test]
    fn prices_match_the_hull_example() {
        assert!(close(price(OptionType::Call, S, K, T, R, SIGMA), 10.4506, 1e-3));
        assert!(close(price(OptionType::Put, S, K, T, R, SIGMA), 5.5735, 1e-3));
    }

    #[test]
    fn greeks_match_the_hull_example() {
        let call = greeks(OptionType::Call, S, K, T, R, SIGMA);
        assert!(close(call.delta, 0.6368, 1e-3), "{}", call.delta);
        assert!(close(call.gamma, 0.018762, 1e-4), "{}", call.gamma);
        assert!(close(call.vega, 0.37524, 1e-3), "{}", call.vega);
        assert!(close(call.theta, -0.017573, 1e-4), "{}", call.theta);

        let put = greeks(OptionType::Put, S, K, T, R, SIGMA);
        assert!(close(put.delta, -0.3632, 1e-3), "{}", put.delta);
        // Gamma e vega são iguais pra call e put.
        assert!(close(put.gamma, call.gamma, 1e-12));
        assert!(close(put.vega, call.vega, 1e-12));
    }

    #[test]
    fn put_call_parity_holds() {
        let call = price(OptionType::Call, S, K, T, R, SIGMA);
        let put = price(OptionType::Put, S, K, T, R, SIGMA);
        assert!(close(call - put, S - K * (-R * T).exp(), 1e-6));
    }

    #[test]
    fn implied_volatility_round_trips() {
        for (option_type, sigma) in [(OptionType::Call, 0.35), (OptionType::Put, 0.5)] {
            let market = price(option_type, S, K, T, R, sigma);
            let iv = implied_volatility(option_type, market, S, K, T, R).unwrap();
            assert!(close(iv, sigma, 1e-4), "{option_type:?}: {iv}");
        }
    }

    #[test]
    fn price_below_intrinsic_has_no_implied_volatility() {
        assert_eq!(implied_volatility(OptionType::Call, 10.0, 150.0, 100.0, 1.0, 0.05), None);
        assert_eq!(implied_volatility(OptionType::Put, 5.0, 50.0, 100.0, 1.0, 0.05), None);
    }

    #[test]
    fn deep_in_the_money_still_solves() {
        // Newton tem pouca vega aqui: cai na bisseção ou converge por ela.
        let market = price(OptionType::Call, 150.0, 100.0, 0.25, R, 0.4);
        let iv = implied_volatility(OptionType::Call, market, 150.0, 100.0, 0.25, R);
        assert!(iv.is_none_or(|v| close(v, 0.4, 1e-2)), "{iv:?}");
    }
}
