// IR de cripto (ganho de capital, exchange nacional, pessoa física). Separado da apuração da B3 porque o DARF tem
// código próprio (4600), o mínimo e o saldo acumulado são próprios e nada aqui compensa prejuízo nem tem IRRF.
//
// Regras (valores em `tax_rules`, editáveis):
// - Vendas do mês somando até o limite (R$ 35 mil, todas as moedas juntas) → ganho 100% isento. Acima dele, o ganho
//   **do mês inteiro** é tributado (tudo ou nada).
// - Alíquota única de 15%. A tabela real sobe acima de R$ 5 milhões de ganho no ano (17,5%, 20%, 22,5%); isso só avisa.
// - Prejuízo do mês dá imposto zero e não compensa nem carrega.
// - Custo médio por ativo. Venda em moeda que não é BRL (exchange estrangeira) segue outra regra e só avisa.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Datelike, NaiveDate};
use serde::Serialize;

use super::assessment::{Side, TaxWarning, Trade};
use super::calendar::darf_due_date;
use super::classify::{treatment, TaxTreatment};
use super::rules::{self, RuleSet};
use crate::domain::cost_basis::{take_from_position, Position, EPS};
use crate::error::AppError;

/// Uma venda de cripto, com o custo médio já baixado.
#[derive(Clone, Debug, Serialize)]
pub struct CryptoSale {
    pub tx_id: i32,
    pub portfolio_id: i32,
    pub asset_id: i32,
    pub ticker: String,
    pub date: NaiveDate,
    pub quantity: f64,
    /// Valor de venda bruto (base do limite de isenção).
    pub sale_value: f64,
    /// Valor de venda − taxa.
    pub proceeds: f64,
    pub cost: f64,
    pub gain: f64,
    /// Vendeu mais do que o histórico de compras conhecido: o ganho só considera a parte com custo conhecido.
    pub cost_unknown: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CryptoMonth {
    /// `YYYY-MM`
    pub period: String,
    /// Total vendido no mês (base do limite de isenção).
    pub sales_total: f64,
    pub exempt: bool,
    /// Ganho do mês (ganhos − perdas).
    pub gain: f64,
    pub rate: f64,
    pub tax: f64,
    /// Imposto abaixo do mínimo de DARF vindo de meses anteriores.
    pub carried_in: f64,
    /// Valor do DARF do mês (0 se ficou abaixo do mínimo).
    pub due: f64,
    pub carried_out: f64,
    pub due_date: NaiveDate,
    pub darf_code: i32,
    pub sales: Vec<CryptoSale>,
}

fn period_of(date: NaiveDate) -> String {
    format!("{:04}-{:02}", date.year(), date.month())
}

fn first_day_of(period: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d")
        .map_err(|_| AppError::InvalidGuard(format!("invalid period {period}")))
}

/// Apura mês a mês o IR das vendas de cripto em BRL. `trades` pode vir em qualquer ordem e misturar outras classes
/// (são ignoradas). Só consulta as regras de cripto quando há venda de cripto.
pub fn assess_crypto(trades: &[Trade], rules: &RuleSet) -> Result<(Vec<CryptoMonth>, Vec<TaxWarning>), AppError> {
    let mut warnings = Vec::new();
    let mut non_brl_periods: BTreeSet<String> = BTreeSet::new();

    let mut crypto: Vec<&Trade> = Vec::new();
    for t in trades.iter().filter(|t| treatment(&t.asset_class, &t.ticker) == TaxTreatment::Crypto) {
        if t.currency != "BRL" {
            if t.side == Side::Sell {
                non_brl_periods.insert(period_of(t.date));
            }
            continue;
        }
        crypto.push(t);
    }
    // Dentro do dia, compras e eventos antes das vendas, como na apuração da B3.
    crypto.sort_by_key(|t| (t.asset_id, t.date, t.side == Side::Sell, t.tx_id));

    let mut by_month: BTreeMap<String, Vec<CryptoSale>> = BTreeMap::new();
    let mut positions: HashMap<i32, Position> = HashMap::new();
    for t in crypto {
        let pos = positions.entry(t.asset_id).or_default();
        match t.side {
            Side::Buy => {
                pos.quantity += t.quantity;
                pos.cost += t.gross_value + t.fee;
            }
            Side::Split => pos.quantity *= t.quantity,
            Side::Bonus => {
                pos.quantity += t.quantity;
                pos.cost += t.gross_value;
            }
            Side::Sell => {
                let (cost, covered) = take_from_position(pos, t.quantity);
                let cost_unknown = covered + EPS < t.quantity;
                let share = if t.quantity > EPS { covered / t.quantity } else { 0.0 };
                if cost_unknown {
                    warnings.push(TaxWarning::CostUnknown { ticker: t.ticker.clone(), date: t.date });
                }
                let proceeds = (t.gross_value - t.fee) * share;
                by_month.entry(period_of(t.date)).or_default().push(CryptoSale {
                    tx_id: t.tx_id,
                    portfolio_id: t.portfolio_id,
                    asset_id: t.asset_id,
                    ticker: t.ticker.clone(),
                    date: t.date,
                    quantity: covered,
                    sale_value: t.gross_value * share,
                    proceeds,
                    cost,
                    gain: proceeds - cost,
                    cost_unknown,
                });
            }
        }
    }

    let mut months = Vec::new();
    let mut carried = 0.0;
    let mut taxable_by_year: BTreeMap<i32, f64> = BTreeMap::new();
    let mut above_first_tier: BTreeSet<i32> = BTreeSet::new();
    for (period, sales) in by_month {
        let first_day = first_day_of(&period)?;
        let limit = rules.get(rules::CRYPTO_EXEMPTION_MONTHLY_SALES, first_day)?;
        let rate = rules.get(rules::CRYPTO_RATE, first_day)?;
        let sales_total: f64 = sales.iter().map(|s| s.sale_value).sum();
        let gain: f64 = sales.iter().map(|s| s.gain).sum();
        let exempt = sales_total <= limit + EPS;
        let taxable = if exempt { 0.0 } else { gain.max(0.0) };
        let tax = taxable * rate;

        let year_total = taxable_by_year.entry(first_day.year()).or_insert(0.0);
        *year_total += taxable;
        if taxable > 0.0 && *year_total > rules.get(rules::CRYPTO_FIRST_TIER_GAIN_LIMIT, first_day)? + EPS {
            above_first_tier.insert(first_day.year());
        }

        let minimum = rules.get(rules::DARF_MINIMUM, first_day)?;
        let carried_in = carried;
        let payable = tax + carried_in;
        let (due, carried_out) = if payable + EPS < minimum { (0.0, payable) } else { (payable, 0.0) };
        carried = carried_out;

        months.push(CryptoMonth {
            period,
            sales_total,
            exempt,
            gain,
            rate,
            tax,
            carried_in,
            due,
            carried_out,
            due_date: darf_due_date(first_day.year(), first_day.month()),
            darf_code: rules.get(rules::DARF_CODE_CRYPTO, first_day)? as i32,
            sales,
        });
    }

    for period in non_brl_periods {
        warnings.push(TaxWarning::CryptoSalesNonBrl { period });
    }
    for year in above_first_tier {
        warnings.push(TaxWarning::CryptoGainAboveFirstTier { year });
    }
    Ok((months, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax::rules::test_rule_set;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn trade(id: i32, asset: i32, ticker: &str, date: &str, side: Side, quantity: f64, gross: f64) -> Trade {
        Trade {
            tx_id: id,
            portfolio_id: 1,
            asset_id: asset,
            ticker: ticker.to_string(),
            asset_class: "cripto".to_string(),
            currency: "BRL".to_string(),
            custodia_id: None,
            date: d(date),
            side,
            quantity,
            gross_value: gross,
            fee: 0.0,
        }
    }

    fn run(trades: &[Trade]) -> (Vec<CryptoMonth>, Vec<TaxWarning>) {
        assess_crypto(trades, &test_rule_set()).unwrap()
    }

    #[test]
    fn monthly_sales_up_to_the_limit_are_exempt_even_with_a_gain() {
        let (months, warnings) = run(&[
            trade(1, 1, "BTC", "2026-01-02", Side::Buy, 1.0, 10_000.0),
            trade(2, 1, "BTC", "2026-02-10", Side::Sell, 1.0, 35_000.0),
        ]);
        assert!(months[0].exempt && close(months[0].tax, 0.0) && close(months[0].due, 0.0));
        assert!(close(months[0].gain, 25_000.0) && close(months[0].sales_total, 35_000.0), "o limite exato é isento");
        assert!(warnings.is_empty());
    }

    #[test]
    fn above_the_limit_the_whole_months_gain_is_taxed_at_15_percent() {
        let (months, _) = run(&[
            trade(1, 1, "BTC", "2026-01-02", Side::Buy, 1.0, 10_000.0),
            trade(2, 1, "BTC", "2026-02-10", Side::Sell, 1.0, 40_000.0),
        ]);
        let feb = &months[0];
        assert!(!feb.exempt && close(feb.gain, 30_000.0));
        assert!(close(feb.tax, 4_500.0) && close(feb.due, 4_500.0), "{}", feb.tax);
        assert_eq!(feb.darf_code, 4600);
        assert_eq!(feb.due_date, d("2026-03-31"));
    }

    #[test]
    fn the_limit_adds_up_every_coin_sold_in_the_month() {
        let (months, _) = run(&[
            trade(1, 1, "BTC", "2026-01-02", Side::Buy, 1.0, 10_000.0),
            trade(2, 2, "ETH", "2026-01-02", Side::Buy, 10.0, 10_000.0),
            trade(3, 1, "BTC", "2026-02-10", Side::Sell, 1.0, 20_000.0),
            trade(4, 2, "ETH", "2026-02-12", Side::Sell, 10.0, 20_000.0),
        ]);
        assert!(close(months[0].sales_total, 40_000.0) && !months[0].exempt);
        assert!(close(months[0].tax, 0.15 * 20_000.0));
    }

    #[test]
    fn a_losing_month_owes_nothing_and_the_loss_is_not_carried() {
        let (months, _) = run(&[
            trade(1, 1, "BTC", "2026-01-02", Side::Buy, 2.0, 100_000.0),
            trade(2, 1, "BTC", "2026-02-10", Side::Sell, 1.0, 40_000.0),
            trade(3, 1, "BTC", "2026-03-10", Side::Sell, 1.0, 60_000.0),
        ]);
        // Fev: vende 1 por 40 mil com custo 50 mil → perda de 10 mil. Mar: ganho de 10 mil, tributado inteiro (sem compensar).
        assert!(close(months[0].gain, -10_000.0) && close(months[0].tax, 0.0));
        assert!(close(months[1].gain, 10_000.0) && close(months[1].tax, 1_500.0), "{}", months[1].tax);
    }

    #[test]
    fn the_cost_is_the_average_per_asset() {
        let (months, _) = run(&[
            trade(1, 1, "BTC", "2026-01-02", Side::Buy, 1.0, 10_000.0),
            trade(2, 1, "BTC", "2026-01-20", Side::Buy, 1.0, 30_000.0),
            trade(3, 1, "BTC", "2026-02-10", Side::Sell, 1.0, 50_000.0),
        ]);
        assert!(close(months[0].sales[0].cost, 20_000.0) && close(months[0].gain, 30_000.0));
    }

    #[test]
    fn tax_below_the_darf_minimum_accumulates_to_the_next_month() {
        // Fev: ganho de 40 → imposto de 6 (abaixo de R$ 10); Mar: mais 6 → DARF de 12.
        let (months, _) = run(&[
            trade(1, 1, "SHIB", "2026-01-02", Side::Buy, 2.0, 80_000.0),
            trade(2, 1, "SHIB", "2026-02-10", Side::Sell, 1.0, 40_040.0),
            trade(3, 1, "SHIB", "2026-03-10", Side::Sell, 1.0, 40_040.0),
        ]);
        assert!(close(months[0].gain, 40.0) && close(months[0].due, 0.0) && close(months[0].carried_out, 6.0));
        assert!(close(months[1].due, 12.0) && close(months[1].carried_out, 0.0), "{}", months[1].due);
    }

    #[test]
    fn a_sale_in_a_foreign_currency_only_warns_and_a_sale_above_known_cost_flags_it() {
        let mut foreign = trade(2, 1, "BTC", "2026-02-10", Side::Sell, 1.0, 50_000.0);
        foreign.currency = "USD".to_string();
        let (months, warnings) = run(&[trade(1, 1, "BTC", "2026-01-02", Side::Buy, 1.0, 10_000.0), foreign]);
        assert!(months.is_empty());
        assert!(warnings.iter().any(|w| matches!(w, TaxWarning::CryptoSalesNonBrl { period } if period == "2026-02")));

        let (months, warnings) = run(&[
            trade(1, 1, "BTC", "2026-01-02", Side::Buy, 1.0, 10_000.0),
            trade(2, 1, "BTC", "2026-02-10", Side::Sell, 2.0, 80_000.0),
        ]);
        assert!(months[0].sales[0].cost_unknown && close(months[0].sales[0].quantity, 1.0));
        assert!(warnings.iter().any(|w| matches!(w, TaxWarning::CostUnknown { .. })));
    }

    #[test]
    fn a_taxable_gain_above_the_first_tier_in_a_year_warns() {
        let (_, warnings) = run(&[
            trade(1, 1, "BTC", "2026-01-02", Side::Buy, 1.0, 1_000_000.0),
            trade(2, 1, "BTC", "2026-02-10", Side::Sell, 1.0, 6_500_000.0),
        ]);
        assert!(warnings.iter().any(|w| matches!(w, TaxWarning::CryptoGainAboveFirstTier { year: 2026 })));
    }

    #[test]
    fn other_classes_and_a_portfolio_without_crypto_need_no_crypto_rules() {
        let mut stock = trade(1, 1, "PETR4", "2026-01-02", Side::Buy, 1.0, 10.0);
        stock.asset_class = "acao_br".to_string();
        let empty = RuleSet::new(vec![]);
        let (months, warnings) = assess_crypto(&[stock], &empty).unwrap();
        assert!(months.is_empty() && warnings.is_empty());
    }
}
