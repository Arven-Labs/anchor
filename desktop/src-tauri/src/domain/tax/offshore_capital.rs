// Ganho de capital na venda de participação em entidade controlada no exterior (Lei 14.754, art. 7º). A variação cambial do
// principal aplicado compõe o ganho percebido na alienação, baixa ou liquidação do investimento, "inclusive por meio de
// devolução de capital", tributado pelo art. 21 da Lei 8.981/1995 (§1º: valor percebido em reais − custo de aquisição médio em
// reais; §2º: devolução de capital sem cancelar cotas usa a proporção devolvida / capital total aplicado).
//
// O app não conta cotas: o usuário lança os aportes (valor na moeda da entidade, mais o custo em reais da DAA para os antigos)
// e, na venda, o valor recebido e a fração da participação vendida; na devolução de capital, o valor devolvido.
//
// Regras (as faixas, o mínimo e o código do DARF vêm de `tax_rules`, o resto é código):
// - Estado por entidade: capital aplicado (moeda da entidade) e custo (BRL). Aporte soma os dois; o custo é o informado em reais
//   ou o valor × PTAX de venda do dia (art. 15). A atualização de 2023 (art. 14 §4º II) soma o custo adicional em 31/12/2023.
// - Venda: recebido = valor × PTAX de venda do dia; custo vendido = custo × fração; ganho = recebido − custo vendido.
//   Devolução: fração = valor / capital aplicado (no máximo 1). Capital e custo caem na mesma fração.
// - Venda ou devolução sem capital lançado: custo 0 (ganho = recebido) e aviso. Sem câmbio na data: evento ignorado e aviso.
//   Só vale para fatos a partir de 2024; antes disso a lei não se aplica e o evento fica de fora.
// - Imposto por mês do evento: ganho positivo pelas faixas do art. 21 sobre o ganho acumulado **no ano** (só desta fonte),
//   com mínimo de DARF e saldo acumulado, como a cripto. Prejuízo do mês não gera imposto, não compensa e não carrega.
//
// ATENÇÃO: premissas a conferir na Receita: o aporte converte pela PTAX de venda (o art. 15 não é explícito para custo de
// aquisição), a tabela acumula só estes ganhos (não soma com cripto) e não há isenção de pequeno valor.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use serde::Serialize;

use super::assessment::TaxWarning;
use super::calendar::darf_due_date;
use super::crypto::{gain_tiers, tiered_tax};
use super::foreign::{convert, update_2023_date, RateToBrl, FIRST_YEAR};
use super::offshore::{OffshoreEntity, OffshoreUpdate2023Row};
use super::rules::{self, RuleSet};
use crate::domain::cost_basis::EPS;
use crate::domain::ptax::{RateKind, RateSource};
use crate::error::AppError;

/// O que aconteceu com o capital aplicado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum CapitalKind {
    /// Capital aplicado na entidade.
    #[serde(rename = "aporte")]
    Contribution,
    /// Alienação, baixa ou liquidação da participação (inteira ou em parte).
    #[serde(rename = "venda")]
    Sale,
    /// Devolução de capital, sem cancelar cotas.
    #[serde(rename = "devolucao")]
    Return,
}

impl CapitalKind {
    /// O `kind` gravado em `offshore_capital_events`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "aporte" => Some(Self::Contribution),
            "venda" => Some(Self::Sale),
            "devolucao" => Some(Self::Return),
            _ => None,
        }
    }
}

/// Um evento lançado pelo usuário, na moeda da entidade.
#[derive(Clone, Debug)]
pub struct CapitalEvent {
    pub id: i32,
    pub entity_id: i32,
    pub kind: CapitalKind,
    pub date: NaiveDate,
    pub amount: f64,
    /// Só no aporte: o custo em reais declarado. `None` = converte pela PTAX do dia.
    pub brl_cost: Option<f64>,
    /// Só na venda: a fração da participação vendida, de 0 (exclusive) a 1.
    pub fraction: Option<f64>,
}

/// Uma venda ou devolução já convertida, com o custo baixado.
#[derive(Clone, Debug, Serialize)]
pub struct CapitalDisposal {
    pub event_id: i32,
    pub entity_id: i32,
    pub entity_name: String,
    pub currency: String,
    pub kind: CapitalKind,
    pub date: NaiveDate,
    /// Valor recebido, na moeda da entidade.
    pub amount: f64,
    /// Câmbio de venda do dia e de onde veio.
    pub fx_rate: f64,
    pub fx_source: RateSource,
    pub received_brl: f64,
    /// A fração do custo que saiu.
    pub fraction: f64,
    pub cost_brl: f64,
    pub gain: f64,
    /// Não havia capital lançado: o ganho considera custo zero.
    pub cost_unknown: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct OffshoreCapitalMonth {
    /// `YYYY-MM`
    pub period: String,
    /// Ganho do mês (ganhos − perdas).
    pub gain: f64,
    /// Alíquota efetiva do mês.
    pub rate: f64,
    pub tax: f64,
    /// Imposto abaixo do mínimo de DARF vindo de meses anteriores.
    pub carried_in: f64,
    /// Valor do DARF do mês (0 se ficou abaixo do mínimo).
    pub due: f64,
    pub carried_out: f64,
    pub due_date: NaiveDate,
    pub darf_code: i32,
    pub disposals: Vec<CapitalDisposal>,
}

/// O que sobra da participação depois de todos os eventos lançados.
#[derive(Clone, Debug, Serialize)]
pub struct OffshoreCapitalPosition {
    pub entity_id: i32,
    pub entity_name: String,
    pub currency: String,
    /// Capital aplicado que resta, na moeda da entidade.
    pub capital: f64,
    /// Custo de aquisição que resta, em reais.
    pub cost_brl: f64,
}

fn period_of(date: NaiveDate) -> String {
    format!("{:04}-{:02}", date.year(), date.month())
}

/// Apura o ganho de capital das vendas e devoluções, mês a mês, e devolve a posição de cada entidade com eventos.
pub fn assess_offshore_capital(
    entities: &[OffshoreEntity],
    updates: &[OffshoreUpdate2023Row],
    events: &[CapitalEvent],
    rules: &RuleSet,
    rate: RateToBrl,
) -> Result<(Vec<OffshoreCapitalMonth>, Vec<OffshoreCapitalPosition>, Vec<TaxWarning>), AppError> {
    let by_id: BTreeMap<i32, &OffshoreEntity> = entities.iter().map(|e| (e.id, e)).collect();
    let mut warnings = Vec::new();
    let mut positions = Vec::new();
    let mut by_month: BTreeMap<String, Vec<CapitalDisposal>> = BTreeMap::new();

    for (&entity_id, entity) in &by_id {
        let mut mine: Vec<&CapitalEvent> = events.iter().filter(|e| e.entity_id == entity_id).collect();
        if mine.is_empty() {
            continue;
        }
        // Dentro do dia, o aporte vem antes da venda.
        mine.sort_by_key(|e| (e.date, e.kind != CapitalKind::Contribution, e.id));
        let mut update_cost = updates.iter().find(|u| u.entity_id == entity_id).map(|u| u.additional_cost_brl);
        let (mut capital, mut cost) = (0.0_f64, 0.0_f64);

        for event in mine {
            // O custo adicional da atualização de 2023 vale no fim de 31/12/2023: antes do primeiro evento seguinte.
            if event.date > update_2023_date() {
                cost += update_cost.take().unwrap_or(0.0);
            }
            let fx = convert(&entity.currency, event.date, RateKind::Sell, rate);
            match event.kind {
                CapitalKind::Contribution => {
                    let brl = match (event.brl_cost, fx) {
                        (Some(declared), _) => declared,
                        (None, Some((fx_rate, _))) => event.amount * fx_rate,
                        (None, None) => {
                            warnings.push(TaxWarning::OffshoreCapitalFxMissing { entity: entity.name.clone(), date: event.date });
                            continue;
                        }
                    };
                    capital += event.amount;
                    cost += brl;
                }
                CapitalKind::Sale | CapitalKind::Return => {
                    // A Lei 14.754 vale para fatos a partir de 2024.
                    if event.date.year() < FIRST_YEAR {
                        continue;
                    }
                    let Some((fx_rate, fx_source)) = fx else {
                        warnings.push(TaxWarning::OffshoreCapitalFxMissing { entity: entity.name.clone(), date: event.date });
                        continue;
                    };
                    let cost_unknown = capital <= EPS;
                    let fraction = match event.kind {
                        _ if cost_unknown => 0.0,
                        CapitalKind::Sale => event.fraction.unwrap_or(1.0).clamp(0.0, 1.0),
                        _ => (event.amount / capital).min(1.0),
                    };
                    if cost_unknown {
                        warnings.push(TaxWarning::OffshoreCapitalCostUnknown { entity: entity.name.clone(), date: event.date });
                    }
                    let received_brl = event.amount * fx_rate;
                    let cost_brl = cost * fraction;
                    capital -= capital * fraction;
                    cost -= cost_brl;
                    by_month.entry(period_of(event.date)).or_default().push(CapitalDisposal {
                        event_id: event.id,
                        entity_id,
                        entity_name: entity.name.clone(),
                        currency: entity.currency.clone(),
                        kind: event.kind,
                        date: event.date,
                        amount: event.amount,
                        fx_rate,
                        fx_source,
                        received_brl,
                        fraction,
                        cost_brl,
                        gain: received_brl - cost_brl,
                        cost_unknown,
                    });
                }
            }
        }
        // A atualização de 2023 pode ser posterior a todos os eventos.
        cost += update_cost.take().unwrap_or(0.0);
        positions.push(OffshoreCapitalPosition {
            entity_id,
            entity_name: entity.name.clone(),
            currency: entity.currency.clone(),
            capital,
            cost_brl: cost,
        });
    }

    let mut months = Vec::new();
    let mut carried = 0.0;
    let mut taxable_by_year: BTreeMap<i32, f64> = BTreeMap::new();
    for (period, mut disposals) in by_month {
        disposals.sort_by_key(|d| (d.date, d.event_id));
        let first_day = NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d")
            .map_err(|_| AppError::InvalidGuard(format!("invalid period {period}")))?;
        let gain: f64 = disposals.iter().map(|d| d.gain).sum();
        let taxable = gain.max(0.0);

        // A tabela progressiva corre sobre o ganho tributável acumulado no ano (só desta fonte).
        let year_total = taxable_by_year.entry(first_day.year()).or_insert(0.0);
        let tiers = gain_tiers(rules, first_day)?;
        let tax = tiered_tax(*year_total, taxable, &tiers);
        *year_total += taxable;
        let rate = if taxable > EPS { tax / taxable } else { tiers[0].1 };

        let minimum = rules.get(rules::DARF_MINIMUM, first_day)?;
        let carried_in = carried;
        let payable = tax + carried_in;
        let (due, carried_out) = if payable + EPS < minimum { (0.0, payable) } else { (payable, 0.0) };
        carried = carried_out;

        months.push(OffshoreCapitalMonth {
            period,
            gain,
            rate,
            tax,
            carried_in,
            due,
            carried_out,
            due_date: darf_due_date(first_day.year(), first_day.month()),
            // Ganho de capital da pessoa física: o mesmo código 4600 da cripto.
            darf_code: rules.get(rules::DARF_CODE_CRYPTO, first_day)? as i32,
            disposals,
        });
    }
    Ok((months, positions, warnings))
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

    fn entity() -> OffshoreEntity {
        OffshoreEntity {
            id: 1,
            name: "Holding LLC".to_string(),
            country: "US".to_string(),
            currency: "USD".to_string(),
            ownership_percent: 100.0,
            active_regime: false,
        }
    }

    /// PTAX de venda: 4,00 em 2023, 5,00 em 2024 e 6,00 em 2025; sem cotação para o resto.
    fn ptax(_: &str, date: NaiveDate, _: RateKind) -> Option<(f64, RateSource)> {
        let sell = match date.year() {
            2023 => 4.0,
            2024 => 5.0,
            2025 => 6.0,
            _ => return None,
        };
        Some((sell, RateSource::Ptax))
    }

    fn event(id: i32, kind: CapitalKind, date: &str, amount: f64) -> CapitalEvent {
        CapitalEvent { id, entity_id: 1, kind, date: d(date), amount, brl_cost: None, fraction: None }
    }

    fn sale(id: i32, date: &str, amount: f64, fraction: f64) -> CapitalEvent {
        CapitalEvent { fraction: Some(fraction), ..event(id, CapitalKind::Sale, date, amount) }
    }

    fn run(events: &[CapitalEvent]) -> (Vec<OffshoreCapitalMonth>, Vec<OffshoreCapitalPosition>, Vec<TaxWarning>) {
        assess_offshore_capital(&[entity()], &[], events, &test_rule_set(), &ptax).unwrap()
    }

    #[test]
    fn a_sale_of_half_the_stake_taxes_the_gain_over_the_average_cost() {
        // aporte de US$ 1.000 a 5,00 (2024) = custo R$ 5.000. Venda de 50% por US$ 800 a 6,00 (2025) = R$ 4.800 recebidos,
        // custo vendido R$ 2.500, ganho R$ 2.300, imposto 15% = R$ 345
        let (months, positions, warnings) = run(&[
            event(1, CapitalKind::Contribution, "2024-03-01", 1_000.0),
            sale(2, "2025-02-10", 800.0, 0.5),
        ]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let m = &months[0];
        assert_eq!(m.period, "2025-02");
        let s = &m.disposals[0];
        assert!(close(s.received_brl, 4_800.0) && close(s.cost_brl, 2_500.0) && close(s.gain, 2_300.0), "{s:?}");
        assert!(close(m.tax, 345.0) && close(m.due, 345.0) && close(m.rate, 0.15), "{m:?}");
        assert_eq!(m.darf_code, 4600);
        // sobra metade: capital US$ 500 e custo R$ 2.500
        assert!(close(positions[0].capital, 500.0) && close(positions[0].cost_brl, 2_500.0), "{positions:?}");
    }

    #[test]
    fn a_capital_return_uses_the_proportion_over_the_capital_applied() {
        // capital US$ 1.000 (custo R$ 5.000). Devolução de US$ 250 = 25%: recebido 250 × 6,00 = R$ 1.500, custo R$ 1.250,
        // ganho R$ 250, imposto R$ 37,50
        let (months, positions, _) = run(&[
            event(1, CapitalKind::Contribution, "2024-03-01", 1_000.0),
            event(2, CapitalKind::Return, "2025-02-10", 250.0),
        ]);
        let s = &months[0].disposals[0];
        assert!(close(s.fraction, 0.25) && close(s.cost_brl, 1_250.0) && close(s.gain, 250.0), "{s:?}");
        assert!(close(months[0].tax, 37.5), "{:?}", months[0]);
        assert!(close(positions[0].capital, 750.0) && close(positions[0].cost_brl, 3_750.0), "{positions:?}");
    }

    #[test]
    fn a_declared_brl_cost_replaces_the_ptax_conversion_of_a_contribution() {
        // aporte antigo de US$ 1.000 com custo declarado de R$ 3.000 (a PTAX de 2024 daria 5.000); venda total por US$ 1.000
        // a 6,00 = R$ 6.000: ganho R$ 3.000
        let (months, _, _) = run(&[
            CapitalEvent { brl_cost: Some(3_000.0), ..event(1, CapitalKind::Contribution, "2024-01-10", 1_000.0) },
            sale(2, "2025-02-10", 1_000.0, 1.0),
        ]);
        assert!(close(months[0].gain, 3_000.0), "{:?}", months[0]);
    }

    #[test]
    fn the_2023_update_adds_its_additional_cost_to_the_basis() {
        // custo R$ 4.000 (US$ 1.000 a 4,00 em 2023) + custo adicional da atualização R$ 1.000 = R$ 5.000. Venda total por
        // US$ 1.000 a 6,00 = R$ 6.000: ganho R$ 1.000 (sem a atualização seria R$ 2.000)
        let events = [
            event(1, CapitalKind::Contribution, "2023-06-01", 1_000.0),
            sale(2, "2025-02-10", 1_000.0, 1.0),
        ];
        let update = OffshoreUpdate2023Row {
            entity_id: 1,
            entity_name: "Holding LLC".to_string(),
            country: "US".to_string(),
            currency: "USD".to_string(),
            active_regime: false,
            equity: 1_500.0,
            profits: 500.0,
            fx_rate: 4.0,
            fx_source: RateSource::Ptax,
            equity_brl: 6_000.0,
            profits_brl: 2_000.0,
            additional_cost_brl: 1_000.0,
        };
        let (months, _, _) = assess_offshore_capital(&[entity()], &[update], &events, &test_rule_set(), &ptax).unwrap();
        assert!(close(months[0].gain, 1_000.0), "{:?}", months[0]);
        let (months, _, _) = run(&events);
        assert!(close(months[0].gain, 2_000.0), "{:?}", months[0]);
    }

    #[test]
    fn a_big_gain_is_taxed_by_bracket_over_the_year() {
        // sem aporte (custo 0) e recebido US$ 1.000.000 × 6,00 = R$ 6.000.000 de ganho. As faixas:
        // 5.000.000 × 15% + 1.000.000 × 17,5% = 750.000 + 175.000 = R$ 925.000
        let (months, _, warnings) = run(&[sale(1, "2025-02-10", 1_000_000.0, 1.0)]);
        assert!(close(months[0].gain, 6_000_000.0), "{:?}", months[0]);
        assert!(close(months[0].tax, 925_000.0), "{:?}", months[0]);
        assert!(matches!(warnings.as_slice(), [TaxWarning::OffshoreCapitalCostUnknown { .. }]), "{warnings:?}");
    }

    #[test]
    fn a_small_tax_below_the_darf_minimum_accumulates_to_the_next_month() {
        // custo 0 e recebido US$ 8 a 6,00 = R$ 48: imposto R$ 7,20 (< R$ 10) acumula; o mês seguinte soma R$ 6,00 de imposto
        // (US$ 6,67 × 6,00 = 40 × 15%) e passa do mínimo: DARF de R$ 13,20
        let (months, _, _) = run(&[sale(1, "2025-02-10", 8.0, 1.0), sale(2, "2025-03-10", 40.0 / 6.0, 1.0)]);
        assert!(close(months[0].tax, 7.2) && close(months[0].due, 0.0) && close(months[0].carried_out, 7.2), "{:?}", months[0]);
        assert!(close(months[1].carried_in, 7.2) && close(months[1].due, 13.2), "{:?}", months[1]);
    }

    #[test]
    fn a_losing_month_owes_nothing_and_does_not_carry() {
        // aporte de US$ 1.000 a 5,00 = R$ 5.000 e venda total por US$ 500 a 6,00 = R$ 3.000: perda de R$ 2.000
        let (months, _, _) = run(&[
            event(1, CapitalKind::Contribution, "2024-03-01", 1_000.0),
            sale(2, "2025-02-10", 500.0, 1.0),
        ]);
        assert!(close(months[0].gain, -2_000.0) && close(months[0].tax, 0.0) && close(months[0].carried_out, 0.0), "{:?}", months[0]);
    }

    #[test]
    fn a_sale_without_capital_warns_and_an_event_without_a_rate_is_left_out() {
        // sem aporte: custo 0 e aviso. Venda em 2026 sem câmbio: ignorada, com aviso
        let (months, _, warnings) = run(&[sale(1, "2025-02-10", 100.0, 1.0), sale(2, "2026-02-10", 100.0, 1.0)]);
        assert_eq!(months.len(), 1, "{months:?}");
        assert!(months[0].disposals[0].cost_unknown);
        assert!(
            warnings.iter().any(|w| matches!(w, TaxWarning::OffshoreCapitalCostUnknown { .. }))
                && warnings.iter().any(|w| matches!(w, TaxWarning::OffshoreCapitalFxMissing { .. })),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_sale_before_2024_is_left_out_but_the_contribution_counts() {
        // venda de 2023 não entra (a lei vale de 2024 em diante); o aporte de 2023 forma o custo
        let (months, positions, warnings) = run(&[
            event(1, CapitalKind::Contribution, "2023-06-01", 1_000.0),
            sale(2, "2023-12-01", 100.0, 0.5),
        ]);
        assert!(months.is_empty() && warnings.is_empty(), "{months:?} {warnings:?}");
        assert!(close(positions[0].capital, 1_000.0) && close(positions[0].cost_brl, 4_000.0), "{positions:?}");
    }
}
