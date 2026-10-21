use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Datelike, NaiveDate};
use serde::Serialize;

use super::calendar::darf_due_date;
use super::classify::{is_unit, treatment, TaxTreatment};
use super::crypto::{assess_crypto, CryptoMonth};
use super::foreign::{self, ForeignYear};
use super::rules::{self, RuleSet};
use crate::domain::cost_basis::{take_from_position, Position, EPS};
use crate::error::AppError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
    /// Desdobramento/grupamento: `quantity` é a razão (2 = 1→2; 0,1 = 10→1).
    Split,
    /// Bonificação: `quantity` são as ações novas e `gross_value` o custo atribuído.
    Bonus,
}

/// Uma compra ou venda, já resolvida com os dados do ativo.
#[derive(Clone, Debug)]
pub struct Trade {
    pub tx_id: i32,
    pub portfolio_id: i32,
    pub asset_id: i32,
    pub ticker: String,
    pub asset_class: String,
    pub currency: String,
    pub custodia_id: Option<i32>,
    pub date: NaiveDate,
    pub side: Side,
    pub quantity: f64,
    /// `transactions.total_value` (sem taxas).
    pub gross_value: f64,
    pub fee: f64,
}

/// Resultado realizado de uma posição em opção **encerrada** (Fase 18). Opção
/// não tem custo médio nem isenção: cada posição é uma operação com receita e
/// custo próprios, apurada no mês do encerramento.
#[derive(Clone, Debug)]
pub struct OptionResult {
    pub position_id: i32,
    pub portfolio_id: i32,
    /// Ticker da série (`PETRK600`).
    pub ticker: String,
    /// Data do encerramento.
    pub date: NaiveDate,
    /// Aberta e encerrada no mesmo dia: vai pra gaveta de day trade.
    pub day_trade: bool,
    pub quantity: f64,
    pub proceeds: f64,
    pub cost: f64,
    /// Opção exercida: o prêmio entra no custo da ação, não é ganho nem perda
    /// própria. Só gera aviso.
    pub exercised: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bucket {
    /// Operações comuns: ações, units, ETF, BDR.
    Common,
    DayTrade,
    Fii,
}

/// Resultado de uma venda (ou da parte day trade dela).
#[derive(Clone, Debug, Serialize)]
pub struct SaleResult {
    pub tx_id: i32,
    pub portfolio_id: i32,
    pub asset_id: i32,
    pub ticker: String,
    pub date: NaiveDate,
    pub bucket: Bucket,
    pub is_unit: bool,
    /// Ação comum (não unit) vendida em operação comum — entra na soma do
    /// limite de isenção.
    pub exemption_eligible: bool,
    pub quantity: f64,
    /// Valor de venda bruto (base do limite de isenção e do IRRF 0,005%).
    pub sale_value: f64,
    /// Valor de venda − taxas de venda.
    pub proceeds: f64,
    /// Custo de aquisição (preço médio × quantidade, taxas de compra inclusas).
    pub cost: f64,
    pub gain: f64,
    /// Vendeu mais do que o histórico de compras conhecido — o ganho só
    /// considera a parte com custo conhecido.
    pub cost_unknown: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct BucketResult {
    pub bucket: Bucket,
    /// Resultado do mês (ganhos − perdas), já sem o lucro isento.
    pub result: f64,
    pub loss_used: f64,
    /// Prejuízo acumulado dessa gaveta depois deste mês.
    pub loss_carried: f64,
    pub taxable: f64,
    pub rate: f64,
    pub tax: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MonthAssessment {
    /// `YYYY-MM`
    pub period: String,
    pub stock_sales_total: f64,
    pub stock_exempt: bool,
    pub exempt_gain: f64,
    pub buckets: Vec<BucketResult>,
    pub gross_tax: f64,
    /// IRRF estimado retido nas vendas deste mês.
    pub irrf_withheld: f64,
    pub irrf_used: f64,
    /// Imposto abaixo do mínimo de DARF vindo de meses anteriores.
    pub carried_in: f64,
    /// Valor do DARF deste mês (0 se ficou abaixo do mínimo).
    pub due: f64,
    /// Valor que ficou abaixo do mínimo e passa pro mês seguinte.
    pub carried_out: f64,
    pub due_date: NaiveDate,
    pub darf_code: i32,
    pub sales: Vec<SaleResult>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaxWarning {
    CostUnknown { ticker: String, date: NaiveDate },
    /// Venda de cripto em moeda que não é BRL (exchange estrangeira): outra regra, não apurada aqui.
    CryptoSalesNonBrl { period: String },
    /// Venda no exterior de antes da Lei 14.754 (2024): não apurada.
    ForeignSales { year: i32 },
    /// Operação ou provento no exterior sem cotação do câmbio na data: ficou de fora da apuração anual.
    ForeignFxMissing { year: i32 },
    /// Operação ou provento no exterior convertido pelo câmbio de mercado (a PTAX do dia faltava): o valor do ano é aproximado.
    ForeignFxApproximate { year: i32 },
    /// Opção exercida: o resultado não é apurado aqui (o prêmio vai pro custo da ação).
    OptionExercised { ticker: String, date: NaiveDate },
}

/// Prejuízo acumulado de uma gaveta depois do último mês apurado, e quanto
/// de imposto ele abateria se virasse lucro tributável na mesma gaveta.
#[derive(Clone, Debug, Serialize)]
pub struct LossBalance {
    pub bucket: Bucket,
    pub balance: f64,
    /// Alíquota vigente no último mês em que a gaveta teve vendas.
    pub rate: f64,
    pub tax_saved: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Assessment {
    pub months: Vec<MonthAssessment>,
    /// Cripto: DARF próprio (código 4600), fora de `months`.
    pub crypto_months: Vec<CryptoMonth>,
    /// Exterior (Lei 14.754): apuração anual, preenchida pelo comando (precisa de câmbio); vazia na apuração pura.
    pub foreign_years: Vec<ForeignYear>,
    pub warnings: Vec<TaxWarning>,
    /// Só gavetas com saldo de prejuízo > 0.
    pub loss_balances: Vec<LossBalance>,
}

#[derive(Default)]
struct DayGroup {
    buy_qty: f64,
    buy_cost: f64,
    sell_qty: f64,
    sell_gross: f64,
    sell_proceeds: f64,
    sell_tx: Vec<i32>,
}

fn period_of(date: NaiveDate) -> String {
    format!("{:04}-{:02}", date.year(), date.month())
}

/// Apura mês a mês o imposto de renda variável (B3) a partir das compras e
/// vendas. `trades` pode vir em qualquer ordem e conter classes fora do
/// escopo (viram avisos ou são ignoradas).
///
/// Soma o resultado das opções encerradas: swing na gaveta comum e day trade
/// na sua, **sem** isenção dos R$ 20 mil (só vale pra venda de ações).
/// IRRF: o day trade retém 1% do ganho, como nas ações; o do swing (0,005%
/// sobre a venda) **não** é estimado, porque `sale_value` fica 0, então o
/// imposto de swing pode sair um pouco acima do real.
/// Prejuízo de opção compensa na mesma gaveta, inclusive contra ações.
pub fn assess_with_options(
    trades: &[Trade],
    options: &[OptionResult],
    rules: &RuleSet,
) -> Result<Assessment, AppError> {
    let mut warnings = Vec::new();
    collect_out_of_scope_warnings(trades, &mut warnings);

    let mut in_scope: Vec<&Trade> = trades
        .iter()
        .filter(|t| {
            matches!(
                treatment(&t.asset_class, &t.ticker),
                TaxTreatment::StockExemptEligible | TaxTreatment::CommonNoExemption | TaxTreatment::Fii
            )
        })
        .collect();
    in_scope.sort_by(|a, b| (a.asset_id, a.date, a.tx_id).cmp(&(b.asset_id, b.date, b.tx_id)));

    // 1) Por ativo, dia a dia: casa day trade por corretora, o resto mexe
    //    no custo médio (compras antes das vendas dentro do mesmo dia).
    let mut sales: Vec<SaleResult> = Vec::new();
    let mut by_asset: BTreeMap<i32, Vec<&Trade>> = BTreeMap::new();
    for t in in_scope {
        by_asset.entry(t.asset_id).or_default().push(t);
    }

    for asset_trades in by_asset.values() {
        let first = asset_trades[0];
        let kind = treatment(&first.asset_class, &first.ticker);
        let unit = is_unit(&first.asset_class, &first.ticker);
        let mut pos = Position::default();

        let mut by_day: BTreeMap<NaiveDate, Vec<&Trade>> = BTreeMap::new();
        for t in asset_trades {
            by_day.entry(t.date).or_default().push(t);
        }

        for (date, day_trades) in by_day {
            // Eventos corporativos mexem na posição antes dos negócios do dia.
            for t in &day_trades {
                match t.side {
                    Side::Split => {
                        pos.quantity *= t.quantity;
                    }
                    Side::Bonus => {
                        pos.quantity += t.quantity;
                        pos.cost += t.gross_value;
                    }
                    Side::Buy | Side::Sell => {}
                }
            }
            let mut groups: BTreeMap<Option<i32>, DayGroup> = BTreeMap::new();
            for t in day_trades.iter().filter(|t| matches!(t.side, Side::Buy | Side::Sell)) {
                let g = groups.entry(t.custodia_id).or_default();
                match t.side {
                    Side::Split | Side::Bonus => {}
                    Side::Buy => {
                        g.buy_qty += t.quantity;
                        g.buy_cost += t.gross_value + t.fee;
                    }
                    Side::Sell => {
                        g.sell_qty += t.quantity;
                        g.sell_gross += t.gross_value;
                        g.sell_proceeds += t.gross_value - t.fee;
                        g.sell_tx.push(t.tx_id);
                    }
                }
            }

            let mut swing_buys: Vec<(f64, f64)> = Vec::new();
            let mut swing_sells: Vec<(i32, f64, f64, f64)> = Vec::new();

            for g in groups.values() {
                let matched = g.buy_qty.min(g.sell_qty);
                if matched > EPS {
                    let buy_share = matched / g.buy_qty;
                    let sell_share = matched / g.sell_qty;
                    let cost = g.buy_cost * buy_share;
                    let proceeds = g.sell_proceeds * sell_share;
                    sales.push(SaleResult {
                        tx_id: g.sell_tx[0],
                        portfolio_id: first.portfolio_id,
                        asset_id: first.asset_id,
                        ticker: first.ticker.clone(),
                        date,
                        bucket: if kind == TaxTreatment::Fii { Bucket::Fii } else { Bucket::DayTrade },
                        is_unit: unit,
                        exemption_eligible: false,
                        quantity: matched,
                        sale_value: g.sell_gross * sell_share,
                        proceeds,
                        cost,
                        gain: proceeds - cost,
                        cost_unknown: false,
                    });
                }
                if g.buy_qty - matched > EPS {
                    swing_buys.push((g.buy_qty - matched, g.buy_cost * (1.0 - matched / g.buy_qty)));
                }
                if g.sell_qty - matched > EPS {
                    let rest = 1.0 - matched / g.sell_qty;
                    swing_sells.push((
                        g.sell_tx[0],
                        g.sell_qty - matched,
                        g.sell_gross * rest,
                        g.sell_proceeds * rest,
                    ));
                }
            }

            for (qty, cost) in swing_buys {
                pos.quantity += qty;
                pos.cost += cost;
            }
            for (tx_id, qty, gross, proceeds) in swing_sells {
                let (cost, covered) = take_from_position(&mut pos, qty);
                let cost_unknown = covered + EPS < qty;
                let share = if qty > EPS { covered / qty } else { 0.0 };
                if cost_unknown {
                    warnings.push(TaxWarning::CostUnknown { ticker: first.ticker.clone(), date });
                }
                let proceeds = proceeds * share;
                sales.push(SaleResult {
                    tx_id,
                    portfolio_id: first.portfolio_id,
                    asset_id: first.asset_id,
                    ticker: first.ticker.clone(),
                    date,
                    bucket: if kind == TaxTreatment::Fii { Bucket::Fii } else { Bucket::Common },
                    is_unit: unit,
                    exemption_eligible: kind == TaxTreatment::StockExemptEligible,
                    quantity: covered,
                    sale_value: gross * share,
                    proceeds,
                    cost,
                    gain: proceeds - cost,
                    cost_unknown,
                });
            }
        }
    }

    // `portfolio_id` por venda: o ativo pode estar em mais de uma carteira do
    // Workspace; usa o da própria transação de venda.
    let tx_portfolio: HashMap<i32, i32> = trades.iter().map(|t| (t.tx_id, t.portfolio_id)).collect();
    for s in &mut sales {
        if let Some(p) = tx_portfolio.get(&s.tx_id) {
            s.portfolio_id = *p;
        }
    }

    // Opções encerradas: uma operação cada. `tx_id` e `asset_id` negativos (o id
    // da posição com o sinal trocado) nunca colidem com os reais.
    for option in options {
        if option.exercised {
            warnings.push(TaxWarning::OptionExercised { ticker: option.ticker.clone(), date: option.date });
            continue;
        }
        sales.push(SaleResult {
            tx_id: -option.position_id,
            portfolio_id: option.portfolio_id,
            asset_id: -option.position_id,
            ticker: option.ticker.clone(),
            date: option.date,
            bucket: if option.day_trade { Bucket::DayTrade } else { Bucket::Common },
            is_unit: false,
            exemption_eligible: false,
            quantity: option.quantity,
            sale_value: 0.0,
            proceeds: option.proceeds,
            cost: option.cost,
            gain: option.proceeds - option.cost,
            cost_unknown: false,
        });
    }

    // 2) Mês a mês, em ordem: isenção, gavetas, compensação, IRRF, mínimo.
    let mut by_month: BTreeMap<String, Vec<SaleResult>> = BTreeMap::new();
    for s in sales {
        by_month.entry(period_of(s.date)).or_default().push(s);
    }

    let mut loss: HashMap<Bucket, f64> = HashMap::new();
    let mut irrf_credit = 0.0;
    let mut carried = 0.0;
    let mut months = Vec::new();

    for (period, month_sales) in by_month {
        let first_day = NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d")
            .map_err(|_| AppError::InvalidGuard(format!("invalid period {period}")))?;

        let limit = rules.get(rules::STOCK_EXEMPTION_MONTHLY_SALES, first_day)?;
        let stock_sales_total: f64 = month_sales
            .iter()
            .filter(|s| s.exemption_eligible)
            .map(|s| s.sale_value)
            .sum();
        let stock_exempt = stock_sales_total <= limit + EPS;

        // Mês isento: lucro das ações comuns não é tributado e não consome
        // prejuízo; prejuízo delas continua indo pra gaveta comum (prática de
        // mercado — ver PHASE.md Fase 24).
        let eligible_net: f64 = month_sales
            .iter()
            .filter(|s| s.exemption_eligible)
            .map(|s| s.gain)
            .sum();
        let exempt_gain = if stock_exempt && eligible_net > 0.0 { eligible_net } else { 0.0 };

        let mut buckets = Vec::new();
        let mut gross_tax = 0.0;
        for bucket in [Bucket::Common, Bucket::DayTrade, Bucket::Fii] {
            let in_bucket: Vec<&SaleResult> = month_sales.iter().filter(|s| s.bucket == bucket).collect();
            if in_bucket.is_empty() {
                continue;
            }
            let mut result: f64 = in_bucket.iter().map(|s| s.gain).sum();
            if bucket == Bucket::Common {
                result -= exempt_gain;
            }
            let carried_loss = loss.entry(bucket).or_insert(0.0);
            let (loss_used, taxable) = if result > 0.0 {
                let used = result.min(*carried_loss);
                *carried_loss -= used;
                (used, result - used)
            } else {
                *carried_loss += -result;
                (0.0, 0.0)
            };
            let rate_key = match bucket {
                Bucket::Common => rules::COMMON_RATE,
                Bucket::DayTrade => rules::DAY_TRADE_RATE,
                Bucket::Fii => rules::FII_RATE,
            };
            let rate = rules.get(rate_key, first_day)?;
            let tax = taxable * rate;
            gross_tax += tax;
            buckets.push(BucketResult {
                bucket,
                result,
                loss_used,
                loss_carried: *carried_loss,
                taxable,
                rate,
                tax,
            });
        }

        // IRRF estimado: 0,005% do valor de venda das operações comuns/FII;
        // 1% do ganho positivo de day trade (por venda casada).
        let irrf_common = rules.get(rules::IRRF_COMMON_RATE, first_day)?;
        let irrf_dt = rules.get(rules::IRRF_DAY_TRADE_RATE, first_day)?;
        let irrf_withheld: f64 = month_sales
            .iter()
            .map(|s| match s.bucket {
                Bucket::DayTrade => s.gain.max(0.0) * irrf_dt,
                _ => s.sale_value * irrf_common,
            })
            .sum();
        irrf_credit += irrf_withheld;
        let irrf_used = gross_tax.min(irrf_credit);
        irrf_credit -= irrf_used;

        let minimum = rules.get(rules::DARF_MINIMUM, first_day)?;
        let carried_in = carried;
        let payable = gross_tax - irrf_used + carried_in;
        let (due, carried_out) = if payable + EPS < minimum { (0.0, payable) } else { (payable, 0.0) };
        carried = carried_out;

        months.push(MonthAssessment {
            period: period.clone(),
            stock_sales_total,
            stock_exempt,
            exempt_gain,
            buckets,
            gross_tax,
            irrf_withheld,
            irrf_used,
            carried_in,
            due,
            carried_out,
            due_date: darf_due_date(first_day.year(), first_day.month()),
            darf_code: rules.get(rules::DARF_CODE_B3, first_day)? as i32,
            sales: month_sales,
        });
    }

    // Saldo atual = `loss_carried` do último mês em que cada gaveta apareceu.
    let mut loss_balances = Vec::new();
    for bucket in [Bucket::Common, Bucket::DayTrade, Bucket::Fii] {
        let last = months
            .iter()
            .rev()
            .find_map(|m: &MonthAssessment| m.buckets.iter().find(|b| b.bucket == bucket));
        if let Some(b) = last {
            if b.loss_carried > EPS {
                loss_balances.push(LossBalance {
                    bucket,
                    balance: b.loss_carried,
                    rate: b.rate,
                    tax_saved: b.loss_carried * b.rate,
                });
            }
        }
    }

    let (crypto_months, crypto_warnings) = assess_crypto(trades, rules)?;
    warnings.extend(crypto_warnings);

    Ok(Assessment { months, crypto_months, foreign_years: Vec::new(), warnings, loss_balances })
}

/// `tx_id` reservado da venda hipotética (maior que qualquer id real, então
/// ela é a última do dia).
pub const SIMULATED_TX_ID: i32 = i32::MAX;

/// Efeito de uma venda hipotética na apuração.
#[derive(Clone, Debug, Serialize)]
pub struct SaleSimulation {
    /// Mês da venda, `YYYY-MM`.
    pub period: String,
    /// Partes da venda simulada (duas se parte casar como day trade).
    pub sales: Vec<SaleResult>,
    /// Imposto bruto do mês da venda sem e com ela.
    pub month_tax_before: f64,
    pub month_tax_after: f64,
    /// Variação do imposto bruto somando todos os meses — pega o efeito nos
    /// meses seguintes, que perdem (ou ganham) prejuízo a compensar.
    pub total_tax_delta: f64,
    /// Prejuízo acumulado que a venda consome no mês dela.
    pub loss_used: f64,
    /// Lucro que a venda deixa isento no mês (limite de vendas de ações).
    pub exempt_gain: f64,
    pub loss_balances_before: Vec<LossBalance>,
    pub loss_balances_after: Vec<LossBalance>,
}

/// Simula uma venda: apura as transações reais com e sem `sale` e compara.
/// Usa `custodia_id` da própria venda — com `None` ela nunca casa como day
/// trade com compra do mesmo dia (a simulação é de swing trade).
///
/// As opções encerradas entram na base: o prejuízo (ou o ganho) delas muda
/// quanto a venda consome de compensação.
pub fn simulate_sale_with_options(
    trades: &[Trade],
    options: &[OptionResult],
    rules: &RuleSet,
    sale: Trade,
) -> Result<SaleSimulation, AppError> {
    let mut sale = sale;
    sale.tx_id = SIMULATED_TX_ID;
    sale.side = Side::Sell;
    let period = period_of(sale.date);

    let before = assess_with_options(trades, options, rules)?;
    let mut with_sale = trades.to_vec();
    with_sale.push(sale);
    let after = assess_with_options(&with_sale, options, rules)?;

    let after_month = after
        .months
        .iter()
        .find(|m| m.period == period)
        .ok_or_else(|| AppError::InvalidGuard("this asset has no recorded purchases to sell".to_string()))?;
    let sales: Vec<SaleResult> = after_month
        .sales
        .iter()
        .filter(|s| s.tx_id == SIMULATED_TX_ID)
        .cloned()
        .collect();
    // Sem compras registradas a venda sai com quantidade coberta 0.
    if sales.iter().all(|s| s.quantity <= EPS) {
        return Err(AppError::InvalidGuard(
            "this asset has no recorded purchases to sell".to_string(),
        ));
    }

    let before_month = before.months.iter().find(|m| m.period == period);
    let loss_used_of = |m: Option<&MonthAssessment>| -> f64 {
        m.map(|m| m.buckets.iter().map(|b| b.loss_used).sum()).unwrap_or(0.0)
    };
    let total = |a: &Assessment| -> f64 { a.months.iter().map(|m| m.gross_tax).sum() };

    Ok(SaleSimulation {
        period,
        sales,
        month_tax_before: before_month.map(|m| m.gross_tax).unwrap_or(0.0),
        month_tax_after: after_month.gross_tax,
        total_tax_delta: total(&after) - total(&before),
        loss_used: loss_used_of(Some(after_month)) - loss_used_of(before_month),
        exempt_gain: after_month.exempt_gain - before_month.map(|m| m.exempt_gain).unwrap_or(0.0),
        loss_balances_before: before.loss_balances,
        loss_balances_after: after.loss_balances,
    })
}

/// Venda de ativo no exterior antes da Lei 14.754/2023: só avisa. De 2024 em diante a apuração anual é de
/// `domain::tax::foreign`.
fn collect_out_of_scope_warnings(trades: &[Trade], warnings: &mut Vec<TaxWarning>) {
    let mut foreign_years: BTreeSet<i32> = BTreeSet::new();
    for t in trades.iter().filter(|t| t.side == Side::Sell && t.date.year() < foreign::FIRST_YEAR) {
        if treatment(&t.asset_class, &t.ticker) == TaxTreatment::ForeignWarning {
            foreign_years.insert(t.date.year());
        }
    }
    for year in foreign_years {
        warnings.push(TaxWarning::ForeignSales { year });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax::rules::test_rule_set;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    /// Apuração só de ações (sem opções): o que a maioria dos testes exercita.
    fn assess(trades: &[Trade], rules: &RuleSet) -> Result<Assessment, AppError> {
        assess_with_options(trades, &[], rules)
    }

    fn simulate_sale(trades: &[Trade], rules: &RuleSet, sale: Trade) -> Result<SaleSimulation, AppError> {
        simulate_sale_with_options(trades, &[], rules, sale)
    }

    struct Builder {
        trades: Vec<Trade>,
    }

    impl Builder {
        fn new() -> Self {
            Self { trades: Vec::new() }
        }

        #[allow(clippy::too_many_arguments)]
        fn trade(
            mut self,
            asset_id: i32,
            ticker: &str,
            class: &str,
            date: &str,
            side: Side,
            quantity: f64,
            gross: f64,
            fee: f64,
        ) -> Self {
            let tx_id = self.trades.len() as i32 + 1;
            self.trades.push(Trade {
                tx_id,
                portfolio_id: 1,
                asset_id,
                ticker: ticker.to_string(),
                asset_class: class.to_string(),
                currency: "BRL".to_string(),
                custodia_id: Some(1),
                date: d(date),
                side,
                quantity,
                gross_value: gross,
                fee,
            });
            self
        }

        fn buy(self, asset_id: i32, ticker: &str, class: &str, date: &str, qty: f64, gross: f64, fee: f64) -> Self {
            self.trade(asset_id, ticker, class, date, Side::Buy, qty, gross, fee)
        }

        fn sell(self, asset_id: i32, ticker: &str, class: &str, date: &str, qty: f64, gross: f64, fee: f64) -> Self {
            self.trade(asset_id, ticker, class, date, Side::Sell, qty, gross, fee)
        }

        fn assess(self) -> Assessment {
            assess(&self.trades, &test_rule_set()).unwrap()
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn receita_official_example() {
        // Página "Cálculo e Pagamento do Imposto" da Receita: compra 50.000 +
        // 15 de taxas, venda 60.000 − 18 de taxas (fev/2024) → ganho 9.967,
        // imposto 1.495,05, IRRF 3,00 → 1.492,05 a pagar.
        let a = Builder::new()
            .buy(1, "PETR4", "acao_br", "2024-01-10", 1000.0, 50000.0, 15.0)
            .sell(1, "PETR4", "acao_br", "2024-02-15", 1000.0, 60000.0, 18.0)
            .assess();

        let feb = &a.months[0];
        assert_eq!(feb.period, "2024-02");
        assert!(!feb.stock_exempt);
        assert!(close(feb.sales[0].gain, 9967.0));
        assert!(close(feb.gross_tax, 1495.05));
        assert!(close(feb.irrf_withheld, 3.0));
        assert!(close(feb.due, 1492.05));
        assert_eq!(feb.darf_code, 6015);
        assert_eq!(feb.due_date, d("2024-03-28"));
    }

    #[test]
    fn stock_sales_up_to_limit_are_exempt() {
        let a = Builder::new()
            .buy(1, "PETR4", "acao_br", "2026-01-05", 100.0, 10000.0, 0.0)
            .sell(1, "PETR4", "acao_br", "2026-02-05", 100.0, 18000.0, 0.0)
            .assess();

        let m = &a.months[0];
        assert!(m.stock_exempt);
        assert!(close(m.exempt_gain, 8000.0));
        assert!(close(m.gross_tax, 0.0));
        assert!(close(m.due, 0.0));
    }

    #[test]
    fn etf_is_never_exempt() {
        let a = Builder::new()
            .buy(1, "BOVA11", "etf_br", "2026-01-05", 100.0, 10000.0, 0.0)
            .sell(1, "BOVA11", "etf_br", "2026-02-05", 100.0, 12000.0, 0.0)
            .assess();

        assert!(close(a.months[0].gross_tax, 300.0));
    }

    #[test]
    fn unit_is_taxed_and_does_not_count_towards_stock_limit() {
        // TAEE11 vendida por só R$ 5 mil: tributada mesmo assim. E os R$ 18
        // mil de PETR4 continuam isentos — a unit não soma no limite.
        let a = Builder::new()
            .buy(1, "TAEE11", "acao_br", "2026-01-05", 100.0, 4000.0, 0.0)
            .buy(2, "PETR4", "acao_br", "2026-01-05", 100.0, 10000.0, 0.0)
            .sell(1, "TAEE11", "acao_br", "2026-02-05", 100.0, 5000.0, 0.0)
            .sell(2, "PETR4", "acao_br", "2026-02-05", 100.0, 18000.0, 0.0)
            .assess();

        let m = &a.months[0];
        assert!(close(m.stock_sales_total, 18000.0));
        assert!(m.stock_exempt);
        let unit_sale = m.sales.iter().find(|s| s.ticker == "TAEE11").unwrap();
        assert!(unit_sale.is_unit);
        assert!(close(m.gross_tax, 150.0)); // 15% × 1.000 de lucro da unit
    }

    #[test]
    fn day_trade_is_matched_and_taxed_at_20_percent() {
        // Compra 100 e vende 100 no mesmo dia/corretora: day trade, sem
        // isenção mesmo com venda pequena.
        let a = Builder::new()
            .buy(1, "PETR4", "acao_br", "2026-03-10", 100.0, 3000.0, 0.0)
            .sell(1, "PETR4", "acao_br", "2026-03-10", 100.0, 3500.0, 0.0)
            .assess();

        let m = &a.months[0];
        assert_eq!(m.sales[0].bucket, Bucket::DayTrade);
        assert!(close(m.gross_tax, 100.0)); // 20% × 500
        assert!(close(m.irrf_withheld, 5.0)); // 1% × 500
        assert!(close(m.due, 95.0));
    }

    #[test]
    fn partial_day_trade_leaves_rest_in_average_cost() {
        // Tinha 100 a R$ 10; no dia compra 50 a R$ 12 e vende 80 a R$ 15.
        // 50 casados = day trade (ganho 50 × 3 = 150). Os outros 30 vendidos
        // saem do custo médio antigo (R$ 10) → ganho comum 30 × 5 = 150.
        let a = Builder::new()
            .buy(1, "VALE3", "acao_br", "2026-04-01", 100.0, 1000.0, 0.0)
            .buy(1, "VALE3", "acao_br", "2026-04-20", 50.0, 600.0, 0.0)
            .sell(1, "VALE3", "acao_br", "2026-04-20", 80.0, 1200.0, 0.0)
            .assess();

        let m = &a.months[0];
        let dt = m.sales.iter().find(|s| s.bucket == Bucket::DayTrade).unwrap();
        let common = m.sales.iter().find(|s| s.bucket == Bucket::Common).unwrap();
        assert!(close(dt.quantity, 50.0));
        assert!(close(dt.gain, 150.0));
        assert!(close(common.quantity, 30.0));
        assert!(close(common.gain, 150.0));
    }

    #[test]
    fn different_brokers_same_day_is_not_day_trade() {
        let mut trades = Builder::new()
            .buy(1, "PETR4", "acao_br", "2026-03-01", 100.0, 3000.0, 0.0)
            .buy(1, "PETR4", "acao_br", "2026-03-10", 100.0, 3000.0, 0.0)
            .sell(1, "PETR4", "acao_br", "2026-03-10", 100.0, 3500.0, 0.0)
            .trades;
        trades[2].custodia_id = Some(2);

        let a = assess(&trades, &test_rule_set()).unwrap();
        assert!(a.months[0].sales.iter().all(|s| s.bucket == Bucket::Common));
    }

    #[test]
    fn fii_has_own_bucket_and_rate() {
        let a = Builder::new()
            .buy(1, "HGLG11", "fii", "2026-01-05", 10.0, 1500.0, 0.0)
            .sell(1, "HGLG11", "fii", "2026-02-05", 10.0, 1600.0, 0.0)
            .assess();

        let m = &a.months[0];
        assert_eq!(m.buckets[0].bucket, Bucket::Fii);
        assert!(close(m.gross_tax, 20.0)); // 20% × 100, sem isenção
    }

    #[test]
    fn loss_carries_forward_within_bucket_only() {
        // Jan: prejuízo de 1.000 em ETF (gaveta comum). Fev: lucro de 1.000
        // em FII (outra gaveta, não compensa) e 1.500 em ETF (compensa 1.000).
        let a = Builder::new()
            .buy(1, "BOVA11", "etf_br", "2026-01-02", 100.0, 10000.0, 0.0)
            .sell(1, "BOVA11", "etf_br", "2026-01-20", 100.0, 9000.0, 0.0)
            .buy(2, "HGLG11", "fii", "2026-01-02", 10.0, 1000.0, 0.0)
            .sell(2, "HGLG11", "fii", "2026-02-10", 10.0, 2000.0, 0.0)
            .buy(3, "IVVB11", "etf_br", "2026-01-02", 10.0, 3000.0, 0.0)
            .sell(3, "IVVB11", "etf_br", "2026-02-10", 10.0, 4500.0, 0.0)
            .assess();

        let feb = &a.months[1];
        let common = feb.buckets.iter().find(|b| b.bucket == Bucket::Common).unwrap();
        let fii = feb.buckets.iter().find(|b| b.bucket == Bucket::Fii).unwrap();
        assert!(close(common.loss_used, 1000.0));
        assert!(close(common.taxable, 500.0));
        assert!(close(fii.loss_used, 0.0));
        assert!(close(fii.taxable, 1000.0));
    }

    #[test]
    fn loss_balances_report_remaining_loss_per_bucket() {
        // Jan: prejuízo de 1.000 em ETF e de 300 em FII. Fev: lucro de 400
        // em ETF (consome 400, sobram 600) → FII segue com 300 (último mês
        // dela foi janeiro). Economia = saldo × alíquota da gaveta.
        let a = Builder::new()
            .buy(1, "BOVA11", "etf_br", "2026-01-02", 100.0, 10000.0, 0.0)
            .sell(1, "BOVA11", "etf_br", "2026-01-20", 100.0, 9000.0, 0.0)
            .buy(2, "HGLG11", "fii", "2026-01-02", 10.0, 1000.0, 0.0)
            .sell(2, "HGLG11", "fii", "2026-01-25", 10.0, 700.0, 0.0)
            .buy(3, "IVVB11", "etf_br", "2026-01-02", 10.0, 3000.0, 0.0)
            .sell(3, "IVVB11", "etf_br", "2026-02-10", 10.0, 3400.0, 0.0)
            .assess();

        let common = a.loss_balances.iter().find(|l| l.bucket == Bucket::Common).unwrap();
        let fii = a.loss_balances.iter().find(|l| l.bucket == Bucket::Fii).unwrap();
        assert!(close(common.balance, 600.0));
        assert!(close(common.tax_saved, 600.0 * common.rate));
        assert!(close(fii.balance, 300.0));
        assert!(close(fii.tax_saved, 300.0 * fii.rate));
        assert!(a.loss_balances.iter().all(|l| l.bucket != Bucket::DayTrade));
    }

    fn hypothetical(b: &Builder, asset_id: i32, ticker: &str, class: &str, date: &str, qty: f64, gross: f64) -> Trade {
        let mut t = b.trades[0].clone();
        t.asset_id = asset_id;
        t.ticker = ticker.to_string();
        t.asset_class = class.to_string();
        t.date = d(date);
        t.quantity = qty;
        t.gross_value = gross;
        t.fee = 0.0;
        t
    }

    #[test]
    fn simulate_sale_uses_carried_loss_and_reports_tax_delta() {
        // Jan: prejuízo de 1.000 em ETF (comum). Simula vender 10 IVVB11
        // (custo 3.000) por 4.500 em fev: lucro 1.500, compensa 1.000, paga
        // 15% × 500 = 75.
        let b = Builder::new()
            .buy(1, "BOVA11", "etf_br", "2026-01-02", 100.0, 10000.0, 0.0)
            .sell(1, "BOVA11", "etf_br", "2026-01-20", 100.0, 9000.0, 0.0)
            .buy(3, "IVVB11", "etf_br", "2026-01-02", 10.0, 3000.0, 0.0);
        let sale = hypothetical(&b, 3, "IVVB11", "etf_br", "2026-02-10", 10.0, 4500.0);
        let sim = simulate_sale(&b.trades, &test_rule_set(), sale).unwrap();

        assert_eq!(sim.period, "2026-02");
        assert!(close(sim.sales[0].gain, 1500.0));
        assert!(close(sim.loss_used, 1000.0));
        assert!(close(sim.month_tax_before, 0.0));
        assert!(close(sim.month_tax_after, 75.0));
        assert!(close(sim.total_tax_delta, 75.0));
        assert_eq!(sim.loss_balances_before.len(), 1);
        assert!(sim.loss_balances_after.is_empty());
    }

    #[test]
    fn simulate_sale_of_stocks_under_exemption_pays_nothing() {
        let b = Builder::new().buy(2, "PETR4", "acao_br", "2026-01-02", 100.0, 3000.0, 0.0);
        let sale = hypothetical(&b, 2, "PETR4", "acao_br", "2026-02-10", 100.0, 5000.0);
        let sim = simulate_sale(&b.trades, &test_rule_set(), sale).unwrap();

        assert!(close(sim.month_tax_after, 0.0));
        assert!(close(sim.exempt_gain, 2000.0));
    }

    #[test]
    fn simulate_sale_without_purchases_is_an_error() {
        let b = Builder::new().buy(1, "BOVA11", "etf_br", "2026-01-02", 10.0, 1000.0, 0.0);
        let sale = hypothetical(&b, 9, "HGLG11", "fii", "2026-02-10", 10.0, 1500.0);
        assert!(simulate_sale(&b.trades, &test_rule_set(), sale).is_err());
    }

    #[test]
    fn loss_balances_empty_when_loss_fully_used() {
        let a = Builder::new()
            .buy(1, "BOVA11", "etf_br", "2026-01-02", 10.0, 1500.0, 0.0)
            .sell(1, "BOVA11", "etf_br", "2026-01-20", 10.0, 1000.0, 0.0)
            .buy(2, "IVVB11", "etf_br", "2026-01-02", 10.0, 1000.0, 0.0)
            .sell(2, "IVVB11", "etf_br", "2026-02-10", 10.0, 1700.0, 0.0)
            .assess();

        assert!(a.loss_balances.is_empty());
    }

    #[test]
    fn exempt_gain_does_not_consume_carried_loss() {
        // Jan: prejuízo de 500 em ETF. Fev: lucro isento de 2.000 em ações
        // (vendas < 20k) → prejuízo segue intacto.
        let a = Builder::new()
            .buy(1, "BOVA11", "etf_br", "2026-01-02", 10.0, 1500.0, 0.0)
            .sell(1, "BOVA11", "etf_br", "2026-01-20", 10.0, 1000.0, 0.0)
            .buy(2, "PETR4", "acao_br", "2026-01-02", 100.0, 3000.0, 0.0)
            .sell(2, "PETR4", "acao_br", "2026-02-10", 100.0, 5000.0, 0.0)
            .assess();

        let feb = &a.months[1];
        let common = feb.buckets.iter().find(|b| b.bucket == Bucket::Common).unwrap();
        assert!(close(common.loss_used, 0.0));
        assert!(close(common.loss_carried, 500.0));
    }

    #[test]
    fn tax_below_minimum_accumulates() {
        // Fev: 15% × 40 = 6 → abaixo de R$ 10, acumula. Mar: 15% × 40 = 6 +
        // 6 acumulado = 12 → DARF de 12 (IRRF de centavos ignorado no close).
        let a = Builder::new()
            .buy(1, "BOVA11", "etf_br", "2026-01-02", 20.0, 200.0, 0.0)
            .sell(1, "BOVA11", "etf_br", "2026-02-10", 10.0, 140.0, 0.0)
            .sell(1, "BOVA11", "etf_br", "2026-03-10", 10.0, 140.0, 0.0)
            .assess();

        let (feb, mar) = (&a.months[0], &a.months[1]);
        assert!(close(feb.due, 0.0));
        assert!(feb.carried_out > 5.9 && feb.carried_out < 6.0);
        assert!(mar.due > 11.9 && mar.due < 12.0);
        assert!(close(mar.carried_out, 0.0));
    }

    #[test]
    fn average_cost_resets_after_position_is_closed() {
        // Compra 10 a 100, vende tudo; recompra 10 a 50 e vende a 60 → o
        // custo da segunda venda é 500 (não a média histórica de 75).
        let a = Builder::new()
            .buy(1, "BOVA11", "etf_br", "2026-01-02", 10.0, 1000.0, 0.0)
            .sell(1, "BOVA11", "etf_br", "2026-01-10", 10.0, 1000.0, 0.0)
            .buy(1, "BOVA11", "etf_br", "2026-02-02", 10.0, 500.0, 0.0)
            .sell(1, "BOVA11", "etf_br", "2026-02-10", 10.0, 600.0, 0.0)
            .assess();

        assert!(close(a.months[1].sales[0].cost, 500.0));
        assert!(close(a.months[1].sales[0].gain, 100.0));
    }

    #[test]
    fn selling_more_than_known_flags_cost_unknown() {
        let a = Builder::new()
            .buy(1, "PETR4", "acao_br", "2026-01-02", 10.0, 300.0, 0.0)
            .sell(1, "PETR4", "acao_br", "2026-02-10", 20.0, 800.0, 0.0)
            .assess();

        let sale = &a.months[0].sales[0];
        assert!(sale.cost_unknown);
        assert!(close(sale.quantity, 10.0));
        assert!(close(sale.gain, 100.0)); // só os 10 com custo conhecido
        assert!(matches!(a.warnings[0], TaxWarning::CostUnknown { .. }));
    }

    #[test]
    fn crypto_is_assessed_apart_from_the_exchange_and_foreign_only_warns() {
        let mut b = Builder::new()
            .buy(1, "BTC", "cripto", "2026-01-02", 1.0, 30000.0, 0.0)
            .sell(1, "BTC", "cripto", "2026-02-10", 1.0, 40000.0, 0.0)
            .buy(2, "AAPL", "acao_internacional", "2023-01-02", 1.0, 1000.0, 0.0)
            .sell(2, "AAPL", "acao_internacional", "2023-02-10", 1.0, 1200.0, 0.0);
        b.trades[2].currency = "USD".to_string();
        b.trades[3].currency = "USD".to_string();
        let a = b.assess();

        assert!(a.months.is_empty(), "o DARF da B3 não leva a cripto");
        assert_eq!(a.crypto_months.len(), 1);
        assert!((a.crypto_months[0].tax - 1500.0).abs() < 1e-6, "15% de 10.000 de ganho, venda acima de R$ 35 mil");
        assert!(a.warnings.iter().any(|w| matches!(w, TaxWarning::ForeignSales { year: 2023 })));
    }

    #[test]
    fn a_foreign_sale_from_2024_on_is_left_to_the_annual_assessment() {
        let a = Builder::new()
            .buy(1, "AAPL", "acao_internacional", "2025-01-02", 1.0, 1000.0, 0.0)
            .sell(1, "AAPL", "acao_internacional", "2025-02-10", 1.0, 1200.0, 0.0)
            .assess();
        assert!(!a.warnings.iter().any(|w| matches!(w, TaxWarning::ForeignSales { .. })));
    }

    // -- Opções (Fase 18) --------------------------------------------------

    fn option(id: i32, date: &str, day_trade: bool, proceeds: f64, cost: f64) -> OptionResult {
        OptionResult {
            position_id: id,
            portfolio_id: 1,
            ticker: "PETRK600".to_string(),
            date: d(date),
            day_trade,
            quantity: 100.0,
            proceeds,
            cost,
            exercised: false,
        }
    }

    fn assess_options(b: Builder, options: &[OptionResult]) -> Assessment {
        assess_with_options(&b.trades, options, &test_rule_set()).unwrap()
    }

    #[test]
    fn option_gain_is_taxed_at_15_percent_with_no_exemption_and_no_swing_irrf() {
        // Ganho de 1.000 (receita 1.500, custo 500): mesmo sem nenhuma venda de ação
        // (que deixaria o mês "isento"), a opção swing paga 15 % e não gera IRRF estimado.
        let a = assess_options(Builder::new(), &[option(1, "2026-03-10", false, 1500.0, 500.0)]);

        let m = &a.months[0];
        assert!(close(m.stock_sales_total, 0.0));
        assert!(close(m.exempt_gain, 0.0));
        assert_eq!(m.buckets.len(), 1);
        assert_eq!(m.buckets[0].bucket, Bucket::Common);
        assert!(close(m.gross_tax, 150.0));
        assert!(close(m.irrf_withheld, 0.0));
        assert!(close(m.due, 150.0));
        assert_eq!(m.sales[0].tx_id, -1);
        assert!(!m.sales[0].exemption_eligible);
    }

    #[test]
    fn option_gain_is_taxed_even_when_the_stock_gain_of_the_month_is_exempt() {
        // Ação: venda de 4.000 (abaixo de 20 mil) com lucro 500 → isento. Opção: ganho 1.000.
        // Gaveta comum: (500 + 1.000) − 500 isento = 1.000 → 15 % = 150.
        let b = Builder::new()
            .buy(1, "PETR4", "acao_br", "2026-01-02", 100.0, 3500.0, 0.0)
            .sell(1, "PETR4", "acao_br", "2026-03-10", 100.0, 4000.0, 0.0);
        let a = assess_options(b, &[option(1, "2026-03-20", false, 1500.0, 500.0)]);

        let m = &a.months[0];
        assert!(m.stock_exempt);
        assert!(close(m.exempt_gain, 500.0));
        assert!(close(m.stock_sales_total, 4000.0)); // a opção não entra no limite
        assert!(close(m.buckets[0].taxable, 1000.0));
        assert!(close(m.gross_tax, 150.0));
    }

    #[test]
    fn an_option_loss_offsets_a_later_stock_gain_in_the_common_bucket() {
        // Jan: opção perde 500. Fev: ETF ganha 1.500 → compensa 500, tributa 1.000.
        let b = Builder::new()
            .buy(3, "IVVB11", "etf_br", "2026-01-02", 10.0, 3000.0, 0.0)
            .sell(3, "IVVB11", "etf_br", "2026-02-10", 10.0, 4500.0, 0.0);
        let a = assess_options(b, &[option(1, "2026-01-15", false, 0.0, 500.0)]);

        assert_eq!(a.months.len(), 2);
        assert!(close(a.months[0].gross_tax, 0.0));
        let feb = a.months[1].buckets.iter().find(|x| x.bucket == Bucket::Common).unwrap();
        assert!(close(feb.loss_used, 500.0));
        assert!(close(feb.taxable, 1000.0));
        assert!(a.loss_balances.is_empty());
    }

    #[test]
    fn a_day_trade_option_goes_to_the_day_trade_bucket_at_20_percent() {
        let a = assess_options(Builder::new(), &[option(1, "2026-03-10", true, 1000.0, 500.0)]);

        let m = &a.months[0];
        assert_eq!(m.buckets.len(), 1);
        assert_eq!(m.buckets[0].bucket, Bucket::DayTrade);
        assert!(close(m.gross_tax, 100.0)); // 20 % × 500
        assert!(close(m.irrf_withheld, 5.0)); // 1 % do ganho, como em qualquer day trade
        assert!(close(m.due, 95.0));
    }

    #[test]
    fn an_exercised_option_only_warns() {
        let mut exercised = option(1, "2026-03-10", false, 1500.0, 500.0);
        exercised.exercised = true;
        let a = assess_options(Builder::new(), &[exercised]);

        assert!(a.months.is_empty());
        assert!(matches!(&a.warnings[0], TaxWarning::OptionExercised { ticker, .. } if ticker == "PETRK600"));
    }

    #[test]
    fn assess_without_options_is_unchanged() {
        let build = || {
            Builder::new()
                .buy(3, "IVVB11", "etf_br", "2026-01-02", 10.0, 3000.0, 0.0)
                .sell(3, "IVVB11", "etf_br", "2026-02-10", 10.0, 4500.0, 0.0)
        };
        let plain = build().assess();
        let with_empty = assess_options(build(), &[]);
        assert_eq!(plain.months.len(), with_empty.months.len());
        assert!(close(plain.months[0].gross_tax, with_empty.months[0].gross_tax));
    }

    #[test]
    fn simulate_sale_sees_the_loss_from_closed_options() {
        // Jan: a opção perde 1.000. Simula vender 10 IVVB11 (custo 3.000) por 4.500 em fev:
        // lucro 1.500, compensa 1.000 da opção, paga 15 % × 500 = 75.
        let b = Builder::new().buy(3, "IVVB11", "etf_br", "2026-01-02", 10.0, 3000.0, 0.0);
        let sale = hypothetical(&b, 3, "IVVB11", "etf_br", "2026-02-10", 10.0, 4500.0);
        let options = [option(1, "2026-01-15", false, 0.0, 1000.0)];
        let sim = simulate_sale_with_options(&b.trades, &options, &test_rule_set(), sale).unwrap();

        assert!(close(sim.loss_used, 1000.0));
        assert!(close(sim.month_tax_after, 75.0));
        assert_eq!(sim.loss_balances_before.len(), 1);
        assert!(sim.loss_balances_after.is_empty());
    }
}
