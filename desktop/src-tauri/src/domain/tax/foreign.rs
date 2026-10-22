// IR de ativos no exterior (Lei 14.754/2023): apuração anual, sem DARF mensal. Separado da B3 e da cripto porque a base é o
// resultado do ano inteiro (ganho de capital + proventos) em reais, com crédito do imposto retido lá fora.
//
// Regras (a alíquota vem de `tax_rules`, o resto é código):
// - Vale para fatos a partir de 2024. Antes disso a venda só gera o aviso `ForeignSales` da B3; as compras de antes de 2024
//   ainda formam o custo médio.
// - Cada operação e cada provento é convertido para BRL pelo câmbio do próprio dia. O custo médio já nasce em reais.
//   A lei (art. 15) manda a PTAX de fechamento para venda; o imposto retido no exterior usa a de compra. Quem chama
//   passa a taxa pronta (`RateToBrl`), PTAX quando houver e câmbio de mercado de reserva; item convertido pelo mercado
//   marca o ano com `ForeignFxApproximate`.
// - Ganhos e perdas do ano se compensam, e os proventos brutos somam ao resultado. Resultado negativo vira prejuízo que
//   abate a base dos anos seguintes.
// - Imposto = base × alíquota (15%). O imposto retido no exterior abate o devido, até o limite dele.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Datelike, NaiveDate};
use serde::Serialize;

use super::assessment::{Side, TaxWarning, Trade};
use super::classify::{treatment, TaxTreatment};
use super::rules::{self, RuleSet};
use crate::domain::cost_basis::{take_from_position, Position, EPS};
use crate::domain::ptax::{RateKind, RateSource};
use crate::error::AppError;

/// Primeiro ano-calendário da apuração anual da Lei 14.754.
pub const FIRST_YEAR: i32 = 2024;

/// Câmbio para BRL: `(moeda, data, lado)` → `(taxa, de onde veio)`, `None` sem cotação. Nunca é chamado para BRL.
pub type RateToBrl<'a> = &'a dyn Fn(&str, NaiveDate, RateKind) -> Option<(f64, RateSource)>;

/// Um provento de ativo no exterior, como saiu de `transactions`, na moeda do ativo.
#[derive(Clone, Debug)]
pub struct ForeignDividend {
    pub tx_id: i32,
    pub asset_id: i32,
    pub ticker: String,
    pub currency: String,
    pub date: NaiveDate,
    pub gross_value: f64,
    /// Imposto retido no exterior, na mesma moeda.
    pub withheld: f64,
}

/// Uma venda, com o custo médio em reais já baixado.
#[derive(Clone, Debug, Serialize)]
pub struct ForeignSale {
    pub tx_id: i32,
    pub portfolio_id: i32,
    pub asset_id: i32,
    pub ticker: String,
    pub currency: String,
    pub date: NaiveDate,
    pub quantity: f64,
    /// Câmbio do dia da venda (1 para ativo em BRL).
    pub fx_rate: f64,
    /// PTAX ou câmbio de mercado (de reserva).
    pub fx_source: RateSource,
    /// Valor de venda bruto, em BRL.
    pub sale_value: f64,
    /// Valor de venda − taxa, em BRL.
    pub proceeds: f64,
    pub cost: f64,
    pub gain: f64,
    /// Vendeu mais do que o histórico de compras conhecido: o ganho só considera a parte com custo conhecido.
    pub cost_unknown: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForeignDividendItem {
    pub tx_id: i32,
    pub asset_id: i32,
    pub ticker: String,
    pub currency: String,
    pub date: NaiveDate,
    /// Câmbio do dia do provento (venda).
    pub fx_rate: f64,
    pub fx_source: RateSource,
    /// Bruto, em BRL.
    pub gross: f64,
    /// Retido no exterior, em BRL.
    pub withheld: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForeignYear {
    pub year: i32,
    pub capital_gain: f64,
    pub dividends: f64,
    /// Imposto retido no exterior no ano, em BRL (o que poderia virar crédito).
    pub withheld: f64,
    /// Prejuízo de anos anteriores disponível para abater.
    pub loss_carried_in: f64,
    pub taxable_base: f64,
    pub rate: f64,
    pub tax_before_credit: f64,
    pub credit_used: f64,
    pub tax_due: f64,
    pub loss_carried_out: f64,
    pub sales: Vec<ForeignSale>,
    pub dividend_items: Vec<ForeignDividendItem>,
}

/// BRL não converte nada (taxa 1, exata: conta como PTAX para não marcar o ano como aproximado).
pub(super) fn convert(currency: &str, date: NaiveDate, kind: RateKind, rate: RateToBrl) -> Option<(f64, RateSource)> {
    if currency == "BRL" {
        Some((1.0, RateSource::Ptax))
    } else {
        rate(currency, date, kind).filter(|(r, _)| r.is_finite() && *r > 0.0)
    }
}

/// Compra ou bonificação já convertida (`fx`): soma quantidade e custo em reais. Compartilhada pela apuração e pelo
/// informe de bens e direitos, para o custo da posição nunca divergir do custo baixado numa venda.
fn apply_acquisition(pos: &mut Position, t: &Trade, fx: f64) {
    pos.quantity += t.quantity;
    pos.cost += match t.side {
        Side::Bonus => t.gross_value * fx,
        _ => (t.gross_value + t.fee) * fx,
    };
}

/// Transferência de um ativo entre custódias: move só quantidade, o custo não muda (o custo médio é do ativo).
#[derive(Clone, Debug)]
pub struct Transfer {
    pub tx_id: i32,
    pub asset_id: i32,
    pub date: NaiveDate,
    pub quantity: f64,
    pub from: Option<i32>,
    pub to: Option<i32>,
}

/// Um ativo do exterior numa custódia numa data de 31/12: quantidade e custo médio em reais (o que a ficha Bens e
/// Direitos da DAA pede: custo de aquisição convertido na data de cada compra, não o valor de mercado).
#[derive(Clone, Debug, Serialize)]
pub struct ForeignHolding {
    /// 31/12 deste ano.
    pub year: i32,
    pub asset_id: i32,
    /// `None` = operações sem custódia informada.
    pub custodia_id: Option<i32>,
    pub ticker: String,
    pub currency: String,
    pub quantity: f64,
    /// Custo médio do ativo × a quantidade desta custódia, em BRL (as custódias somam o custo da posição).
    pub cost_brl: f64,
    /// Alguma compra da posição foi convertida pelo câmbio de mercado (sem PTAX).
    pub fx_approximate: bool,
    /// Faltou câmbio numa operação, ou uma venda passou do histórico conhecido: o custo está subestimado.
    pub incomplete: bool,
}

/// Um fato do ativo na linha do tempo do informe: operação ou transferência entre custódias.
enum HoldingEvent<'a> {
    Trade(&'a Trade),
    Transfer(&'a Transfer),
}

impl HoldingEvent<'_> {
    fn date(&self) -> NaiveDate {
        match self {
            Self::Trade(t) => t.date,
            Self::Transfer(t) => t.date,
        }
    }

    /// No mesmo dia: compra, bonificação e desdobramento, depois a transferência, depois a venda.
    fn order_key(&self) -> (NaiveDate, u8, i32) {
        match self {
            Self::Trade(t) => (t.date, if t.side == Side::Sell { 2 } else { 0 }, t.tx_id),
            Self::Transfer(t) => (t.date, 1, t.tx_id),
        }
    }
}

/// Posição de cada ativo do exterior, por custódia, em cada 31/12 de `FIRST_YEAR − 1` até `through_year` (só com
/// quantidade). Usa a mesma ordem e a mesma conversão de `assess_foreign`; ano sem fato repete a posição anterior.
/// O custo é do ativo (custo médio global): cada custódia leva a quantidade dela × o custo médio, então a soma das
/// custódias é a posição do ativo. Ordenada por `(ano, ativo, custódia)`.
pub fn foreign_holdings(
    trades: &[Trade],
    transfers: &[Transfer],
    rate: RateToBrl,
    through_year: i32,
) -> Vec<ForeignHolding> {
    let mut by_asset: BTreeMap<i32, Vec<HoldingEvent>> = BTreeMap::new();
    for t in trades.iter().filter(|t| treatment(&t.asset_class, &t.ticker) == TaxTreatment::ForeignWarning) {
        by_asset.entry(t.asset_id).or_default().push(HoldingEvent::Trade(t));
    }
    for t in transfers {
        // Só ativos que têm operação do exterior: transferência de outro ativo não entra.
        if let Some(events) = by_asset.get_mut(&t.asset_id) {
            events.push(HoldingEvent::Transfer(t));
        }
    }

    let mut holdings = Vec::new();
    for (asset_id, mut events) in by_asset {
        events.sort_by_key(HoldingEvent::order_key);
        let (ticker, currency) = events
            .iter()
            .find_map(|e| match e {
                HoldingEvent::Trade(t) => Some((t.ticker.clone(), t.currency.clone())),
                HoldingEvent::Transfer(_) => None,
            })
            .expect("o ativo entrou por uma operação");

        let mut pos = Position::default();
        let mut by_custodia: BTreeMap<Option<i32>, f64> = BTreeMap::new();
        let (mut approximate, mut incomplete) = (false, false);
        let mut next_year = FIRST_YEAR - 1;
        let snapshot = |year: i32,
                        pos: &Position,
                        by_custodia: &BTreeMap<Option<i32>, f64>,
                        approximate: bool,
                        incomplete: bool,
                        out: &mut Vec<ForeignHolding>| {
            if pos.quantity <= EPS {
                return;
            }
            // Saldo negativo numa custódia, ou custódias que não somam a posição: o rateio não fecha.
            let sum: f64 = by_custodia.values().sum();
            let uneven = by_custodia.values().any(|q| *q < -1e-6) || (sum - pos.quantity).abs() > 1e-6;
            let before = out.len();
            for (custodia_id, quantity) in by_custodia.iter().filter(|(_, q)| **q > EPS) {
                out.push(ForeignHolding {
                    year,
                    asset_id,
                    custodia_id: *custodia_id,
                    ticker: ticker.clone(),
                    currency: currency.clone(),
                    quantity: *quantity,
                    cost_brl: pos.cost * quantity / pos.quantity,
                    fx_approximate: approximate,
                    incomplete: incomplete || uneven,
                });
            }
            if out.len() == before {
                // Venda sem saldo deixou todas as custódias zeradas ou negativas, mas a posição existe: mostra-a inteira.
                out.push(ForeignHolding {
                    year,
                    asset_id,
                    custodia_id: None,
                    ticker: ticker.clone(),
                    currency: currency.clone(),
                    quantity: pos.quantity,
                    cost_brl: pos.cost,
                    fx_approximate: approximate,
                    incomplete: true,
                });
            }
        };

        for event in &events {
            // Os 31/12 que ficaram para trás são fotos do estado antes deste evento.
            while next_year < event.date().year() && next_year <= through_year {
                snapshot(next_year, &pos, &by_custodia, approximate, incomplete, &mut holdings);
                next_year += 1;
            }
            let t = match event {
                HoldingEvent::Transfer(tr) => {
                    *by_custodia.entry(tr.from).or_insert(0.0) -= tr.quantity;
                    *by_custodia.entry(tr.to).or_insert(0.0) += tr.quantity;
                    continue;
                }
                HoldingEvent::Trade(t) => t,
            };
            if t.side == Side::Split {
                pos.quantity *= t.quantity;
                by_custodia.values_mut().for_each(|q| *q *= t.quantity);
                continue;
            }
            let Some((fx, source)) = convert(&t.currency, t.date, RateKind::Sell, rate) else {
                incomplete = true;
                continue;
            };
            match t.side {
                Side::Buy | Side::Bonus => {
                    approximate |= source == RateSource::Market;
                    apply_acquisition(&mut pos, t, fx);
                    *by_custodia.entry(t.custodia_id).or_insert(0.0) += t.quantity;
                }
                Side::Sell => {
                    let (_, covered) = take_from_position(&mut pos, t.quantity);
                    incomplete |= covered + EPS < t.quantity;
                    *by_custodia.entry(t.custodia_id).or_insert(0.0) -= t.quantity;
                }
                Side::Split => unreachable!("tratado acima"),
            }
        }
        while next_year <= through_year {
            snapshot(next_year, &pos, &by_custodia, approximate, incomplete, &mut holdings);
            next_year += 1;
        }
    }
    holdings.sort_by_key(|h| (h.year, h.asset_id, h.custodia_id));
    holdings
}

/// Apura ano a ano o IR dos ativos do exterior. `trades` e `dividends` podem vir em qualquer ordem e misturar outras
/// classes (os trades fora de `ForeignWarning` são ignorados). Item sem câmbio na data fica de fora e gera aviso.
pub fn assess_foreign(
    trades: &[Trade],
    dividends: &[ForeignDividend],
    rules: &RuleSet,
    rate: RateToBrl,
) -> Result<(Vec<ForeignYear>, Vec<TaxWarning>), AppError> {
    let mut warnings = Vec::new();
    let mut fx_missing_years: BTreeSet<i32> = BTreeSet::new();
    let mut fx_approximate_years: BTreeSet<i32> = BTreeSet::new();

    let mut foreign: Vec<&Trade> =
        trades.iter().filter(|t| treatment(&t.asset_class, &t.ticker) == TaxTreatment::ForeignWarning).collect();
    // Dentro do dia, compras e eventos antes das vendas, como na apuração da B3.
    foreign.sort_by_key(|t| (t.asset_id, t.date, t.side == Side::Sell, t.tx_id));

    let mut sales_by_year: BTreeMap<i32, Vec<ForeignSale>> = BTreeMap::new();
    let mut positions: HashMap<i32, Position> = HashMap::new();
    for t in foreign {
        let pos = positions.entry(t.asset_id).or_default();
        // Desdobramento só tem razão, sem valor: não precisa de câmbio.
        if t.side == Side::Split {
            pos.quantity *= t.quantity;
            continue;
        }
        let Some((fx, fx_source)) = convert(&t.currency, t.date, RateKind::Sell, rate) else {
            fx_missing_years.insert(t.date.year());
            continue;
        };
        if fx_source == RateSource::Market {
            fx_approximate_years.insert(t.date.year());
        }
        match t.side {
            Side::Buy | Side::Bonus => apply_acquisition(pos, t, fx),
            Side::Sell => {
                let (cost, covered) = take_from_position(pos, t.quantity);
                let cost_unknown = covered + EPS < t.quantity;
                let share = if t.quantity > EPS { covered / t.quantity } else { 0.0 };
                if cost_unknown && t.date.year() >= FIRST_YEAR {
                    warnings.push(TaxWarning::CostUnknown { ticker: t.ticker.clone(), date: t.date });
                }
                if t.date.year() < FIRST_YEAR {
                    continue;
                }
                let proceeds = (t.gross_value - t.fee) * fx * share;
                sales_by_year.entry(t.date.year()).or_default().push(ForeignSale {
                    tx_id: t.tx_id,
                    portfolio_id: t.portfolio_id,
                    asset_id: t.asset_id,
                    ticker: t.ticker.clone(),
                    currency: t.currency.clone(),
                    date: t.date,
                    quantity: covered,
                    fx_rate: fx,
                    fx_source,
                    sale_value: t.gross_value * fx * share,
                    proceeds,
                    cost,
                    gain: proceeds - cost,
                    cost_unknown,
                });
            }
            Side::Split => unreachable!("tratado acima"),
        }
    }

    let mut dividends_by_year: BTreeMap<i32, Vec<ForeignDividendItem>> = BTreeMap::new();
    for dividend in dividends.iter().filter(|d| d.date.year() >= FIRST_YEAR) {
        let Some((fx, fx_source)) = convert(&dividend.currency, dividend.date, RateKind::Sell, rate) else {
            fx_missing_years.insert(dividend.date.year());
            continue;
        };
        // O imposto retido usa a cotação para compra; sem ela, a de venda do mesmo dia (já convertida acima).
        let (withheld_fx, withheld_source) =
            convert(&dividend.currency, dividend.date, RateKind::Buy, rate).unwrap_or((fx, fx_source));
        if fx_source == RateSource::Market || (dividend.withheld > 0.0 && withheld_source == RateSource::Market) {
            fx_approximate_years.insert(dividend.date.year());
        }
        dividends_by_year.entry(dividend.date.year()).or_default().push(ForeignDividendItem {
            tx_id: dividend.tx_id,
            asset_id: dividend.asset_id,
            ticker: dividend.ticker.clone(),
            currency: dividend.currency.clone(),
            date: dividend.date,
            fx_rate: fx,
            fx_source,
            gross: dividend.gross_value * fx,
            withheld: dividend.withheld * withheld_fx,
        });
    }
    for items in dividends_by_year.values_mut() {
        items.sort_by_key(|i| (i.date, i.tx_id));
    }

    let years: BTreeSet<i32> = sales_by_year.keys().chain(dividends_by_year.keys()).copied().collect();
    let mut result = Vec::new();
    let mut loss_carried: f64 = 0.0;
    for year in years {
        let mut sales = sales_by_year.remove(&year).unwrap_or_default();
        sales.sort_by_key(|s| (s.date, s.tx_id));
        let dividend_items = dividends_by_year.remove(&year).unwrap_or_default();

        // `sum()` de f64 vazio dá −0,0 (e a tela mostraria "-R$ 0,00"): soma a partir de +0,0.
        let capital_gain = sales.iter().fold(0.0, |acc, s| acc + s.gain);
        let dividends_total = dividend_items.iter().fold(0.0, |acc, i| acc + i.gross);
        let withheld = dividend_items.iter().fold(0.0, |acc, i| acc + i.withheld);
        let net_result = capital_gain + dividends_total;

        let loss_carried_in = loss_carried;
        let (taxable_base, loss_carried_out) = if net_result >= 0.0 {
            let used = loss_carried_in.min(net_result);
            (net_result - used, loss_carried_in - used)
        } else {
            (0.0, loss_carried_in - net_result)
        };
        loss_carried = loss_carried_out;

        let year_end = NaiveDate::from_ymd_opt(year, 12, 31)
            .ok_or_else(|| AppError::InvalidGuard(format!("invalid year {year}")))?;
        let tax_rate = rules.get(rules::FOREIGN_RATE, year_end)?;
        let tax_before_credit = taxable_base * tax_rate;
        let credit_used = withheld.min(tax_before_credit);

        result.push(ForeignYear {
            year,
            capital_gain,
            dividends: dividends_total,
            withheld,
            loss_carried_in,
            taxable_base,
            rate: tax_rate,
            tax_before_credit,
            credit_used,
            tax_due: tax_before_credit - credit_used,
            loss_carried_out,
            sales,
            dividend_items,
        });
    }

    for year in fx_approximate_years {
        warnings.push(TaxWarning::ForeignFxApproximate { year });
    }
    for year in fx_missing_years {
        warnings.push(TaxWarning::ForeignFxMissing { year });
    }
    Ok((result, warnings))
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

    fn trade(id: i32, asset: i32, date: &str, side: Side, quantity: f64, gross: f64, fee: f64) -> Trade {
        Trade {
            tx_id: id,
            portfolio_id: 1,
            asset_id: asset,
            ticker: format!("T{asset}"),
            asset_class: "acao_internacional".to_string(),
            currency: "USD".to_string(),
            custodia_id: None,
            date: d(date),
            side,
            quantity,
            gross_value: gross,
            fee,
        }
    }

    fn dividend(id: i32, date: &str, gross: f64, withheld: f64) -> ForeignDividend {
        ForeignDividend {
            tx_id: id,
            asset_id: 1,
            ticker: "T1".to_string(),
            currency: "USD".to_string(),
            date: d(date),
            gross_value: gross,
            withheld,
        }
    }

    /// PTAX do dólar por ano (igual para compra e venda): 5 em 2024, 6 em 2025, 4 em 2026; sem cotação para o resto.
    fn yearly_rate(currency: &str, date: NaiveDate, _kind: RateKind) -> Option<(f64, RateSource)> {
        match (currency, date.year()) {
            ("USD", 2024) => Some((5.0, RateSource::Ptax)),
            ("USD", 2025) => Some((6.0, RateSource::Ptax)),
            ("USD", 2026) => Some((4.0, RateSource::Ptax)),
            _ => None,
        }
    }

    fn run(trades: &[Trade], dividends: &[ForeignDividend]) -> (Vec<ForeignYear>, Vec<TaxWarning>) {
        assess_foreign(trades, dividends, &test_rule_set(), &yearly_rate).unwrap()
    }

    #[test]
    fn the_withholding_is_converted_at_the_buy_quote_and_the_rest_at_the_sell_quote() {
        // venda 6,00 e compra 5,90 em 2025: provento US$ 100 → 600; retido US$ 10 → 59
        let ptax = |currency: &str, date: NaiveDate, kind: RateKind| -> Option<(f64, RateSource)> {
            let (sell, _) = yearly_rate(currency, date, kind)?;
            let value = if kind == RateKind::Buy { sell - 0.1 } else { sell };
            Some((value, RateSource::Ptax))
        };
        let (years, warnings) =
            assess_foreign(&[], &[dividend(1, "2025-05-01", 100.0, 10.0)], &test_rule_set(), &ptax).unwrap();
        let y = &years[0];
        assert!(close(y.dividends, 600.0) && close(y.withheld, 59.0), "{y:?}");
        assert!(close(y.dividend_items[0].fx_rate, 6.0) && y.dividend_items[0].fx_source == RateSource::Ptax);
        assert!(warnings.is_empty());
    }

    #[test]
    fn items_converted_at_the_market_rate_mark_the_year_as_approximate() {
        let market = |currency: &str, date: NaiveDate, kind: RateKind| {
            yearly_rate(currency, date, kind).map(|(rate, _)| (rate, RateSource::Market))
        };
        let (years, warnings) = assess_foreign(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0),
                trade(2, 1, "2025-03-01", Side::Sell, 10.0, 1_200.0, 0.0),
            ],
            &[],
            &test_rule_set(),
            &market,
        )
        .unwrap();
        // o resultado é o mesmo; o aviso aparece nos dois anos de onde saíram taxas de mercado
        assert!(close(years[0].capital_gain, 2_200.0));
        assert_eq!(years[0].sales[0].fx_source, RateSource::Market);
        let approximate: Vec<i32> = warnings
            .iter()
            .filter_map(|w| match w {
                TaxWarning::ForeignFxApproximate { year } => Some(*year),
                _ => None,
            })
            .collect();
        assert_eq!(approximate, vec![2024, 2025]);
    }

    #[test]
    fn ptax_everywhere_and_brl_assets_leave_the_year_exact() {
        let (_, warnings) = run(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0),
                trade(2, 1, "2025-03-01", Side::Sell, 10.0, 1_200.0, 0.0),
            ],
            &[dividend(3, "2025-05-01", 100.0, 10.0)],
        );
        assert!(warnings.is_empty(), "{warnings:?}");

        let mut brl = trade(4, 2, "2025-01-10", Side::Buy, 1.0, 100.0, 0.0);
        brl.currency = "BRL".to_string();
        let mut brl_sale = trade(5, 2, "2025-06-10", Side::Sell, 1.0, 150.0, 0.0);
        brl_sale.currency = "BRL".to_string();
        let market_only = |_: &str, _: NaiveDate, _: RateKind| Some((9.0, RateSource::Market));
        let (years, warnings) = assess_foreign(&[brl, brl_sale], &[], &test_rule_set(), &market_only).unwrap();
        assert!(close(years[0].capital_gain, 50.0), "BRL não passa pelo câmbio");
        assert!(warnings.is_empty());
    }

    fn holdings(trades: &[Trade], through: i32) -> Vec<ForeignHolding> {
        foreign_holdings(trades, &[], &yearly_rate, through)
    }

    fn in_custodia(mut t: Trade, custodia: i32) -> Trade {
        t.custodia_id = Some(custodia);
        t
    }

    fn transfer(id: i32, date: &str, quantity: f64, from: i32, to: i32) -> Transfer {
        Transfer { tx_id: id, asset_id: 1, date: d(date), quantity, from: Some(from), to: Some(to) }
    }

    fn at_custodia(list: &[ForeignHolding], year: i32, custodia: i32) -> Option<&ForeignHolding> {
        list.iter().find(|h| h.year == year && h.asset_id == 1 && h.custodia_id == Some(custodia))
    }

    #[test]
    fn a_transfer_splits_the_holding_by_custody_at_the_average_cost_of_the_asset() {
        // 10 ações na custódia 1 (5.000); em 2025 transfere 4 para a 2 e compra mais 10 por 7.200 na custódia 2
        let list = foreign_holdings(
            &[
                in_custodia(trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0), 1),
                in_custodia(trade(3, 1, "2025-06-01", Side::Buy, 10.0, 1_200.0, 0.0), 2),
            ],
            &[transfer(2, "2025-03-01", 4.0, 1, 2)],
            &yearly_rate,
            2025,
        );
        let (a24, a25, b25) =
            (at_custodia(&list, 2024, 1).unwrap(), at_custodia(&list, 2025, 1).unwrap(), at_custodia(&list, 2025, 2).unwrap());
        assert!(close(a24.quantity, 10.0) && close(a24.cost_brl, 5_000.0), "{a24:?}");
        assert!(at_custodia(&list, 2024, 2).is_none());
        // custo médio global em 2025: 12.200 / 20 = 610 por ação
        assert!(close(a25.quantity, 6.0) && close(a25.cost_brl, 3_660.0), "{a25:?}");
        assert!(close(b25.quantity, 14.0) && close(b25.cost_brl, 8_540.0), "{b25:?}");
        assert!(close(a25.cost_brl + b25.cost_brl, 12_200.0), "as custódias somam a posição");
        assert!(!a25.incomplete && !b25.incomplete);
    }

    #[test]
    fn a_sale_takes_the_quantity_from_its_custody_and_the_cost_at_the_global_average() {
        let list = foreign_holdings(
            &[
                in_custodia(trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0), 1),
                in_custodia(trade(3, 1, "2025-03-01", Side::Sell, 3.0, 450.0, 0.0), 2),
            ],
            &[transfer(2, "2024-12-01", 5.0, 1, 2)],
            &yearly_rate,
            2025,
        );
        let (a, b) = (at_custodia(&list, 2025, 1).unwrap(), at_custodia(&list, 2025, 2).unwrap());
        // sobram 7 com 3.500 (500 por ação): 5 na custódia 1 e 2 na 2
        assert!(close(a.quantity, 5.0) && close(a.cost_brl, 2_500.0), "{a:?}");
        assert!(close(b.quantity, 2.0) && close(b.cost_brl, 1_000.0), "{b:?}");
    }

    #[test]
    fn a_transfer_on_the_day_of_a_sale_counts_before_it() {
        // vende 4 na custódia 2 no mesmo dia em que as 4 chegam a ela: não fica saldo negativo
        let list = foreign_holdings(
            &[
                in_custodia(trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0), 1),
                in_custodia(trade(2, 1, "2025-03-01", Side::Sell, 4.0, 600.0, 0.0), 2),
            ],
            &[transfer(3, "2025-03-01", 4.0, 1, 2)],
            &yearly_rate,
            2025,
        );
        assert!(at_custodia(&list, 2025, 2).is_none(), "a custódia 2 zerou");
        let a = at_custodia(&list, 2025, 1).unwrap();
        assert!(close(a.quantity, 6.0) && close(a.cost_brl, 3_000.0) && !a.incomplete, "{a:?}");
    }

    #[test]
    fn a_split_scales_every_custody() {
        let list = foreign_holdings(
            &[
                in_custodia(trade(1, 1, "2024-03-01", Side::Buy, 2.0, 200.0, 0.0), 1),
                trade(3, 1, "2025-01-02", Side::Split, 2.0, 0.0, 0.0),
            ],
            &[transfer(2, "2024-06-01", 1.0, 1, 2)],
            &yearly_rate,
            2025,
        );
        let (a, b) = (at_custodia(&list, 2025, 1).unwrap(), at_custodia(&list, 2025, 2).unwrap());
        assert!(close(a.quantity, 2.0) && close(b.quantity, 2.0), "{a:?} {b:?}");
        assert!(close(a.cost_brl + b.cost_brl, 1_000.0));
    }

    #[test]
    fn a_sale_above_the_balance_of_its_custody_marks_the_holding_incomplete() {
        // a custódia 2 vende 3 sem ter nada: o saldo dela fica negativo e o rateio não fecha
        let list = foreign_holdings(
            &[
                in_custodia(trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0), 1),
                in_custodia(trade(2, 1, "2025-03-01", Side::Sell, 3.0, 450.0, 0.0), 2),
            ],
            &[],
            &yearly_rate,
            2025,
        );
        let a = at_custodia(&list, 2025, 1).unwrap();
        assert!(a.incomplete && close(a.quantity, 10.0), "{a:?}");
    }

    #[test]
    fn a_transfer_of_an_asset_without_foreign_trades_is_ignored() {
        let mut other = transfer(1, "2025-03-01", 1.0, 1, 2);
        other.asset_id = 9;
        assert!(foreign_holdings(&[], &[other], &yearly_rate, 2025).is_empty());
    }

    fn at(list: &[ForeignHolding], year: i32, asset: i32) -> Option<&ForeignHolding> {
        list.iter().find(|h| h.year == year && h.asset_id == asset)
    }

    #[test]
    fn the_position_in_each_december_carries_the_average_cost_in_reais() {
        // 2024: US$ 1.000 a R$ 5 = 5.000; 2025: mais US$ 1.200 a R$ 6 = 7.200 → 12.200 com 20 ações
        let list = holdings(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0),
                trade(2, 1, "2025-03-01", Side::Buy, 10.0, 1_200.0, 0.0),
            ],
            2026,
        );
        let (y24, y25, y26) = (at(&list, 2024, 1).unwrap(), at(&list, 2025, 1).unwrap(), at(&list, 2026, 1).unwrap());
        assert!(close(y24.quantity, 10.0) && close(y24.cost_brl, 5_000.0), "{y24:?}");
        assert!(close(y25.quantity, 20.0) && close(y25.cost_brl, 12_200.0), "{y25:?}");
        assert!(close(y26.quantity, 20.0) && close(y26.cost_brl, 12_200.0), "um ano sem fato repete a posição");
        assert!(at(&list, 2023, 1).is_none(), "ainda não tinha nada em 31/12/2023");
        assert!(!y25.incomplete && !y25.fx_approximate);
    }

    #[test]
    fn a_partial_sale_keeps_the_average_cost_and_a_full_sale_removes_the_asset() {
        let list = holdings(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0),
                trade(2, 1, "2025-03-01", Side::Sell, 4.0, 600.0, 0.0),
                trade(3, 1, "2026-03-01", Side::Sell, 6.0, 900.0, 0.0),
            ],
            2026,
        );
        let y25 = at(&list, 2025, 1).unwrap();
        // custo médio 500 por ação: sobram 6 com 3.000
        assert!(close(y25.quantity, 6.0) && close(y25.cost_brl, 3_000.0), "{y25:?}");
        assert!(at(&list, 2026, 1).is_none(), "vendido por inteiro em 2026");
    }

    #[test]
    fn a_split_scales_the_quantity_and_keeps_the_cost_in_the_holding() {
        let list = holdings(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 1.0, 100.0, 0.0),
                trade(2, 1, "2025-01-02", Side::Split, 2.0, 0.0, 0.0),
            ],
            2025,
        );
        let y25 = at(&list, 2025, 1).unwrap();
        assert!(close(y25.quantity, 2.0) && close(y25.cost_brl, 500.0), "{y25:?}");
    }

    #[test]
    fn a_missing_rate_or_a_sale_above_the_known_cost_marks_the_holding_incomplete() {
        // 2027 não tem câmbio: a segunda compra fica de fora e a posição fica marcada
        let missing = holdings(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0),
                trade(2, 1, "2027-03-01", Side::Buy, 10.0, 1_000.0, 0.0),
            ],
            2027,
        );
        assert!(!at(&missing, 2026, 1).unwrap().incomplete);
        let y27 = at(&missing, 2027, 1).unwrap();
        assert!(y27.incomplete && close(y27.quantity, 10.0), "{y27:?}");

        // vendeu 3 tendo comprado 1: o custo conhecido não cobre
        let unknown = holdings(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 1.0, 100.0, 0.0),
                trade(2, 1, "2025-03-01", Side::Sell, 3.0, 300.0, 0.0),
                trade(3, 1, "2026-03-01", Side::Buy, 1.0, 100.0, 0.0),
            ],
            2026,
        );
        assert!(at(&unknown, 2025, 1).is_none());
        assert!(at(&unknown, 2026, 1).unwrap().incomplete);
    }

    #[test]
    fn a_purchase_converted_at_the_market_rate_marks_the_holding_as_approximate() {
        let market = |currency: &str, date: NaiveDate, kind: RateKind| {
            yearly_rate(currency, date, kind)
                .map(|(rate, _)| (rate, if date.year() >= 2025 { RateSource::Market } else { RateSource::Ptax }))
        };
        let list = foreign_holdings(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0),
                trade(2, 1, "2025-03-01", Side::Buy, 10.0, 1_200.0, 0.0),
            ],
            &[],
            &market,
            2025,
        );
        assert!(!at(&list, 2024, 1).unwrap().fx_approximate);
        assert!(at(&list, 2025, 1).unwrap().fx_approximate);
    }

    #[test]
    fn each_asset_has_its_own_holding_ordered_by_year_and_asset() {
        let list = holdings(
            &[
                trade(1, 2, "2024-03-01", Side::Buy, 5.0, 500.0, 0.0),
                trade(2, 1, "2024-04-01", Side::Buy, 10.0, 1_000.0, 0.0),
            ],
            2025,
        );
        let keys: Vec<(i32, i32)> = list.iter().map(|h| (h.year, h.asset_id)).collect();
        assert_eq!(keys, vec![(2024, 1), (2024, 2), (2025, 1), (2025, 2)]);
    }

    #[test]
    fn the_holding_cost_is_the_cost_a_later_sale_takes_out() {
        let trades = [
            trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 20.0),
            trade(2, 1, "2024-09-01", Side::Buy, 5.0, 700.0, 0.0),
            trade(3, 1, "2025-06-01", Side::Sell, 15.0, 2_000.0, 0.0),
        ];
        let y24 = holdings(&trades, 2024).into_iter().find(|h| h.year == 2024).unwrap();
        let (years, _) = run(&trades, &[]);
        assert!(close(years[0].sales[0].cost, y24.cost_brl), "{} x {}", years[0].sales[0].cost, y24.cost_brl);
    }

    #[test]
    fn other_classes_and_assets_in_brl_follow_the_same_rules() {
        let mut stock = trade(1, 1, "2024-03-01", Side::Buy, 1.0, 100.0, 0.0);
        stock.asset_class = "acao_br".to_string();
        let mut brl = trade(2, 2, "2024-03-01", Side::Buy, 2.0, 100.0, 0.0);
        brl.currency = "BRL".to_string();
        let list = holdings(&[stock, brl], 2024);
        assert!(at(&list, 2024, 1).is_none(), "ação brasileira não é do exterior");
        assert!(close(at(&list, 2024, 2).unwrap().cost_brl, 100.0), "ativo em BRL não passa por câmbio");
    }

    #[test]
    fn the_cost_is_converted_on_the_buy_day_and_the_sale_on_its_own_day() {
        // compra US$ 1.000 a R$ 5 (custo 5.000), venda US$ 1.200 a R$ 6 (7.200): ganho 2.200, imposto 330
        let (years, warnings) = run(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0),
                trade(2, 1, "2025-03-01", Side::Sell, 10.0, 1_200.0, 0.0),
            ],
            &[],
        );
        assert!(warnings.is_empty());
        assert_eq!(years.len(), 1);
        let y = &years[0];
        assert_eq!(y.year, 2025);
        assert!(close(y.capital_gain, 2_200.0) && close(y.taxable_base, 2_200.0), "{y:?}");
        assert!(close(y.tax_due, 330.0) && close(y.rate, 0.15));
        assert!(close(y.sales[0].sale_value, 7_200.0) && close(y.sales[0].cost, 5_000.0));
    }

    #[test]
    fn fees_add_to_the_cost_and_reduce_the_proceeds() {
        // custo (1.000 + 20) × 5 = 5.100; recebe (1.200 − 30) × 6 = 7.020; ganho 1.920
        let (years, _) = run(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 20.0),
                trade(2, 1, "2025-03-01", Side::Sell, 10.0, 1_200.0, 30.0),
            ],
            &[],
        );
        assert!(close(years[0].capital_gain, 1_920.0), "{:?}", years[0]);
        assert!(close(years[0].sales[0].sale_value, 7_200.0), "o valor bruto não leva a taxa");
    }

    #[test]
    fn gains_and_losses_of_the_year_offset_each_other() {
        // ativo 1: ganho (US$ 100 → 150, a R$ 6 vs 5); ativo 2: perda
        let (years, _) = run(
            &[
                trade(1, 1, "2025-01-10", Side::Buy, 1.0, 100.0, 0.0),
                trade(2, 2, "2025-01-10", Side::Buy, 1.0, 500.0, 0.0),
                trade(3, 1, "2025-06-10", Side::Sell, 1.0, 200.0, 0.0),
                trade(4, 2, "2025-06-10", Side::Sell, 1.0, 300.0, 0.0),
            ],
            &[],
        );
        // ganho 100 × 6 = 600; perda −200 × 6 = −1.200 → resultado −600
        assert!(close(years[0].capital_gain, -600.0), "{:?}", years[0]);
        assert!(close(years[0].taxable_base, 0.0) && close(years[0].tax_due, 0.0));
        assert!(close(years[0].loss_carried_out, 600.0));
    }

    #[test]
    fn a_year_without_dividends_or_sales_has_positive_zero_not_negative_zero() {
        // `sum()` de f64 vazio dá −0,0 e a tela mostrava "-R$ 0,00"
        let (years, _) = run(
            &[
                trade(1, 1, "2024-03-01", Side::Buy, 10.0, 1_000.0, 0.0),
                trade(2, 1, "2025-03-01", Side::Sell, 10.0, 1_200.0, 0.0),
            ],
            &[],
        );
        let y = &years[0];
        assert!(y.dividends.is_sign_positive() && y.withheld.is_sign_positive(), "{y:?}");

        let (years, _) = run(&[], &[dividend(1, "2025-05-01", 100.0, 0.0)]);
        assert!(years[0].capital_gain.is_sign_positive(), "{:?}", years[0]);
    }

    #[test]
    fn a_loss_carries_into_the_next_years_and_is_used_up() {
        let (years, _) = run(
            &[
                trade(1, 1, "2024-01-10", Side::Buy, 2.0, 1_000.0, 0.0),
                trade(2, 1, "2024-06-10", Side::Sell, 1.0, 400.0, 0.0),
                trade(3, 1, "2025-06-10", Side::Sell, 1.0, 700.0, 0.0),
            ],
            &[],
        );
        // 2024: custo 500 × 5 = 2.500, venda 400 × 5 = 2.000 → −500. 2025: 700 × 6 = 4.200 − 2.500 = 1.700; abate 500.
        assert_eq!(years.iter().map(|y| y.year).collect::<Vec<_>>(), vec![2024, 2025]);
        assert!(close(years[0].loss_carried_out, 500.0) && close(years[0].tax_due, 0.0));
        assert!(close(years[1].loss_carried_in, 500.0) && close(years[1].taxable_base, 1_200.0), "{:?}", years[1]);
        assert!(close(years[1].tax_due, 180.0) && close(years[1].loss_carried_out, 0.0));
    }

    #[test]
    fn dividends_add_to_the_base_and_the_foreign_withholding_is_a_credit() {
        // US$ 100 de provento a R$ 6 = 600, retido US$ 30 = 180; imposto 90, crédito limitado a 90
        let (years, _) = run(&[], &[dividend(1, "2025-05-01", 100.0, 30.0)]);
        let y = &years[0];
        assert!(close(y.dividends, 600.0) && close(y.withheld, 180.0) && close(y.taxable_base, 600.0), "{y:?}");
        assert!(close(y.tax_before_credit, 90.0) && close(y.credit_used, 90.0) && close(y.tax_due, 0.0));
    }

    #[test]
    fn a_small_withholding_only_reduces_the_tax() {
        // retido US$ 5 = 30; imposto 90 → paga 60
        let (years, _) = run(&[], &[dividend(1, "2025-05-01", 100.0, 5.0)]);
        assert!(close(years[0].credit_used, 30.0) && close(years[0].tax_due, 60.0), "{:?}", years[0]);
    }

    #[test]
    fn a_missing_rate_leaves_the_item_out_and_warns() {
        let (years, warnings) = run(&[], &[dividend(1, "2027-05-01", 100.0, 0.0)]);
        assert!(years.is_empty());
        assert!(warnings.iter().any(|w| matches!(w, TaxWarning::ForeignFxMissing { year: 2027 })));
    }

    #[test]
    fn a_sale_before_2024_is_not_assessed_but_its_buy_still_forms_the_cost() {
        let (years, warnings) = run(
            &[
                trade(1, 1, "2023-01-10", Side::Buy, 2.0, 1_000.0, 0.0),
                trade(2, 1, "2023-06-10", Side::Sell, 1.0, 600.0, 0.0),
            ],
            &[dividend(3, "2023-07-01", 50.0, 0.0)],
        );
        assert!(years.is_empty());
        // sem câmbio em 2023 as duas operações ficam de fora (o aviso diz o ano)
        assert!(warnings.iter().any(|w| matches!(w, TaxWarning::ForeignFxMissing { year: 2023 })));
    }

    #[test]
    fn a_sale_above_the_known_cost_warns_and_only_counts_the_covered_part() {
        let (years, warnings) = run(
            &[
                trade(1, 1, "2024-01-10", Side::Buy, 1.0, 100.0, 0.0),
                trade(2, 1, "2025-06-10", Side::Sell, 2.0, 400.0, 0.0),
            ],
            &[],
        );
        assert!(warnings.iter().any(|w| matches!(w, TaxWarning::CostUnknown { .. })));
        let sale = &years[0].sales[0];
        assert!(sale.cost_unknown && close(sale.quantity, 1.0));
        // metade vendida com custo: valor 400 × 6 × ½ = 1.200, custo 500, ganho 700
        assert!(close(sale.gain, 700.0), "{sale:?}");
    }

    #[test]
    fn other_classes_are_ignored() {
        let mut stock = trade(1, 1, "2025-01-10", Side::Buy, 1.0, 100.0, 0.0);
        stock.asset_class = "acao_br".to_string();
        let mut sale = trade(2, 1, "2025-06-10", Side::Sell, 1.0, 200.0, 0.0);
        sale.asset_class = "acao_br".to_string();
        let (years, warnings) = run(&[stock, sale], &[]);
        assert!(years.is_empty() && warnings.is_empty());
    }

    #[test]
    fn a_split_scales_the_quantity_and_keeps_the_cost() {
        let (years, warnings) = run(
            &[
                trade(1, 1, "2024-01-10", Side::Buy, 1.0, 100.0, 0.0),
                trade(2, 1, "2025-01-02", Side::Split, 2.0, 0.0, 0.0),
                trade(3, 1, "2025-06-10", Side::Sell, 2.0, 300.0, 0.0),
            ],
            &[],
        );
        // custo 100 × 5 = 500; venda 300 × 6 = 1.800; ganho 1.300, sem aviso de custo desconhecido
        assert!(warnings.is_empty());
        assert!(close(years[0].capital_gain, 1_300.0), "{:?}", years[0]);
    }
}
