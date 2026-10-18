// Fase 22, fatia 2 — motor puro do backtest: simula uma carteira de pesos-alvo sobre séries diárias de fechamento
// ajustado (retorno total), com rebalanceamento periódico e aporte mensal, e mede o resultado. Sem banco nem rede:
// quem chama (`commands/backtest.rs`) carrega as séries e o CDI.
//
// Convenções:
// - **Janela comum**: do primeiro dia em que todos os ativos têm preço até o último dia em que todos ainda têm.
//   O calendário é a união das datas dentro da janela; o dia sem pregão de um ativo repete o último preço.
// - **Fluxos no fim do dia**: o retorno do dia é o do valor da carteira de ontem com os preços de hoje; só depois
//   entram o aporte e o rebalanceamento. Assim o aporte não vira "retorno" e a série de retornos (TWR) é limpa.
// - **Aporte** vai pelos pesos-alvo, no primeiro dia de cada mês depois do mês de início. **Rebalancear** volta aos
//   pesos-alvo no primeiro dia do mês, trimestre ou ano seguinte ao do início, depois do aporte do mesmo dia.
// - Sem custos nem imposto (fatia 4). Rendimento passado não garante nada, a tela diz isso.
use chrono::{Datelike, NaiveDate};

/// Dias de pregão por ano, para anualizar a volatilidade.
const TRADING_DAYS_PER_YEAR: f64 = 252.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rebalance {
    Never,
    Monthly,
    Quarterly,
    Yearly,
}

impl Rebalance {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "never" => Some(Self::Never),
            "monthly" => Some(Self::Monthly),
            "quarterly" => Some(Self::Quarterly),
            "yearly" => Some(Self::Yearly),
            _ => None,
        }
    }
}

/// Fechamento ajustado diário de um ativo, em ordem crescente de data.
#[derive(Clone, Debug)]
pub struct AssetSeries {
    pub name: String,
    pub points: Vec<(NaiveDate, f64)>,
}

#[derive(Clone, Debug)]
pub struct Params {
    /// Peso-alvo de cada ativo, na mesma ordem das séries; são normalizados para somar 1.
    pub weights: Vec<f64>,
    pub rebalance: Rebalance,
    pub initial: f64,
    pub monthly_contribution: f64,
    /// Taxa livre de risco anual (fração, 0,10 = 10%), para o Sharpe.
    pub risk_free_annual: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CurvePoint {
    pub date: NaiveDate,
    /// Patrimônio no fim do dia.
    pub value: f64,
    /// Total aportado até o dia (inicial + aportes).
    pub invested: f64,
    /// Índice de retorno ponderado pelo tempo, base 1,0 (sem o efeito dos aportes).
    pub index: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub total_invested: f64,
    pub final_value: f64,
    /// Retorno ponderado pelo tempo (TWR) no período, em %.
    pub twr_pct: f64,
    /// TWR anualizado, em %.
    pub cagr_pct: f64,
    pub volatility_pct: f64,
    pub max_drawdown_pct: f64,
    pub max_drawdown_date: Option<NaiveDate>,
    /// `None` com volatilidade zero.
    pub sharpe: Option<f64>,
    pub rebalance_count: u32,
}

#[derive(Clone, Debug)]
pub struct BacktestResult {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub names: Vec<String>,
    pub curve: Vec<CurvePoint>,
    pub metrics: Metrics,
    /// Correlação de Pearson dos retornos diários, na ordem de `names`.
    pub correlation: Vec<Vec<f64>>,
    /// Pesos no último dia (mostram a deriva de quem não rebalanceia).
    pub final_weights: Vec<f64>,
}

fn period_key(date: NaiveDate, rebalance: Rebalance) -> (i32, u32) {
    match rebalance {
        Rebalance::Never | Rebalance::Yearly => (date.year(), 0),
        Rebalance::Monthly => (date.year(), date.month()),
        Rebalance::Quarterly => (date.year(), (date.month() - 1) / 3),
    }
}

fn prices_on(series: &[AssetSeries], cursors: &mut [usize], date: NaiveDate) -> Vec<f64> {
    series
        .iter()
        .zip(cursors.iter_mut())
        .map(|(s, cursor)| {
            while *cursor + 1 < s.points.len() && s.points[*cursor + 1].0 <= date {
                *cursor += 1;
            }
            s.points[*cursor].1
        })
        .collect()
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    (values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64).sqrt()
}

fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let (sa, sb) = (std_dev(a), std_dev(b));
    if sa == 0.0 || sb == 0.0 {
        return 0.0;
    }
    let (ma, mb) = (mean(a), mean(b));
    let covariance = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum::<f64>() / (a.len() - 1) as f64;
    covariance / (sa * sb)
}

pub fn run(series: &[AssetSeries], params: &Params) -> Result<BacktestResult, String> {
    if series.is_empty() {
        return Err("escolha pelo menos um ativo".to_string());
    }
    if params.weights.len() != series.len() {
        return Err("é preciso um peso por ativo".to_string());
    }
    if params.weights.iter().any(|w| !w.is_finite() || *w < 0.0) || params.weights.iter().sum::<f64>() <= 0.0 {
        return Err("os pesos têm de ser positivos e somar mais que zero".to_string());
    }
    if !params.initial.is_finite() || params.initial <= 0.0 || !params.monthly_contribution.is_finite() || params.monthly_contribution < 0.0 {
        return Err("o capital inicial tem de ser positivo e o aporte não pode ser negativo".to_string());
    }
    if let Some(empty) = series.iter().find(|s| s.points.is_empty() || s.points.iter().any(|(_, p)| !p.is_finite() || *p <= 0.0)) {
        return Err(format!("{}: sem preços válidos", empty.name));
    }

    let total_weight: f64 = params.weights.iter().sum();
    let weights: Vec<f64> = params.weights.iter().map(|w| w / total_weight).collect();

    let start = series.iter().map(|s| s.points[0].0).max().expect("séries não vazias");
    let end = series.iter().map(|s| s.points[s.points.len() - 1].0).min().expect("séries não vazias");
    if start >= end {
        return Err("os ativos não têm período em comum com pelo menos dois dias de preço".to_string());
    }

    let mut dates: Vec<NaiveDate> = series.iter().flat_map(|s| s.points.iter().map(|(d, _)| *d)).filter(|d| *d >= start && *d <= end).collect();
    dates.sort();
    dates.dedup();

    let mut cursors = vec![0usize; series.len()];
    let first_prices = prices_on(series, &mut cursors, dates[0]);
    let mut units: Vec<f64> = weights.iter().zip(&first_prices).map(|(w, p)| params.initial * w / p).collect();
    let mut value = params.initial;
    let mut invested = params.initial;
    let mut index = 1.0;
    let mut peak = 1.0;
    let mut max_drawdown = 0.0_f64;
    let mut max_drawdown_date = None;
    let mut rebalance_count = 0u32;
    let mut returns: Vec<f64> = Vec::new();
    let mut asset_returns: Vec<Vec<f64>> = vec![Vec::new(); series.len()];
    let mut previous_prices = first_prices;
    let mut curve = vec![CurvePoint { date: dates[0], value, invested, index }];

    for pair in dates.windows(2) {
        let (previous_date, date) = (pair[0], pair[1]);
        let prices = prices_on(series, &mut cursors, date);
        for (asset, (price, previous)) in prices.iter().zip(&previous_prices).enumerate() {
            asset_returns[asset].push(price / previous - 1.0);
        }
        let before_flows: f64 = units.iter().zip(&prices).map(|(u, p)| u * p).sum();
        let daily_return = before_flows / value - 1.0;
        returns.push(daily_return);
        index *= 1.0 + daily_return;
        value = before_flows;

        // Aporte no primeiro dia de um mês novo; o rebalanceamento vem depois, no mesmo dia.
        let new_month = (date.year(), date.month()) != (previous_date.year(), previous_date.month());
        if new_month && params.monthly_contribution > 0.0 {
            for (unit, (w, p)) in units.iter_mut().zip(weights.iter().zip(&prices)) {
                *unit += params.monthly_contribution * w / p;
            }
            value += params.monthly_contribution;
            invested += params.monthly_contribution;
        }
        if params.rebalance != Rebalance::Never && period_key(date, params.rebalance) != period_key(previous_date, params.rebalance) {
            for (unit, (w, p)) in units.iter_mut().zip(weights.iter().zip(&prices)) {
                *unit = value * w / p;
            }
            rebalance_count += 1;
        }

        peak = f64::max(peak, index);
        let drawdown = index / peak - 1.0;
        if drawdown < max_drawdown {
            max_drawdown = drawdown;
            max_drawdown_date = Some(date);
        }
        previous_prices = prices;
        curve.push(CurvePoint { date, value, invested, index });
    }

    let days = (end - start).num_days() as f64;
    let twr = index - 1.0;
    let cagr = if days > 0.0 { index.powf(365.25 / days) - 1.0 } else { 0.0 };
    let volatility = std_dev(&returns) * TRADING_DAYS_PER_YEAR.sqrt();
    let sharpe = (volatility > 0.0).then(|| (cagr - params.risk_free_annual) / volatility);

    let names: Vec<String> = series.iter().map(|s| s.name.clone()).collect();
    let correlation_matrix: Vec<Vec<f64>> = (0..series.len())
        .map(|i| (0..series.len()).map(|j| if i == j { 1.0 } else { correlation(&asset_returns[i], &asset_returns[j]) }).collect())
        .collect();
    let final_values: Vec<f64> = units.iter().zip(&previous_prices).map(|(u, p)| u * p).collect();
    let final_total: f64 = final_values.iter().sum();

    Ok(BacktestResult {
        start,
        end,
        names,
        curve,
        metrics: Metrics {
            total_invested: invested,
            final_value: value,
            twr_pct: twr * 100.0,
            cagr_pct: cagr * 100.0,
            volatility_pct: volatility * 100.0,
            max_drawdown_pct: max_drawdown * 100.0,
            max_drawdown_date,
            sharpe,
            rebalance_count,
        },
        correlation: correlation_matrix,
        final_weights: final_values.iter().map(|v| v / final_total).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(offset: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + chrono::Duration::days(offset)
    }

    fn series(name: &str, from: i64, prices: &[f64]) -> AssetSeries {
        AssetSeries { name: name.to_string(), points: prices.iter().enumerate().map(|(i, p)| (day(from + i as i64), *p)).collect() }
    }

    fn params(weights: &[f64]) -> Params {
        Params { weights: weights.to_vec(), rebalance: Rebalance::Never, initial: 1000.0, monthly_contribution: 0.0, risk_free_annual: 0.0 }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn buy_and_hold_of_one_asset_follows_its_price() {
        let result = run(&[series("A", 0, &[100.0, 105.0, 110.0])], &params(&[1.0])).unwrap();
        assert!(close(result.metrics.final_value, 1100.0) && close(result.metrics.twr_pct, 10.0));
        assert!(close(result.metrics.max_drawdown_pct, 0.0) && result.metrics.max_drawdown_date.is_none());
        assert_eq!(result.curve.len(), 3);
        assert!(close(result.final_weights[0], 1.0));
    }

    #[test]
    fn the_max_drawdown_is_the_worst_fall_from_a_peak_and_has_its_date() {
        let result = run(&[series("A", 0, &[100.0, 120.0, 60.0, 90.0])], &params(&[1.0])).unwrap();
        assert!(close(result.metrics.max_drawdown_pct, -50.0), "{}", result.metrics.max_drawdown_pct);
        assert_eq!(result.metrics.max_drawdown_date, Some(day(2)));
    }

    #[test]
    fn a_monthly_contribution_adds_to_the_invested_total_but_not_to_the_return() {
        // Preço constante por 70 dias: o patrimônio cresce só pelo que foi aportado e o retorno é zero.
        let flat = vec![100.0; 70];
        let mut p = params(&[1.0]);
        p.monthly_contribution = 200.0;
        let result = run(&[series("A", 0, &flat)], &p).unwrap();
        // Janeiro 1 a março 11: aportes em 1/fev e 1/mar.
        assert!(close(result.metrics.total_invested, 1400.0) && close(result.metrics.final_value, 1400.0));
        assert!(close(result.metrics.twr_pct, 0.0));
    }

    #[test]
    fn rebalancing_pulls_the_weights_back_while_buy_and_hold_drifts() {
        // A sobe 1% ao dia, B fica parado, 50/50 desde o começo, 70 dias.
        let a: Vec<f64> = (0..70).map(|i| 100.0 * 1.01_f64.powi(i)).collect();
        let b = vec![100.0; 70];
        let assets = [series("A", 0, &a), series("B", 0, &b)];

        let drift = run(&assets, &params(&[0.5, 0.5])).unwrap();
        assert!(drift.final_weights[0] > 0.6, "sem rebalancear a deriva é grande: {}", drift.final_weights[0]);
        assert_eq!(drift.metrics.rebalance_count, 0);

        let mut monthly = params(&[0.5, 0.5]);
        monthly.rebalance = Rebalance::Monthly;
        let rebalanced = run(&assets, &monthly).unwrap();
        assert_eq!(rebalanced.metrics.rebalance_count, 2, "1/fev e 1/mar");
        assert!(rebalanced.final_weights[0] < drift.final_weights[0]);
        assert!((rebalanced.final_weights[0] - 0.5).abs() < 0.05);
    }

    #[test]
    fn the_window_starts_when_the_last_asset_starts_and_gaps_repeat_the_last_price() {
        let long = series("A", 0, &[100.0, 100.0, 100.0, 100.0, 100.0, 100.0]);
        // B só começa no dia 2 e não tem o dia 4 (repete o preço do dia 3).
        let mut late = series("B", 2, &[10.0, 11.0, 12.0, 13.0]);
        late.points.remove(2);
        let result = run(&[long, late], &params(&[0.5, 0.5])).unwrap();
        assert_eq!(result.start, day(2));
        assert_eq!(result.end, day(5));
        assert_eq!(result.curve.len(), 4);
    }

    #[test]
    fn correlation_is_one_for_proportional_series_and_volatility_zero_has_no_sharpe() {
        let a = [100.0, 102.0, 99.0, 104.0, 101.0];
        let b: Vec<f64> = a.iter().map(|p| p * 2.0).collect();
        let result = run(&[series("A", 0, &a), series("B", 0, &b)], &params(&[0.5, 0.5])).unwrap();
        assert!(close(result.correlation[0][1], 1.0) && close(result.correlation[1][0], 1.0) && close(result.correlation[0][0], 1.0));
        assert!(result.metrics.volatility_pct > 0.0 && result.metrics.sharpe.is_some());

        let flat = run(&[series("A", 0, &[100.0, 100.0, 100.0])], &params(&[1.0])).unwrap();
        assert!(close(flat.metrics.volatility_pct, 0.0) && flat.metrics.sharpe.is_none());
    }

    #[test]
    fn the_sharpe_discounts_the_risk_free_rate() {
        let prices: Vec<f64> = (0..400).map(|i| 100.0 * 1.0005_f64.powi(i) * if i % 2 == 0 { 1.0 } else { 1.002 }).collect();
        let free = run(&[series("A", 0, &prices)], &params(&[1.0])).unwrap();
        let mut with_rate = params(&[1.0]);
        with_rate.risk_free_annual = 0.05;
        let discounted = run(&[series("A", 0, &prices)], &with_rate).unwrap();
        assert!(discounted.metrics.sharpe.unwrap() < free.metrics.sharpe.unwrap());
    }

    #[test]
    fn invalid_inputs_are_refused_with_a_reason() {
        let a = series("A", 0, &[100.0, 101.0]);
        assert!(run(&[], &params(&[])).unwrap_err().contains("pelo menos um"));
        assert!(run(&[a.clone()], &params(&[0.5, 0.5])).unwrap_err().contains("um peso por ativo"));
        assert!(run(&[a.clone()], &params(&[0.0])).unwrap_err().contains("positivos"));
        let mut broke = params(&[1.0]);
        broke.initial = 0.0;
        assert!(run(&[a.clone()], &broke).unwrap_err().contains("capital inicial"));
        let apart = run(&[series("A", 0, &[100.0, 101.0]), series("B", 10, &[5.0, 6.0])], &params(&[0.5, 0.5]));
        assert!(apart.unwrap_err().contains("em comum"));
        assert!(run(&[series("A", 0, &[100.0, -1.0])], &params(&[1.0])).unwrap_err().contains("sem preços válidos"));
    }

    #[test]
    fn the_rebalance_names_parse() {
        assert_eq!(Rebalance::parse("quarterly"), Some(Rebalance::Quarterly));
        assert_eq!(Rebalance::parse("daily"), None);
    }
}
