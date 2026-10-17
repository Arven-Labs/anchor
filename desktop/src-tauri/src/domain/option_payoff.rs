use serde::{Deserialize, Serialize};

use crate::error::AppError;

// Fase 18, 2ª fatia (Sessão 115) — resultado de uma estratégia com opções no
// vencimento. Função pura, sem rede. O resultado da carteira de pernas é
// linear por partes em `S` (preço final da ação), com quebras só nos strikes;
// por isso extremos, equilíbrios e "ilimitado" saem exatos dos pontos de
// quebra (`0` e cada strike) e da inclinação à direita, sem amostrar.
// Sem custos, IR, dividendos nem exercício antecipado.

pub const MAX_LEGS: usize = 8;
const CURVE_POINTS: usize = 121;
const EPS: f64 = 1e-9;
/// A curva vai de 60% da menor referência a 140% da maior.
const CURVE_LOW_FACTOR: f64 = 0.6;
const CURVE_HIGH_FACTOR: f64 = 1.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegKind {
    Stock,
    Call,
    Put,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Long,
    Short,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Leg {
    pub kind: LegKind,
    pub side: Side,
    /// Só em opção.
    pub strike: Option<f64>,
    /// Prêmio da opção ou preço de entrada da ação, por unidade.
    pub price: f64,
    pub quantity: f64,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct PayoffPoint {
    pub price: f64,
    pub pnl: f64,
}

#[derive(Debug, Serialize)]
pub struct StrategyAnalysis {
    pub points: Vec<PayoffPoint>,
    /// Preços finais em que o resultado cruza zero, crescentes.
    pub breakevens: Vec<f64>,
    /// Melhor resultado possível; `None` quando é ilimitado.
    pub max_gain: Option<f64>,
    /// Pior resultado possível (negativo quando há perda); `None` quando é ilimitado.
    pub max_loss: Option<f64>,
    pub unlimited_gain: bool,
    pub unlimited_loss: bool,
    /// Prêmio líquido das opções: positivo = débito, negativo = crédito (a ação não entra).
    pub net_debit: f64,
}

fn sign(side: Side) -> f64 {
    match side {
        Side::Long => 1.0,
        Side::Short => -1.0,
    }
}

fn invalid(detail: impl Into<String>) -> AppError {
    AppError::InvalidInput(detail.into())
}

fn validate(legs: &[Leg]) -> Result<(), AppError> {
    if legs.is_empty() {
        return Err(invalid("add at least one leg"));
    }
    if legs.len() > MAX_LEGS {
        return Err(invalid(format!("at most {MAX_LEGS} legs are supported")));
    }
    for leg in legs {
        if !leg.quantity.is_finite() || leg.quantity <= 0.0 {
            return Err(invalid("quantity must be greater than zero"));
        }
        if !leg.price.is_finite() || leg.price < 0.0 {
            return Err(invalid("price must be zero or greater"));
        }
        match (leg.kind, leg.strike) {
            (LegKind::Stock, None) => {}
            (LegKind::Stock, Some(_)) => return Err(invalid("a stock leg has no strike")),
            (_, Some(strike)) if strike.is_finite() && strike > 0.0 => {}
            _ => return Err(invalid("an option leg needs a strike greater than zero")),
        }
    }
    Ok(())
}

/// Resultado de uma perna se a ação terminar em `s`.
fn leg_pnl(leg: &Leg, s: f64) -> f64 {
    let strike = leg.strike.unwrap_or(0.0);
    let per_unit = match leg.kind {
        LegKind::Stock => s - leg.price,
        LegKind::Call => (s - strike).max(0.0) - leg.price,
        LegKind::Put => (strike - s).max(0.0) - leg.price,
    };
    sign(leg.side) * leg.quantity * per_unit
}

fn total_pnl(legs: &[Leg], s: f64) -> f64 {
    legs.iter().map(|leg| leg_pnl(leg, s)).sum()
}

/// Inclinação do resultado acima do maior strike: ação e call comprada
/// ganham 1 por unidade, vendidas perdem; put não tem inclinação à direita.
fn slope_at_infinity(legs: &[Leg]) -> f64 {
    legs.iter()
        .map(|leg| match leg.kind {
            LegKind::Stock | LegKind::Call => sign(leg.side) * leg.quantity,
            LegKind::Put => 0.0,
        })
        .sum()
}

fn push_unique(values: &mut Vec<f64>, value: f64) {
    if !values.iter().any(|v| (v - value).abs() < EPS * (1.0 + value.abs())) {
        values.push(value);
    }
}

/// Preços em que o resultado cruza zero (`S > 0`), crescentes.
fn breakevens(legs: &[Leg], breaks: &[f64], slope: f64) -> Vec<f64> {
    let values: Vec<f64> = breaks.iter().map(|&b| total_pnl(legs, b)).collect();
    let is_zero = |v: f64| v.abs() < EPS;
    let mut found = Vec::new();

    for i in 0..breaks.len().saturating_sub(1) {
        let (b0, b1, v0, v1) = (breaks[i], breaks[i + 1], values[i], values[i + 1]);
        if is_zero(v0) && is_zero(v1) {
            continue; // trecho zerado: não há ponto de equilíbrio, é uma faixa
        }
        if v0 * v1 < 0.0 {
            push_unique(&mut found, b0 + (b1 - b0) * (-v0) / (v1 - v0));
        }
    }
    for (i, &b) in breaks.iter().enumerate() {
        let flat_neighbor = (i > 0 && is_zero(values[i - 1])) || (i + 1 < breaks.len() && is_zero(values[i + 1]));
        if b > 0.0 && is_zero(values[i]) && !flat_neighbor {
            push_unique(&mut found, b);
        }
    }
    let last = breaks.len() - 1;
    if slope.abs() > EPS && values[last] * slope < 0.0 {
        push_unique(&mut found, breaks[last] - values[last] / slope);
    }
    found.sort_by(|a, b| a.total_cmp(b));
    found
}

/// Analisa a estratégia no vencimento. `spot` só ajusta a faixa do gráfico.
pub fn analyze(legs: &[Leg], spot: Option<f64>) -> Result<StrategyAnalysis, AppError> {
    validate(legs)?;

    let mut breaks = vec![0.0];
    for strike in legs.iter().filter_map(|l| l.strike) {
        push_unique(&mut breaks, strike);
    }
    breaks.sort_by(|a, b| a.total_cmp(b));

    let slope = slope_at_infinity(legs);
    let values: Vec<f64> = breaks.iter().map(|&b| total_pnl(legs, b)).collect();
    let unlimited_gain = slope > EPS;
    let unlimited_loss = slope < -EPS;
    let best = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let worst = values.iter().copied().fold(f64::INFINITY, f64::min);
    let breakevens = breakevens(legs, &breaks, slope);

    let net_debit = legs
        .iter()
        .filter(|l| l.kind != LegKind::Stock)
        .map(|l| sign(l.side) * l.price * l.quantity)
        .sum();

    Ok(StrategyAnalysis {
        points: curve(legs, &breaks, &breakevens, spot),
        breakevens,
        max_gain: (!unlimited_gain).then_some(best),
        max_loss: (!unlimited_loss).then_some(worst),
        unlimited_gain,
        unlimited_loss,
        net_debit,
    })
}

/// Grade uniforme mais as quebras e os equilíbrios dentro da faixa, pra o gráfico
/// mostrar os cantos exatos.
fn curve(legs: &[Leg], breaks: &[f64], breakevens: &[f64], spot: Option<f64>) -> Vec<PayoffPoint> {
    let mut refs: Vec<f64> = breaks.iter().copied().filter(|&b| b > 0.0).collect();
    refs.extend(legs.iter().filter(|l| l.kind == LegKind::Stock && l.price > 0.0).map(|l| l.price));
    refs.extend(spot.filter(|s| s.is_finite() && *s > 0.0));
    refs.extend(breakevens.iter().copied());
    let low_ref = refs.iter().copied().fold(f64::INFINITY, f64::min);
    let high_ref = refs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (low, high) = if refs.is_empty() {
        (0.0, 1.0)
    } else {
        ((low_ref * CURVE_LOW_FACTOR).max(0.0), high_ref * CURVE_HIGH_FACTOR)
    };

    let step = (high - low) / (CURVE_POINTS - 1) as f64;
    let mut xs: Vec<f64> = (0..CURVE_POINTS).map(|i| low + step * i as f64).collect();
    for &x in breaks.iter().chain(breakevens) {
        if x >= low && x <= high {
            push_unique(&mut xs, x);
        }
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    xs.into_iter().map(|price| PayoffPoint { price, pnl: total_pnl(legs, price) }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn stock(side: Side, price: f64, quantity: f64) -> Leg {
        Leg { kind: LegKind::Stock, side, strike: None, price, quantity }
    }

    fn call(side: Side, strike: f64, premium: f64, quantity: f64) -> Leg {
        Leg { kind: LegKind::Call, side, strike: Some(strike), price: premium, quantity }
    }

    fn put(side: Side, strike: f64, premium: f64, quantity: f64) -> Leg {
        Leg { kind: LegKind::Put, side, strike: Some(strike), price: premium, quantity }
    }

    #[test]
    fn long_call_loses_the_premium_and_has_unlimited_gain() {
        let a = analyze(&[call(Side::Long, 40.0, 2.0, 100.0)], Some(40.0)).unwrap();
        assert_eq!(a.max_loss, Some(-200.0));
        assert!(a.unlimited_gain && a.max_gain.is_none() && !a.unlimited_loss);
        assert_eq!(a.breakevens.len(), 1);
        assert!(close(a.breakevens[0], 42.0));
        assert!(close(a.net_debit, 200.0));
    }

    #[test]
    fn covered_call_caps_the_gain_and_breaks_even_at_entry_minus_premium() {
        // Ação a 38 comprada, call 40 vendida a 1,50, 100 unidades.
        let a = analyze(&[stock(Side::Long, 38.0, 100.0), call(Side::Short, 40.0, 1.5, 100.0)], Some(38.0)).unwrap();
        // Ganho máximo: (40 − 38 + 1,5)·100 = 350. Perda máxima em S = 0: (−38 + 1,5)·100.
        assert!(close(a.max_gain.unwrap(), 350.0));
        assert!(close(a.max_loss.unwrap(), -3650.0));
        assert!(!a.unlimited_gain && !a.unlimited_loss);
        assert_eq!(a.breakevens.len(), 1);
        assert!(close(a.breakevens[0], 36.5));
        assert!(close(a.net_debit, -150.0)); // crédito de 150
    }

    #[test]
    fn protective_put_limits_the_loss_at_the_strike() {
        // Ação a 38, put 36 comprada a 1: perda máx = (36 − 38 − 1)·100 = −300.
        let a = analyze(&[stock(Side::Long, 38.0, 100.0), put(Side::Long, 36.0, 1.0, 100.0)], Some(38.0)).unwrap();
        assert!(close(a.max_loss.unwrap(), -300.0));
        assert!(a.unlimited_gain);
        assert!(close(a.breakevens[0], 39.0)); // entrada + prêmio
    }

    #[test]
    fn bull_call_spread_has_limited_gain_and_loss_and_one_breakeven() {
        // Compra call 40 a 3, vende call 45 a 1: débito 2; ganho máx 3, perda máx 2 (por unidade).
        let a = analyze(&[call(Side::Long, 40.0, 3.0, 100.0), call(Side::Short, 45.0, 1.0, 100.0)], Some(41.0)).unwrap();
        assert!(close(a.max_gain.unwrap(), 300.0));
        assert!(close(a.max_loss.unwrap(), -200.0));
        assert_eq!(a.breakevens.len(), 1);
        assert!(close(a.breakevens[0], 42.0));
        assert!(close(a.net_debit, 200.0));
    }

    #[test]
    fn long_straddle_has_two_breakevens_and_loses_the_total_premium() {
        let a = analyze(&[call(Side::Long, 40.0, 2.0, 100.0), put(Side::Long, 40.0, 1.0, 100.0)], Some(40.0)).unwrap();
        assert!(close(a.max_loss.unwrap(), -300.0));
        assert!(a.unlimited_gain);
        assert_eq!(a.breakevens.len(), 2);
        assert!(close(a.breakevens[0], 37.0) && close(a.breakevens[1], 43.0));
    }

    #[test]
    fn short_put_gain_is_the_premium_and_loss_is_bounded_by_zero_price() {
        // Put 40 vendida a 2: ganho máx 200; pior caso S = 0 → (−40 + 2)·100 = −3800.
        let a = analyze(&[put(Side::Short, 40.0, 2.0, 100.0)], Some(41.0)).unwrap();
        assert!(close(a.max_gain.unwrap(), 200.0));
        assert!(close(a.max_loss.unwrap(), -3800.0));
        assert!(!a.unlimited_gain && !a.unlimited_loss);
        assert!(close(a.breakevens[0], 38.0));
    }

    #[test]
    fn naked_short_call_has_unlimited_loss() {
        let a = analyze(&[call(Side::Short, 40.0, 2.0, 100.0)], None).unwrap();
        assert!(a.unlimited_loss && a.max_loss.is_none());
        assert!(close(a.max_gain.unwrap(), 200.0));
        assert!(close(a.breakevens[0], 42.0));
    }

    #[test]
    fn a_stock_only_position_breaks_even_at_its_entry() {
        let a = analyze(&[stock(Side::Long, 30.0, 10.0)], Some(30.0)).unwrap();
        assert!(a.unlimited_gain);
        assert!(close(a.max_loss.unwrap(), -300.0));
        assert!(close(a.breakevens[0], 30.0));
        assert!(close(a.net_debit, 0.0));
    }

    #[test]
    fn a_zero_result_everywhere_has_no_breakeven() {
        // Compra e venda da mesma call ao mesmo preço: resultado zero em todo S.
        let a = analyze(&[call(Side::Long, 40.0, 2.0, 100.0), call(Side::Short, 40.0, 2.0, 100.0)], Some(40.0)).unwrap();
        assert!(a.breakevens.is_empty());
        assert_eq!((a.max_gain, a.max_loss), (Some(0.0), Some(0.0)));
        assert!(a.points.iter().all(|p| p.pnl.abs() < 1e-9));
    }

    #[test]
    fn the_curve_covers_the_strikes_and_breakevens_and_is_sorted() {
        let a = analyze(&[call(Side::Long, 40.0, 3.0, 100.0), call(Side::Short, 45.0, 1.0, 100.0)], Some(41.0)).unwrap();
        let prices: Vec<f64> = a.points.iter().map(|p| p.price).collect();
        assert!(prices.windows(2).all(|w| w[0] < w[1]));
        for must in [40.0, 42.0, 45.0] {
            assert!(prices.iter().any(|p| close(*p, must)), "{must}");
        }
        assert!(prices[0] < 40.0 && *prices.last().unwrap() > 45.0);
        // No strike de 45 em diante o ganho é o máximo.
        assert!(close(a.points.last().unwrap().pnl, 300.0));
    }

    #[test]
    fn invalid_legs_are_rejected() {
        assert!(analyze(&[], None).is_err());
        assert!(analyze(&[call(Side::Long, 40.0, 2.0, 0.0)], None).is_err());
        assert!(analyze(&[call(Side::Long, 40.0, -1.0, 100.0)], None).is_err());
        assert!(analyze(&[call(Side::Long, 0.0, 1.0, 100.0)], None).is_err());
        // Opção sem strike.
        assert!(analyze(&[Leg { kind: LegKind::Call, side: Side::Long, strike: None, price: 1.0, quantity: 1.0 }], None).is_err());
        assert!(analyze(&[Leg { kind: LegKind::Stock, side: Side::Long, strike: Some(10.0), price: 1.0, quantity: 1.0 }], None).is_err());
        assert!(analyze(&[call(Side::Long, 40.0, f64::NAN, 100.0)], None).is_err());
        let nine = vec![call(Side::Long, 40.0, 1.0, 1.0); MAX_LEGS + 1];
        assert!(analyze(&nine, None).is_err());
    }
}
