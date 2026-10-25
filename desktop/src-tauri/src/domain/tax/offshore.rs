// IR sobre entidade controlada no exterior (Lei 14.754/2023): o lucro da offshore é tributado todo 31/12, mesmo sem
// distribuição, e entra na apuração do dono na proporção da participação. O app não faz a contabilidade da entidade: o
// usuário informa o resultado do exercício (e o imposto pago lá fora) na moeda dela.
//
// Regras (a alíquota vem de `tax_rules`, o resto é código):
// - Só vale para anos que já fecharam (31/12 passou); o ano corrente fica "aberto" e não entra, com aviso.
// - Lucro atribuível = lucro × participação × câmbio de 31/12. O câmbio é a PTAX de venda do Banco Central (art. 5º, como
//   anotado na Sessão 188); dia sem PTAX usa a última anterior (`PtaxSeries`). Sem PTAX vale o câmbio de mercado e o ano
//   avisa que é aproximado; sem nenhum dos dois a linha fica de fora e avisa. Entidade em BRL vale 1.
// - O prejuízo de uma entidade só abate lucros futuros da própria entidade.
// - Imposto = base × alíquota (15%). O imposto pago lá fora (× participação) abate o devido, até o limite dele,
//   convertido pela PTAX de compra de 31/12 (pressuposto; sem ela, a de venda do mesmo dia).
// - Não há DARF: vai na declaração anual.
//
// ATENÇÃO: as regras vêm de fontes secundárias, as mesmas do IR de exterior; conferir na Receita antes de confiar.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use serde::Serialize;

use super::assessment::TaxWarning;
use super::foreign::{convert, RateToBrl};
use super::rules::{self, RuleSet};
use crate::domain::ptax::{RateKind, RateSource};
use crate::error::AppError;

/// Uma entidade controlada no exterior, como saiu de `offshore_entities`.
#[derive(Clone, Debug)]
pub struct OffshoreEntity {
    pub id: i32,
    pub name: String,
    pub country: String,
    pub currency: String,
    /// De 0 (exclusive) a 100.
    pub ownership_percent: f64,
    /// Controlada ativa (art. 6º): só é tributada quando o lucro é disponibilizado, não em 31/12.
    pub active_regime: bool,
}

/// O resultado do exercício de uma entidade, na moeda dela.
#[derive(Clone, Debug)]
pub struct OffshoreResult {
    pub entity_id: i32,
    pub year: i32,
    pub profit: f64,
    pub tax_paid_abroad: f64,
}

/// Uma entidade num ano, já convertida e com a compensação de prejuízo aplicada.
#[derive(Clone, Debug, Serialize)]
pub struct OffshoreLine {
    pub entity_id: i32,
    pub entity_name: String,
    pub country: String,
    pub currency: String,
    pub ownership_percent: f64,
    /// Lucro do exercício na moeda da entidade (100% da entidade).
    pub profit: f64,
    /// Câmbio de 31/12 (venda) e de onde veio.
    pub fx_rate: f64,
    pub fx_source: RateSource,
    /// Lucro × participação × câmbio, em BRL (negativo = prejuízo).
    pub profit_share_brl: f64,
    /// Prejuízo dos anos anteriores desta entidade disponível para abater.
    pub loss_carried_in: f64,
    pub taxable_base: f64,
    pub tax_before_credit: f64,
    /// Imposto pago lá fora × participação, em BRL (o que poderia virar crédito).
    pub tax_paid_abroad_brl: f64,
    pub credit_used: f64,
    pub tax_due: f64,
    pub loss_carried_out: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct OffshoreYear {
    pub year: i32,
    pub rate: f64,
    pub profit_share_brl: f64,
    pub taxable_base: f64,
    pub tax_before_credit: f64,
    pub credit_used: f64,
    pub tax_due: f64,
    pub lines: Vec<OffshoreLine>,
}

/// Soma a partir de +0,0: o `sum()` de `f64` vazio dá −0,0 e a tela mostraria "-R$ 0,00".
fn total(lines: &[OffshoreLine], f: impl Fn(&OffshoreLine) -> f64) -> f64 {
    lines.iter().fold(0.0, |acc, line| acc + f(line))
}

/// Apura ano a ano o IR das entidades controladas no exterior. `today` decide quais anos já fecharam. Linha sem câmbio
/// de 31/12 fica de fora e gera aviso; ano que ainda não fechou gera `OffshoreYearOpen`.
pub fn assess_offshore(
    entities: &[OffshoreEntity],
    results: &[OffshoreResult],
    rules: &RuleSet,
    today: NaiveDate,
    rate: RateToBrl,
) -> Result<(Vec<OffshoreYear>, Vec<TaxWarning>), AppError> {
    let mut warnings = Vec::new();
    let mut open_years = std::collections::BTreeSet::new();
    let mut approximate_years = std::collections::BTreeSet::new();
    let by_id: BTreeMap<i32, &OffshoreEntity> = entities.iter().map(|e| (e.id, e)).collect();

    // Entidade ativa (art. 6º) não é tributada em 31/12: o resultado dela fica só como histórico.
    let mut ordered: Vec<&OffshoreResult> =
        results.iter().filter(|r| by_id.get(&r.entity_id).is_some_and(|e| !e.active_regime)).collect();
    ordered.sort_by_key(|r| (r.year, r.entity_id));

    let mut loss_by_entity: BTreeMap<i32, f64> = BTreeMap::new();
    let mut lines_by_year: BTreeMap<i32, Vec<OffshoreLine>> = BTreeMap::new();
    for result in ordered {
        let entity = by_id[&result.entity_id];
        let year_end = NaiveDate::from_ymd_opt(result.year, 12, 31)
            .ok_or_else(|| AppError::InvalidInput(format!("invalid year {}", result.year)))?;
        if today <= year_end {
            open_years.insert(result.year);
            continue;
        }
        let Some((fx_rate, fx_source)) = convert(&entity.currency, year_end, RateKind::Sell, rate) else {
            warnings.push(TaxWarning::OffshoreFxMissing { year: result.year, entity: entity.name.clone() });
            continue;
        };
        // O imposto pago lá fora usa a compra; sem ela, a de venda do mesmo dia.
        let (withheld_rate, withheld_source) =
            convert(&entity.currency, year_end, RateKind::Buy, rate).unwrap_or((fx_rate, fx_source));
        if fx_source == RateSource::Market || (result.tax_paid_abroad > 0.0 && withheld_source == RateSource::Market) {
            approximate_years.insert(result.year);
        }

        let share = entity.ownership_percent / 100.0;
        let profit_share_brl = result.profit * share * fx_rate;
        let tax_paid_abroad_brl = result.tax_paid_abroad * share * withheld_rate;

        let loss_carried_in = loss_by_entity.get(&entity.id).copied().unwrap_or(0.0);
        let (taxable_base, loss_carried_out) = if profit_share_brl >= 0.0 {
            let used = loss_carried_in.min(profit_share_brl);
            (profit_share_brl - used, loss_carried_in - used)
        } else {
            (0.0, loss_carried_in - profit_share_brl)
        };
        loss_by_entity.insert(entity.id, loss_carried_out);

        let tax_rate = rules.get(rules::OFFSHORE_RATE, year_end)?;
        let tax_before_credit = taxable_base * tax_rate;
        let credit_used = tax_paid_abroad_brl.min(tax_before_credit);
        lines_by_year.entry(result.year).or_default().push(OffshoreLine {
            entity_id: entity.id,
            entity_name: entity.name.clone(),
            country: entity.country.clone(),
            currency: entity.currency.clone(),
            ownership_percent: entity.ownership_percent,
            profit: result.profit,
            fx_rate,
            fx_source,
            profit_share_brl,
            loss_carried_in,
            taxable_base,
            tax_before_credit,
            tax_paid_abroad_brl,
            credit_used,
            tax_due: tax_before_credit - credit_used,
            loss_carried_out,
        });
    }

    let mut years = Vec::new();
    for (year, lines) in lines_by_year {
        let year_end = NaiveDate::from_ymd_opt(year, 12, 31).expect("validated above");
        years.push(OffshoreYear {
            year,
            rate: rules.get(rules::OFFSHORE_RATE, year_end)?,
            profit_share_brl: total(&lines, |l| l.profit_share_brl),
            taxable_base: total(&lines, |l| l.taxable_base),
            tax_before_credit: total(&lines, |l| l.tax_before_credit),
            credit_used: total(&lines, |l| l.credit_used),
            tax_due: total(&lines, |l| l.tax_due),
            lines,
        });
    }
    warnings.extend(approximate_years.into_iter().map(|year| TaxWarning::OffshoreFxApproximate { year }));
    warnings.extend(open_years.into_iter().map(|year| TaxWarning::OffshoreYearOpen { year }));
    Ok((years, warnings))
}

// ---------------------------------------------------------------------------------------------------------------------
// Controlada ativa (Lei 14.754, art. 6º)
// ---------------------------------------------------------------------------------------------------------------------
//
// A controlada que não cai no §5º do art. 5º (fora de país de tributação favorecida e com renda ativa própria de 60% ou mais
// da renda total) só tem o lucro tributado "no momento da efetiva disponibilização" ao dono, na forma do art. 2º: 15% sobre
// a parcela anual, sem dedução. O app não classifica a entidade (quem diz é o usuário) e só vê a disponibilização que o usuário
// lança como distribuição (o parágrafo único do art. 6º também conta crédito, emprego e operação de crédito com o dono;
// isso fica por conta dele). Premissas a conferir: a conversão é a PTAX de venda do dia do recebimento (o art. 2º não foi
// lido com a regra de câmbio; é a do exterior, Sessão 187), a base é a distribuição inteira (sem o prejuízo da controlada,
// que é do art. 5º §14) e a participação já está no valor recebido.

/// Uma distribuição de entidade ativa já convertida em reais.
#[derive(Clone, Debug, Serialize)]
pub struct ActiveDistributionLine {
    pub distribution_id: i32,
    pub entity_id: i32,
    pub entity_name: String,
    pub country: String,
    pub currency: String,
    pub received_on: NaiveDate,
    /// Valor recebido, na moeda da entidade.
    pub amount: f64,
    /// Câmbio de venda do dia do recebimento e de onde veio.
    pub fx_rate: f64,
    pub fx_source: RateSource,
    pub amount_brl: f64,
}

/// O ano-calendário das distribuições recebidas de entidades ativas.
#[derive(Clone, Debug, Serialize)]
pub struct OffshoreActiveYear {
    pub year: i32,
    pub rate: f64,
    pub taxable_base: f64,
    pub tax_due: f64,
    pub lines: Vec<ActiveDistributionLine>,
}

/// Apura por ano de recebimento as distribuições de entidades `active_regime`. Distribuição de entidade comum é ignorada
/// (ela reduz o crédito de dividendo, `offshore_credits`); sem câmbio na data, a linha fica de fora e avisa.
pub fn assess_active_distributions(
    entities: &[OffshoreEntity],
    distributions: &[OffshoreDistribution],
    rules: &RuleSet,
    rate: RateToBrl,
) -> Result<(Vec<OffshoreActiveYear>, Vec<TaxWarning>), AppError> {
    let by_id: BTreeMap<i32, &OffshoreEntity> =
        entities.iter().filter(|e| e.active_regime).map(|e| (e.id, e)).collect();
    let mut ordered: Vec<&OffshoreDistribution> =
        distributions.iter().filter(|d| by_id.contains_key(&d.entity_id)).collect();
    ordered.sort_by_key(|d| (d.received_on, d.id));

    let mut warnings = Vec::new();
    let mut approximate_years = std::collections::BTreeSet::new();
    let mut lines_by_year: BTreeMap<i32, Vec<ActiveDistributionLine>> = BTreeMap::new();
    for d in ordered {
        let entity = by_id[&d.entity_id];
        let year = d.received_on.year();
        let Some((fx_rate, fx_source)) = convert(&entity.currency, d.received_on, RateKind::Sell, rate) else {
            warnings.push(TaxWarning::OffshoreDistributionFxMissing { year, entity: entity.name.clone() });
            continue;
        };
        if fx_source == RateSource::Market {
            approximate_years.insert(year);
        }
        lines_by_year.entry(year).or_default().push(ActiveDistributionLine {
            distribution_id: d.id,
            entity_id: entity.id,
            entity_name: entity.name.clone(),
            country: entity.country.clone(),
            currency: entity.currency.clone(),
            received_on: d.received_on,
            amount: d.amount,
            fx_rate,
            fx_source,
            amount_brl: d.amount * fx_rate,
        });
    }

    let mut years = Vec::new();
    for (year, lines) in lines_by_year {
        let year_end = NaiveDate::from_ymd_opt(year, 12, 31).ok_or_else(|| AppError::InvalidInput(format!("invalid year {year}")))?;
        let tax_rate = rules.get(rules::OFFSHORE_RATE, year_end)?;
        let taxable_base = lines.iter().fold(0.0, |acc, l| acc + l.amount_brl);
        years.push(OffshoreActiveYear { year, rate: tax_rate, taxable_base, tax_due: taxable_base * tax_rate, lines });
    }
    warnings.extend(approximate_years.into_iter().map(|year| TaxWarning::OffshoreDistributionFxApproximate { year }));
    Ok((years, warnings))
}

// ---------------------------------------------------------------------------------------------------------------------
// Bens e Direitos das controladas (Lei 14.754, art. 5º §§10 a 12)
// ---------------------------------------------------------------------------------------------------------------------
//
// O lucro tributado em 31/12 entra na ficha Bens e Direitos como custo de aquisição de um "crédito de dividendo a receber"
// da controlada, por ano de origem (§10 IV). A distribuição desse lucro reduz o crédito "pelo valor originalmente declarado
// em moeda nacional" e não é tributada de novo (§11); a variação cambial entre o declarado e o recebido não é tributada nem
// deduzida (§12). Premissas do app (a conferir): o crédito de um ano é a base efetivamente tributada (`taxable_base`, depois
// do prejuízo do §14); a distribuição vem na moeda da entidade e reduz o crédito pela taxa de venda de 31/12 do ano de
// origem (a mesma da apuração); só vale para recebimento a partir de 31/12 do ano de origem.

/// Uma distribuição de lucros que o usuário recebeu de uma controlada, na moeda da entidade.
#[derive(Clone, Debug)]
pub struct OffshoreDistribution {
    pub id: i32,
    pub entity_id: i32,
    /// Ano do lucro distribuído.
    pub origin_year: i32,
    pub received_on: NaiveDate,
    pub amount: f64,
}

/// O que uma distribuição reduziu do crédito, em BRL; `None` quando o ano de origem não tem lucro apurado (sem resultado,
/// ano aberto ou sem câmbio), e então ela não reduz nada.
#[derive(Clone, Debug, Serialize)]
pub struct OffshoreDistributionView {
    pub id: i32,
    pub entity_id: i32,
    pub origin_year: i32,
    pub received_on: NaiveDate,
    pub amount: f64,
    pub reduction_brl: Option<f64>,
}

/// O crédito de dividendo a receber de uma entidade, de um ano de origem, em 31/12 de `as_of_year`.
#[derive(Clone, Debug, Serialize)]
pub struct OffshoreCreditRow {
    pub entity_id: i32,
    pub entity_name: String,
    pub country: String,
    pub currency: String,
    pub origin_year: i32,
    pub as_of_year: i32,
    /// Base tributada do ano de origem × câmbio: o valor declarado, em BRL.
    pub taxed_brl: f64,
    /// Quanto as distribuições recebidas até 31/12 de `as_of_year` já reduziram, em BRL.
    pub distributed_brl: f64,
    /// `max(0, taxed − distributed)`.
    pub balance_brl: f64,
    /// O que as distribuições reduziram além do crédito do ano (o saldo trava em zero).
    pub over_distributed_brl: f64,
}

/// Os créditos de dividendo a receber por entidade, ano de origem e 31/12 (de `origin_year` até o ano de `today`), a partir
/// da apuração (`years`) e das distribuições lançadas; e o que cada distribuição reduziu.
pub fn offshore_credits(
    years: &[OffshoreYear],
    distributions: &[OffshoreDistribution],
    today: NaiveDate,
) -> (Vec<OffshoreCreditRow>, Vec<OffshoreDistributionView>) {
    // (entidade, ano de origem) -> a linha apurada.
    let lines: BTreeMap<(i32, i32), &OffshoreLine> =
        years.iter().flat_map(|y| y.lines.iter().map(move |l| ((l.entity_id, y.year), l))).collect();

    let views: Vec<OffshoreDistributionView> = distributions
        .iter()
        .map(|d| OffshoreDistributionView {
            id: d.id,
            entity_id: d.entity_id,
            origin_year: d.origin_year,
            received_on: d.received_on,
            amount: d.amount,
            reduction_brl: lines.get(&(d.entity_id, d.origin_year)).map(|l| d.amount * l.fx_rate),
        })
        .collect();

    let mut rows = Vec::new();
    for (&(entity_id, origin_year), line) in &lines {
        let targeted = views.iter().any(|v| v.entity_id == entity_id && v.origin_year == origin_year);
        if line.taxable_base <= 0.0 && !targeted {
            continue;
        }
        for as_of_year in origin_year..=today.year().max(origin_year) {
            let year_end = NaiveDate::from_ymd_opt(as_of_year, 12, 31).expect("ano válido");
            let distributed = views
                .iter()
                .filter(|v| v.entity_id == entity_id && v.origin_year == origin_year && v.received_on <= year_end)
                .fold(0.0, |acc, v| acc + v.reduction_brl.unwrap_or(0.0));
            rows.push(OffshoreCreditRow {
                entity_id,
                entity_name: line.entity_name.clone(),
                country: line.country.clone(),
                currency: line.currency.clone(),
                origin_year,
                as_of_year,
                taxed_brl: line.taxable_base,
                distributed_brl: distributed,
                balance_brl: (line.taxable_base - distributed).max(0.0),
                over_distributed_brl: (distributed - line.taxable_base).max(0.0),
            });
        }
    }
    rows.sort_by_key(|r| (r.entity_id, r.origin_year, r.as_of_year));
    (rows, views)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax::rules::test_rule_set;
    use chrono::Datelike;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn entity(id: i32, currency: &str, percent: f64) -> OffshoreEntity {
        OffshoreEntity { id, name: format!("Holding {id}"), country: "US".to_string(), currency: currency.to_string(), ownership_percent: percent, active_regime: false }
    }

    fn result(entity_id: i32, year: i32, profit: f64, tax_paid_abroad: f64) -> OffshoreResult {
        OffshoreResult { entity_id, year, profit, tax_paid_abroad }
    }

    /// PTAX de 31/12: venda 5 em 2024 e 6 em 2025; compra 0,1 abaixo; sem cotação para o resto.
    fn ptax_rate(currency: &str, date: NaiveDate, kind: RateKind) -> Option<(f64, RateSource)> {
        let sell = match (currency, date.year()) {
            ("USD", 2024) => 5.0,
            ("USD", 2025) => 6.0,
            _ => return None,
        };
        Some((if kind == RateKind::Buy { sell - 0.1 } else { sell }, RateSource::Ptax))
    }

    fn run(entities: &[OffshoreEntity], results: &[OffshoreResult]) -> (Vec<OffshoreYear>, Vec<TaxWarning>) {
        assess_offshore(entities, results, &test_rule_set(), d("2026-10-22"), &ptax_rate).unwrap()
    }

    #[test]
    fn the_profit_converts_at_the_ptax_of_31_12_and_pays_15_percent() {
        // US$ 1.000 × R$ 5 = 5.000; imposto 750
        let (years, warnings) = run(&[entity(1, "USD", 100.0)], &[result(1, 2024, 1_000.0, 0.0)]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let y = &years[0];
        assert_eq!(y.year, 2024);
        assert!(close(y.profit_share_brl, 5_000.0) && close(y.taxable_base, 5_000.0), "{y:?}");
        assert!(close(y.tax_before_credit, 750.0) && close(y.tax_due, 750.0) && close(y.rate, 0.15));
        assert!(close(y.lines[0].fx_rate, 5.0) && y.lines[0].fx_source == RateSource::Ptax);
    }

    #[test]
    fn only_the_ownership_share_of_the_profit_is_taxed() {
        // 40% de US$ 1.000 × 5 = 2.000; imposto 300
        let (years, _) = run(&[entity(1, "USD", 40.0)], &[result(1, 2024, 1_000.0, 0.0)]);
        assert!(close(years[0].profit_share_brl, 2_000.0) && close(years[0].tax_due, 300.0), "{:?}", years[0]);
    }

    #[test]
    fn a_loss_offsets_later_profits_of_the_same_entity_only() {
        // entidade 1: −1.000 em 2024 (−5.000) e +1.500 em 2025 (9.000) → base 4.000; entidade 2 não herda o prejuízo
        let (years, _) = run(
            &[entity(1, "USD", 100.0), entity(2, "USD", 100.0)],
            &[result(1, 2024, -1_000.0, 0.0), result(1, 2025, 1_500.0, 0.0), result(2, 2025, 500.0, 0.0)],
        );
        assert_eq!(years.iter().map(|y| y.year).collect::<Vec<_>>(), vec![2024, 2025]);
        assert!(close(years[0].taxable_base, 0.0) && close(years[0].lines[0].loss_carried_out, 5_000.0));
        let (first, second) = (&years[1].lines[0], &years[1].lines[1]);
        assert!(close(first.loss_carried_in, 5_000.0) && close(first.taxable_base, 4_000.0), "{first:?}");
        assert!(close(first.loss_carried_out, 0.0) && close(first.tax_before_credit, 600.0));
        assert!(close(second.loss_carried_in, 0.0) && close(second.taxable_base, 3_000.0), "{second:?}");
        assert!(close(years[1].tax_before_credit, 1_050.0));
    }

    #[test]
    fn the_tax_paid_abroad_is_a_credit_at_the_buy_rate_limited_to_the_tax() {
        // imposto lá fora US$ 100 × 4,9 (compra) = 490 de crédito; devido 750 − 490 = 260
        let (years, _) = run(&[entity(1, "USD", 100.0)], &[result(1, 2024, 1_000.0, 100.0)]);
        let line = &years[0].lines[0];
        assert!(close(line.tax_paid_abroad_brl, 490.0) && close(line.credit_used, 490.0), "{line:?}");
        assert!(close(line.tax_due, 260.0));

        // imposto lá fora US$ 1.000 → 4.900, o crédito para em 750
        let (years, _) = run(&[entity(1, "USD", 100.0)], &[result(1, 2024, 1_000.0, 1_000.0)]);
        assert!(close(years[0].lines[0].credit_used, 750.0) && close(years[0].tax_due, 0.0), "{:?}", years[0]);
    }

    #[test]
    fn the_market_rate_is_a_fallback_and_marks_the_year_approximate() {
        let market = |currency: &str, date: NaiveDate, kind: RateKind| {
            ptax_rate(currency, date, kind).map(|(rate, _)| (rate, RateSource::Market))
        };
        let (years, warnings) = assess_offshore(
            &[entity(1, "USD", 100.0)],
            &[result(1, 2024, 1_000.0, 0.0)],
            &test_rule_set(),
            d("2026-10-22"),
            &market,
        )
        .unwrap();
        assert!(close(years[0].profit_share_brl, 5_000.0) && years[0].lines[0].fx_source == RateSource::Market);
        assert!(warnings.iter().any(|w| matches!(w, TaxWarning::OffshoreFxApproximate { year: 2024 })));
    }

    #[test]
    fn a_year_without_any_rate_is_left_out_and_warns() {
        // 2023 não tem câmbio no teste
        let (years, warnings) = run(&[entity(1, "USD", 100.0)], &[result(1, 2023, 1_000.0, 0.0)]);
        assert!(years.is_empty());
        assert!(warnings
            .iter()
            .any(|w| matches!(w, TaxWarning::OffshoreFxMissing { year: 2023, entity } if entity == "Holding 1")));
    }

    #[test]
    fn a_year_that_has_not_closed_is_open_and_not_assessed() {
        let (years, warnings) = run(&[entity(1, "USD", 100.0)], &[result(1, 2026, 1_000.0, 0.0)]);
        assert!(years.is_empty());
        assert!(warnings.iter().any(|w| matches!(w, TaxWarning::OffshoreYearOpen { year: 2026 })));
    }

    #[test]
    fn an_entity_in_brl_needs_no_exchange_rate() {
        let (years, warnings) = run(&[entity(1, "BRL", 50.0)], &[result(1, 2024, 1_000.0, 0.0)]);
        assert!(warnings.is_empty());
        assert!(close(years[0].profit_share_brl, 500.0) && close(years[0].lines[0].fx_rate, 1.0));
    }

    #[test]
    fn totals_start_at_positive_zero_when_nothing_is_due() {
        // ano de prejuízo: base, imposto e crédito são zero, nunca −0,0
        let (years, _) = run(&[entity(1, "USD", 100.0)], &[result(1, 2024, -1_000.0, 0.0)]);
        let y = &years[0];
        assert!(y.taxable_base.is_sign_positive() && y.tax_due.is_sign_positive() && y.credit_used.is_sign_positive(), "{y:?}");
    }

    #[test]
    fn a_result_of_an_unknown_entity_is_ignored() {
        let (years, warnings) = run(&[entity(1, "USD", 100.0)], &[result(9, 2024, 1_000.0, 0.0)]);
        assert!(years.is_empty() && warnings.is_empty());
    }

    fn distribution(id: i32, entity_id: i32, origin_year: i32, received: &str, amount: f64) -> OffshoreDistribution {
        OffshoreDistribution { id, entity_id, origin_year, received_on: d(received), amount }
    }

    fn credits(
        entities: &[OffshoreEntity],
        results: &[OffshoreResult],
        distributions: &[OffshoreDistribution],
    ) -> (Vec<OffshoreCreditRow>, Vec<OffshoreDistributionView>) {
        let (years, _) = run(entities, results);
        offshore_credits(&years, distributions, d("2026-10-22"))
    }

    fn balance(rows: &[OffshoreCreditRow], entity_id: i32, origin: i32, as_of: i32) -> f64 {
        rows.iter().find(|r| r.entity_id == entity_id && r.origin_year == origin && r.as_of_year == as_of).unwrap().balance_brl
    }

    #[test]
    fn the_taxed_profit_is_the_dividend_credit_and_a_distribution_reduces_it_at_the_origin_rate() {
        // 50% de US$ 2.000 em 2025 × 6,00 = R$ 6.000 de crédito. Distribuição de US$ 500 em 10/03/2026 reduz 500 × 6,00 =
        // R$ 3.000, pela taxa de 31/12/2025 (a declarada), não pela do recebimento
        let (rows, views) = credits(
            &[entity(1, "USD", 50.0)],
            &[result(1, 2025, 2_000.0, 0.0)],
            &[distribution(7, 1, 2025, "2026-03-10", 500.0)],
        );
        assert!(close(balance(&rows, 1, 2025, 2025), 6_000.0), "{rows:?}");
        assert!(close(balance(&rows, 1, 2025, 2026), 3_000.0), "{rows:?}");
        let r = rows.iter().find(|r| r.as_of_year == 2026).unwrap();
        assert!(close(r.taxed_brl, 6_000.0) && close(r.distributed_brl, 3_000.0) && close(r.over_distributed_brl, 0.0), "{r:?}");
        assert!(close(views[0].reduction_brl.unwrap(), 3_000.0));
    }

    #[test]
    fn the_credit_is_the_taxed_base_after_the_loss_of_the_entity_not_the_gross_profit() {
        // 2024: prejuízo de US$ 400 (−2.000 a 5,00). 2025: lucro de US$ 1.500 (9.000 a 6,00), base 7.000 depois do prejuízo
        let (rows, _) = credits(
            &[entity(1, "USD", 100.0)],
            &[result(1, 2024, -400.0, 0.0), result(1, 2025, 1_500.0, 0.0)],
            &[],
        );
        assert!(close(balance(&rows, 1, 2025, 2025), 7_000.0), "{rows:?}");
        // o ano de prejuízo não gera crédito (e sem distribuição nem aparece)
        assert!(rows.iter().all(|r| r.origin_year == 2025), "{rows:?}");
    }

    #[test]
    fn a_distribution_larger_than_the_credit_stops_at_zero_and_reports_the_excess() {
        // crédito 6.000; distribuição de US$ 1.500 × 6,00 = 9.000: saldo zero, excesso 3.000
        let (rows, _) = credits(
            &[entity(1, "USD", 50.0)],
            &[result(1, 2025, 2_000.0, 0.0)],
            &[distribution(1, 1, 2025, "2026-03-10", 1_500.0)],
        );
        let r = rows.iter().find(|r| r.as_of_year == 2026).unwrap();
        assert!(close(r.balance_brl, 0.0) && close(r.over_distributed_brl, 3_000.0), "{r:?}");
    }

    #[test]
    fn a_distribution_without_a_taxed_origin_year_reduces_nothing() {
        // não há resultado de 2024: a distribuição de lucro de 2024 não tem crédito para reduzir
        let (rows, views) = credits(
            &[entity(1, "USD", 50.0)],
            &[result(1, 2025, 2_000.0, 0.0)],
            &[distribution(1, 1, 2024, "2026-03-10", 500.0)],
        );
        assert_eq!(views[0].reduction_brl, None);
        assert!(close(balance(&rows, 1, 2025, 2026), 6_000.0), "{rows:?}");
        assert!(rows.iter().all(|r| r.origin_year == 2025));
    }

    #[test]
    fn entities_have_independent_credits_and_only_distributions_up_to_each_31_12_count() {
        // entidade 1: crédito 5.000 (US$ 1.000 a 5,00 em 2024); entidade 2: 6.000 (US$ 1.000 a 6,00 em 2025). Distribuição
        // de 100 dólares da entidade 1 em 31/12/2024 (conta no 31/12/2024) e outra de 200 em 05/01/2025 (só em 2025)
        let (rows, _) = credits(
            &[entity(1, "USD", 100.0), entity(2, "USD", 100.0)],
            &[result(1, 2024, 1_000.0, 0.0), result(2, 2025, 1_000.0, 0.0)],
            &[distribution(1, 1, 2024, "2024-12-31", 100.0), distribution(2, 1, 2024, "2025-01-05", 200.0)],
        );
        assert!(close(balance(&rows, 1, 2024, 2024), 4_500.0), "{rows:?}");
        assert!(close(balance(&rows, 1, 2024, 2025), 3_500.0), "{rows:?}");
        assert!(close(balance(&rows, 2, 2025, 2025), 6_000.0), "{rows:?}");
    }

    fn active_entity(id: i32, currency: &str) -> OffshoreEntity {
        OffshoreEntity { active_regime: true, ..entity(id, currency, 100.0) }
    }

    fn active_run(
        entities: &[OffshoreEntity],
        distributions: &[OffshoreDistribution],
    ) -> (Vec<OffshoreActiveYear>, Vec<TaxWarning>) {
        assess_active_distributions(entities, distributions, &test_rule_set(), &ptax_rate).unwrap()
    }

    #[test]
    fn an_active_entity_is_not_taxed_on_31_12_even_with_a_profit_and_a_loss_does_not_carry() {
        // lucro de US$ 1.000 em 2024 e prejuízo em 2025: nada apurado em 31/12 e sem aviso de ano aberto
        let (years, warnings) =
            run(&[active_entity(1, "USD")], &[result(1, 2024, 1_000.0, 0.0), result(1, 2025, -400.0, 0.0)]);
        assert!(years.is_empty() && warnings.is_empty(), "{years:?} {warnings:?}");
    }

    #[test]
    fn an_active_distribution_pays_15_percent_at_the_ptax_of_the_day_it_was_received() {
        // US$ 1.000 recebidos em 2024 (PTAX 5,00) = R$ 5.000, imposto 750; US$ 200 em 2025 (6,00) = R$ 1.200, imposto 180
        let (years, warnings) = active_run(
            &[active_entity(1, "USD")],
            &[distribution(1, 1, 2024, "2024-06-10", 1_000.0), distribution(2, 1, 2025, "2025-02-03", 200.0)],
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(years.iter().map(|y| y.year).collect::<Vec<_>>(), vec![2024, 2025]);
        assert!(close(years[0].taxable_base, 5_000.0) && close(years[0].tax_due, 750.0) && close(years[0].rate, 0.15));
        assert!(close(years[1].taxable_base, 1_200.0) && close(years[1].tax_due, 180.0), "{:?}", years[1]);
        assert!(years[0].lines[0].fx_source == RateSource::Ptax && close(years[0].lines[0].fx_rate, 5.0));
    }

    #[test]
    fn the_distributions_of_a_year_add_up_and_an_ordinary_entity_is_ignored() {
        // duas distribuições de 2024 somam 100 × 5 + 300 × 5 = 2.000 (imposto 300); a da entidade comum não entra
        let (years, _) = active_run(
            &[active_entity(1, "USD"), entity(2, "USD", 100.0)],
            &[
                distribution(1, 1, 2024, "2024-03-01", 100.0),
                distribution(2, 1, 2024, "2024-09-01", 300.0),
                distribution(3, 2, 2024, "2024-09-01", 999.0),
            ],
        );
        assert_eq!(years.len(), 1);
        assert!(close(years[0].taxable_base, 2_000.0) && close(years[0].tax_due, 300.0), "{:?}", years[0]);
        assert_eq!(years[0].lines.len(), 2);
    }

    #[test]
    fn an_active_distribution_without_any_rate_is_left_out_and_warns() {
        // não há PTAX nem câmbio para 2026
        let (years, warnings) = active_run(&[active_entity(1, "USD")], &[distribution(1, 1, 2026, "2026-03-10", 100.0)]);
        assert!(years.is_empty());
        assert!(
            matches!(warnings.as_slice(), [TaxWarning::OffshoreDistributionFxMissing { year: 2026, entity }] if entity == "Holding 1"),
            "{warnings:?}"
        );
    }

    #[test]
    fn the_market_rate_marks_the_active_year_approximate() {
        let market = |_: &str, _: NaiveDate, _: RateKind| Some((4.0, RateSource::Market));
        let (years, warnings) = assess_active_distributions(
            &[active_entity(1, "USD")],
            &[distribution(1, 1, 2024, "2024-06-10", 100.0)],
            &test_rule_set(),
            &market,
        )
        .unwrap();
        assert!(close(years[0].taxable_base, 400.0), "{years:?}");
        assert!(matches!(warnings.as_slice(), [TaxWarning::OffshoreDistributionFxApproximate { year: 2024 }]), "{warnings:?}");
    }

    #[test]
    fn an_entity_in_brl_needs_no_rate_for_an_active_distribution() {
        let (years, warnings) = active_run(&[active_entity(1, "BRL")], &[distribution(1, 1, 2026, "2026-03-10", 1_000.0)]);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(close(years[0].taxable_base, 1_000.0) && close(years[0].tax_due, 150.0), "{years:?}");
    }
}
