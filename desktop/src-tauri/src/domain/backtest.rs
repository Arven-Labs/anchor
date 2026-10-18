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
// - **Ordens**: o fluxo do dia (aporte + rebalanceamento) vira uma ordem líquida por ativo, e cada ordem paga a corretagem
//   fixa na hora. Corretagem e imposto são custo, não aporte: entram no TWR.
// - **Imposto** (opcional): as ordens viram `Trade`s para o motor de IR de renda variável, que apura mês a mês; o DARF do mês
//   é debitado do patrimônio no primeiro pregão do mês seguinte. O do último mês fica como "a pagar".
// Rendimento passado não garante nada, a tela diz isso.
use chrono::{Datelike, NaiveDate};

use crate::domain::tax::assessment::{assess_with_options, Side, Trade};
use crate::domain::tax::rules::RuleSet;

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

/// Regras e classe de cada ativo para apurar o IR das vendas.
pub struct TaxModel {
    pub rules: RuleSet,
    /// `(classe, ticker)` na mesma ordem das séries.
    pub assets: Vec<(String, String)>,
}

pub struct Params {
    /// Peso-alvo de cada ativo, na mesma ordem das séries; são normalizados para somar 1.
    pub weights: Vec<f64>,
    pub rebalance: Rebalance,
    pub initial: f64,
    pub monthly_contribution: f64,
    /// Taxa livre de risco anual (fração, 0,10 = 10%), para o Sharpe.
    pub risk_free_annual: f64,
    /// Corretagem fixa, em R$, por ordem executada.
    pub brokerage_per_order: f64,
    pub tax: Option<TaxModel>,
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
    pub total_fees: f64,
    /// IR já debitado do patrimônio.
    pub total_tax_paid: f64,
    /// IR do mês final (e do que ficou abaixo do mínimo de DARF), devido mas não debitado.
    pub tax_pending: f64,
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

/// Ordem líquida abaixo disto (em R$) é arredondamento, não negócio: não paga corretagem.
const MIN_ORDER_VALUE: f64 = 0.005;

fn period_of(date: NaiveDate) -> String {
    format!("{:04}-{:02}", date.year(), date.month())
}

/// Ordens, corretagem e imposto acumulados da simulação.
struct Ledger<'a> {
    brokerage: f64,
    tax: Option<&'a TaxModel>,
    /// Só preenchido quando há imposto a apurar.
    trades: Vec<Trade>,
    fees: f64,
    tax_paid: f64,
    /// Último mês (`YYYY-MM`) cujo DARF já foi debitado.
    paid_through: String,
}

impl<'a> Ledger<'a> {
    /// Compara as unidades antes e depois dos fluxos do dia, registra uma ordem líquida por ativo
    /// e devolve a corretagem do dia.
    fn settle(&mut self, date: NaiveDate, before: &[f64], after: &[f64], prices: &[f64]) -> f64 {
        let mut fee_today = 0.0;
        for (asset, ((b, a), price)) in before.iter().zip(after).zip(prices).enumerate() {
            let delta = a - b;
            let gross = delta.abs() * price;
            if gross < MIN_ORDER_VALUE {
                continue;
            }
            fee_today += self.brokerage;
            if let Some(model) = self.tax {
                let id = self.trades.len() as i32;
                let (class, ticker) = &model.assets[asset];
                self.trades.push(Trade {
                    tx_id: id,
                    portfolio_id: 0,
                    asset_id: asset as i32,
                    ticker: ticker.clone(),
                    asset_class: class.clone(),
                    currency: "BRL".to_string(),
                    // Uma "corretora" por ordem: compra e venda do mesmo dia nunca casam como day trade.
                    custodia_id: Some(id),
                    date,
                    side: if delta > 0.0 { Side::Buy } else { Side::Sell },
                    quantity: delta.abs(),
                    gross_value: gross,
                    fee: self.brokerage,
                });
            }
        }
        self.fees += fee_today;
        fee_today
    }

    /// DARF dos meses até `last_period` (inclusive) que ainda não foi debitado; marca como pago.
    fn take_due(&mut self, last_period: &str) -> Result<f64, String> {
        let Some(model) = self.tax else { return Ok(0.0) };
        let assessment = assess_with_options(&self.trades, &[], &model.rules).map_err(|e| e.to_string())?;
        let due = assessment
            .months
            .iter()
            .filter(|m| m.period.as_str() > self.paid_through.as_str() && m.period.as_str() <= last_period)
            .map(|m| m.due)
            .sum();
        self.paid_through = last_period.to_string();
        Ok(due)
    }

    /// IR devido no fim da janela e ainda não debitado (inclui o que ficou abaixo do mínimo de DARF).
    fn pending(&self) -> Result<f64, String> {
        let Some(model) = self.tax else { return Ok(0.0) };
        let assessment = assess_with_options(&self.trades, &[], &model.rules).map_err(|e| e.to_string())?;
        let unpaid: Vec<_> = assessment.months.iter().filter(|m| m.period.as_str() > self.paid_through.as_str()).collect();
        Ok(unpaid.iter().map(|m| m.due).sum::<f64>() + unpaid.last().map_or(0.0, |m| m.carried_out))
    }
}

/// Debita `amount` do patrimônio, na proporção dos ativos, e o desconta do índice (custo é retorno perdido).
/// Devolve o que foi de fato debitado (nunca mais que o patrimônio).
fn charge(amount: f64, value: &mut f64, index: &mut f64, units: &mut [f64]) -> f64 {
    let amount = amount.min(*value);
    if amount <= 0.0 {
        return 0.0;
    }
    let factor = (*value - amount) / *value;
    units.iter_mut().for_each(|u| *u *= factor);
    *value -= amount;
    *index *= factor;
    amount
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
    if !params.brokerage_per_order.is_finite() || params.brokerage_per_order < 0.0 {
        return Err("a corretagem por ordem não pode ser negativa".to_string());
    }
    if params.tax.as_ref().is_some_and(|t| t.assets.len() != series.len()) {
        return Err("é preciso a classe e o ticker de cada ativo para apurar o imposto".to_string());
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
    let mut ledger = Ledger {
        brokerage: params.brokerage_per_order,
        tax: params.tax.as_ref(),
        trades: Vec::new(),
        fees: 0.0,
        tax_paid: 0.0,
        paid_through: String::new(),
    };
    let initial_fee = ledger.settle(dates[0], &vec![0.0; series.len()], &units, &first_prices);
    charge(initial_fee, &mut value, &mut index, &mut units);
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

        let new_month = (date.year(), date.month()) != (previous_date.year(), previous_date.month());
        // O DARF do mês que acabou sai do patrimônio antes dos fluxos do dia.
        if new_month {
            let due = ledger.take_due(&period_of(previous_date))?;
            ledger.tax_paid += charge(due, &mut value, &mut index, &mut units);
        }
        // Aporte no primeiro dia de um mês novo; o rebalanceamento vem depois, no mesmo dia.
        let units_before_flows = units.clone();
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
        let fee_today = ledger.settle(date, &units_before_flows, &units, &prices);
        charge(fee_today, &mut value, &mut index, &mut units);

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
            total_fees: ledger.fees,
            total_tax_paid: ledger.tax_paid,
            tax_pending: ledger.pending()?,
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
        Params {
            weights: weights.to_vec(),
            rebalance: Rebalance::Never,
            initial: 1000.0,
            monthly_contribution: 0.0,
            risk_free_annual: 0.0,
            brokerage_per_order: 0.0,
            tax: None,
        }
    }

    /// A dobra de preço em 1/fev: 50/50 que rebalanceia em `initial` de capital, 70 dias (até 11/mar), imposto ligado.
    fn doubling_with_tax(class: &str, ticker: &str, initial: f64, days: usize) -> Result<BacktestResult, String> {
        let a: Vec<f64> = (0..days).map(|i| if i < 31 { 100.0 } else { 200.0 }).collect();
        let b = vec![100.0; days];
        let mut p = params(&[0.5, 0.5]);
        p.rebalance = Rebalance::Monthly;
        p.initial = initial;
        p.tax = Some(TaxModel {
            rules: crate::domain::tax::rules::test_rule_set(),
            assets: vec![(class.to_string(), ticker.to_string()), ("acao_br".to_string(), "BBBB3".to_string())],
        });
        run(&[series("A", 0, &a), series("B", 0, &b)], &p)
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
    fn a_fixed_brokerage_per_order_comes_out_of_the_value_and_the_return() {
        let mut p = params(&[1.0]);
        p.brokerage_per_order = 10.0;
        let result = run(&[series("A", 0, &[100.0, 100.0, 100.0])], &p).unwrap();
        // Uma ordem de compra no início: 1000 viram 990 e o custo é retorno perdido.
        assert!(close(result.metrics.final_value, 990.0) && close(result.metrics.total_fees, 10.0));
        assert!(close(result.metrics.twr_pct, -1.0), "{}", result.metrics.twr_pct);
        assert!(close(result.metrics.total_invested, 1000.0));
    }

    #[test]
    fn contribution_and_rebalance_of_the_same_day_are_one_net_order_per_asset() {
        // 50/50 mensal com aporte: no 1º dia de fev há aporte e rebalanceamento, mas só um negócio por ativo.
        let a: Vec<f64> = (0..40).map(|i| if i < 31 { 100.0 } else { 200.0 }).collect();
        let b = vec![100.0; 40];
        let mut p = params(&[0.5, 0.5]);
        p.rebalance = Rebalance::Monthly;
        p.monthly_contribution = 1000.0;
        p.brokerage_per_order = 1.0;
        let result = run(&[series("A", 0, &a), series("B", 0, &b)], &p).unwrap();
        // 2 ordens na compra inicial + 2 em 1/fev (as duas de compra: o aporte cobre o rebalanceamento).
        assert!(close(result.metrics.total_fees, 4.0), "{}", result.metrics.total_fees);
    }

    #[test]
    fn rebalancing_a_gain_above_the_exemption_pays_the_darf_the_next_month() {
        let result = doubling_with_tax("acao_br", "AAAA3", 100_000.0, 70).unwrap();
        // 1/fev: A vale 100 mil e B 50 mil; volta a 75/75 vendendo 125 cotas de A a 200 (25 mil, acima dos 20 mil).
        // Ganho de 125 × 100 = 12.500; 15% = 1.875, menos o IRRF de 0,005% da venda (1,25) = 1.873,75, debitados em 1/mar.
        assert!(close(result.metrics.total_tax_paid, 1873.75), "{}", result.metrics.total_tax_paid);
        assert!(close(result.metrics.tax_pending, 0.0));
        assert!(close(result.metrics.final_value, 150_000.0 - 1873.75), "{}", result.metrics.final_value);
    }

    #[test]
    fn the_tax_of_the_last_month_is_pending_not_paid() {
        let result = doubling_with_tax("acao_br", "AAAA3", 100_000.0, 40).unwrap();
        assert!(close(result.metrics.total_tax_paid, 0.0));
        assert!(close(result.metrics.tax_pending, 1873.75), "{}", result.metrics.tax_pending);
        assert!(close(result.metrics.final_value, 150_000.0));
    }

    #[test]
    fn stock_sales_under_the_monthly_limit_are_exempt() {
        // Capital de 40 mil: a venda de 1/fev é de 10 mil, abaixo dos 20 mil.
        let result = doubling_with_tax("acao_br", "AAAA3", 40_000.0, 70).unwrap();
        assert!(close(result.metrics.total_tax_paid, 0.0) && close(result.metrics.tax_pending, 0.0));
    }

    #[test]
    fn a_fii_is_taxed_at_its_own_rate_with_no_exemption() {
        // Mesma venda de 25 mil (ganho 12.500), mas FII: 20% = 2.500, menos o IRRF de 1,25.
        let result = doubling_with_tax("fii", "HGLG11", 100_000.0, 70).unwrap();
        assert!(close(result.metrics.total_tax_paid, 2498.75),"{}", result.metrics.total_tax_paid);
        let small = doubling_with_tax("fii", "HGLG11", 40_000.0, 70).unwrap();
        assert!(small.metrics.total_tax_paid > 0.0, "FII não tem a isenção das ações");
    }

    #[test]
    fn without_tax_or_brokerage_nothing_is_charged() {
        let a: Vec<f64> = (0..70).map(|i| if i < 31 { 100.0 } else { 200.0 }).collect();
        let b = vec![100.0; 70];
        let mut p = params(&[0.5, 0.5]);
        p.rebalance = Rebalance::Monthly;
        let result = run(&[series("A", 0, &a), series("B", 0, &b)], &p).unwrap();
        assert!(close(result.metrics.total_fees, 0.0) && close(result.metrics.total_tax_paid, 0.0) && close(result.metrics.tax_pending, 0.0));
        assert!(close(result.metrics.final_value, 1500.0), "{}", result.metrics.final_value);
    }

    #[test]
    fn a_negative_brokerage_or_a_tax_model_out_of_step_is_refused() {
        let a = series("A", 0, &[100.0, 101.0]);
        let mut p = params(&[1.0]);
        p.brokerage_per_order = -1.0;
        assert!(run(&[a.clone()], &p).unwrap_err().contains("corretagem"));
        let mut q = params(&[1.0]);
        q.tax = Some(TaxModel { rules: crate::domain::tax::rules::test_rule_set(), assets: vec![] });
        assert!(run(&[a], &q).unwrap_err().contains("classe e o ticker"));
    }

    #[test]
    fn the_rebalance_names_parse() {
        assert_eq!(Rebalance::parse("quarterly"), Some(Rebalance::Quarterly));
        assert_eq!(Rebalance::parse("daily"), None);
    }
}
