// Fase 34 — custo médio de uma posição, um motor só para a tela de posições, a
// conversão pra moeda base e a apuração de IR. Cada lançamento do ativo é
// reproduzido em ordem (data, id):
//   compra     soma valor + taxa ao custo;
//   venda      baixa a quantidade ao custo médio e realiza ganho = valor − taxa − custo baixado;
//   zerou      o preço médio recomeça do zero na compra seguinte;
//   desdobramento/grupamento  multiplica a quantidade, o custo não muda;
//   bonificação  entra quantidade nova com o custo atribuído.
// `transferencia` só move quantidade entre custódias.

use std::collections::BTreeMap;

use chrono::NaiveDate;

pub const EPS: f64 = 1e-9;

#[derive(Default, Clone, Debug)]
pub struct Position {
    pub quantity: f64,
    pub cost: f64,
}

/// Baixa `quantity` da posição ao custo médio. Devolve (custo, quantidade
/// realmente coberta pelo histórico).
pub fn take_from_position(pos: &mut Position, quantity: f64) -> (f64, f64) {
    let covered = quantity.min(pos.quantity.max(0.0));
    let avg = if pos.quantity > EPS { pos.cost / pos.quantity } else { 0.0 };
    let cost = avg * covered;
    pos.quantity -= covered;
    pos.cost -= cost;
    // Posição zerada → preço médio recomeça do zero na próxima compra.
    if pos.quantity <= EPS {
        *pos = Position::default();
    }
    (cost, covered)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Buy,
    Sell,
    Transfer,
    /// `quantity` é a razão: 2 = cada ação vira 2; 0,1 = grupamento de 10 em 1.
    Split,
    /// `quantity` são as ações novas; `value` é o custo atribuído a elas.
    Bonus,
}

#[derive(Clone, Debug)]
pub struct Event {
    pub date: NaiveDate,
    pub id: i32,
    pub kind: EventKind,
    pub quantity: f64,
    /// Valor do negócio sem taxa, na moeda do ativo (compra, venda, bonificação).
    pub value: f64,
    pub fee: f64,
    pub custodia_id: Option<i32>,
    pub to_custodia_id: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PositionState {
    /// Quantidade líquida. Pode ficar negativa em histórico legado com venda sem saldo.
    pub quantity: f64,
    pub cost: f64,
    pub realized_pl: f64,
    /// Alguma venda passou do que o histórico cobria; o ganho só conta a parte coberta.
    pub uncovered_sale: bool,
    pub by_custodia: BTreeMap<Option<i32>, f64>,
}

impl PositionState {
    pub fn average_price(&self) -> Option<f64> {
        (self.quantity > EPS).then(|| self.cost / self.quantity)
    }
}

/// Reproduz os lançamentos de um ativo. `rate` converte o valor de cada evento
/// (data → taxa de câmbio pra moeda de saída); com `|_| Some(1.0)` o resultado
/// fica na moeda do ativo. `None` se alguma data não tem taxa.
pub fn replay(events: &[Event], rate: &dyn Fn(NaiveDate) -> Option<f64>) -> Option<PositionState> {
    let mut ordered: Vec<&Event> = events.iter().collect();
    ordered.sort_by_key(|e| (e.date, e.id));

    let mut state = PositionState::default();
    let mut pos = Position::default();
    for event in ordered {
        match event.kind {
            EventKind::Buy => {
                let r = rate(event.date)?;
                pos.quantity += event.quantity;
                pos.cost += (event.value + event.fee) * r;
                state.quantity += event.quantity;
                *state.by_custodia.entry(event.custodia_id).or_insert(0.0) += event.quantity;
            }
            EventKind::Sell => {
                let r = rate(event.date)?;
                let (cost, covered) = take_from_position(&mut pos, event.quantity);
                let share = if event.quantity > EPS { covered / event.quantity } else { 0.0 };
                if covered + EPS < event.quantity {
                    state.uncovered_sale = true;
                }
                state.realized_pl += (event.value - event.fee) * r * share - cost;
                state.quantity -= event.quantity;
                *state.by_custodia.entry(event.custodia_id).or_insert(0.0) -= event.quantity;
            }
            EventKind::Transfer => {
                *state.by_custodia.entry(event.custodia_id).or_insert(0.0) -= event.quantity;
                *state.by_custodia.entry(event.to_custodia_id).or_insert(0.0) += event.quantity;
            }
            EventKind::Split => {
                pos.quantity *= event.quantity;
                state.quantity *= event.quantity;
                for qty in state.by_custodia.values_mut() {
                    *qty *= event.quantity;
                }
            }
            EventKind::Bonus => {
                let r = rate(event.date)?;
                pos.quantity += event.quantity;
                pos.cost += event.value * r;
                state.quantity += event.quantity;
                *state.by_custodia.entry(event.custodia_id).or_insert(0.0) += event.quantity;
            }
        }
    }
    state.cost = pos.cost;
    // O estado "coberto" e o líquido só divergem com venda sem saldo; o custo
    // acompanha o que o histórico cobre.
    Some(state)
}

pub fn native_rate(_: NaiveDate) -> Option<f64> {
    Some(1.0)
}

/// Razão de cada desdobramento/grupamento/bonificação do ativo, na ordem em que
/// aconteceram. A razão da bonificação é (quantidade antes + nova) ÷ antes, que
/// é como as fontes de preço (Yahoo) ajustam o histórico. Bonificação sem
/// posição anterior não gera razão.
pub fn split_factors(events: &[Event]) -> Vec<(NaiveDate, f64)> {
    let mut ordered: Vec<&Event> = events.iter().collect();
    ordered.sort_by_key(|e| (e.date, e.id));
    let mut held = 0.0;
    let mut factors = Vec::new();
    for event in ordered {
        match event.kind {
            EventKind::Buy => held += event.quantity,
            EventKind::Sell => held -= event.quantity,
            EventKind::Transfer => {}
            EventKind::Split => {
                held *= event.quantity;
                factors.push((event.date, event.quantity));
            }
            EventKind::Bonus => {
                if held > EPS {
                    factors.push((event.date, (held + event.quantity) / held));
                }
                held += event.quantity;
            }
        }
    }
    factors
}

/// Produto das razões que aconteceram **depois** de `date`: quanto a quantidade
/// que se tinha em `date` vale em ações de hoje (que é como o histórico de
/// preço ajustado a desdobramentos a mede).
pub fn factor_after(factors: &[(NaiveDate, f64)], date: NaiveDate) -> f64 {
    factors.iter().filter(|(d, _)| *d > date).map(|(_, ratio)| ratio).product()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn event(id: i32, date: NaiveDate, kind: EventKind, quantity: f64, value: f64, fee: f64) -> Event {
        Event { date, id, kind, quantity, value, fee, custodia_id: None, to_custodia_id: None }
    }

    fn run(events: &[Event]) -> PositionState {
        replay(events, &native_rate).unwrap()
    }

    #[test]
    fn a_sale_does_not_change_the_average_cost() {
        // 100 @ 10, vende 50 @ 20, compra 50 @ 30. Custo restante 500 + 1500 = 2000 sobre 100 = 20.
        let state = run(&[
            event(1, d(2026, 1, 1), EventKind::Buy, 100.0, 1000.0, 0.0),
            event(2, d(2026, 2, 1), EventKind::Sell, 50.0, 1000.0, 0.0),
            event(3, d(2026, 3, 1), EventKind::Buy, 50.0, 1500.0, 0.0),
        ]);
        assert_eq!(state.quantity, 100.0);
        assert!((state.average_price().unwrap() - 20.0).abs() < 1e-9);
        // Vendeu 50 por 1000 contra custo 500.
        assert!((state.realized_pl - 500.0).abs() < 1e-9);
    }

    #[test]
    fn closing_the_position_restarts_the_average() {
        let state = run(&[
            event(1, d(2026, 1, 1), EventKind::Buy, 10.0, 100.0, 0.0),
            event(2, d(2026, 2, 1), EventKind::Sell, 10.0, 150.0, 0.0),
            event(3, d(2026, 3, 1), EventKind::Buy, 10.0, 200.0, 0.0),
        ]);
        assert!((state.average_price().unwrap() - 20.0).abs() < 1e-9);
        assert!((state.realized_pl - 50.0).abs() < 1e-9);
    }

    #[test]
    fn fees_raise_the_cost_and_lower_the_proceeds() {
        let state = run(&[
            event(1, d(2026, 1, 1), EventKind::Buy, 10.0, 100.0, 2.0),
            event(2, d(2026, 2, 1), EventKind::Sell, 10.0, 150.0, 3.0),
        ]);
        // custo 102, recebe 147.
        assert!((state.realized_pl - 45.0).abs() < 1e-9);
        assert_eq!(state.average_price(), None);
    }

    #[test]
    fn same_day_events_follow_the_id_order() {
        // A venda (id 2) só pode vir depois da compra (id 1), mesmo listada antes.
        let state = run(&[
            event(2, d(2026, 1, 1), EventKind::Sell, 5.0, 60.0, 0.0),
            event(1, d(2026, 1, 1), EventKind::Buy, 10.0, 100.0, 0.0),
        ]);
        assert!(!state.uncovered_sale);
        assert_eq!(state.quantity, 5.0);
        assert!((state.realized_pl - 10.0).abs() < 1e-9);
    }

    #[test]
    fn selling_more_than_held_counts_only_the_covered_part() {
        let state = run(&[
            event(1, d(2026, 1, 1), EventKind::Buy, 10.0, 100.0, 0.0),
            event(2, d(2026, 2, 1), EventKind::Sell, 20.0, 400.0, 0.0),
        ]);
        assert!(state.uncovered_sale);
        assert_eq!(state.quantity, -10.0);
        // 10 cobertas: recebe metade (200), custo 100.
        assert!((state.realized_pl - 100.0).abs() < 1e-9);
    }

    #[test]
    fn a_split_multiplies_quantity_and_keeps_the_cost() {
        let state = run(&[
            event(1, d(2026, 1, 1), EventKind::Buy, 10.0, 1000.0, 0.0),
            event(2, d(2026, 2, 1), EventKind::Split, 2.0, 0.0, 0.0),
        ]);
        assert_eq!(state.quantity, 20.0);
        assert!((state.average_price().unwrap() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_reverse_split_divides_the_quantity() {
        let state = run(&[
            event(1, d(2026, 1, 1), EventKind::Buy, 100.0, 100.0, 0.0),
            event(2, d(2026, 2, 1), EventKind::Split, 0.1, 0.0, 0.0),
        ]);
        assert!((state.quantity - 10.0).abs() < 1e-9);
        assert!((state.average_price().unwrap() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_bonus_adds_shares_with_the_declared_cost() {
        // 100 @ 10 (custo 1000) + bonificação de 10 ações atribuídas a R$ 2 cada.
        let state = run(&[
            event(1, d(2026, 1, 1), EventKind::Buy, 100.0, 1000.0, 0.0),
            event(2, d(2026, 2, 1), EventKind::Bonus, 10.0, 20.0, 0.0),
        ]);
        assert_eq!(state.quantity, 110.0);
        assert!((state.cost - 1020.0).abs() < 1e-9);
    }

    #[test]
    fn transfers_only_move_the_custody_breakdown() {
        let mut buy = event(1, d(2026, 1, 1), EventKind::Buy, 10.0, 100.0, 0.0);
        buy.custodia_id = Some(1);
        let mut transfer = event(2, d(2026, 2, 1), EventKind::Transfer, 4.0, 0.0, 0.0);
        transfer.custodia_id = Some(1);
        transfer.to_custodia_id = Some(2);
        let state = run(&[buy, transfer]);
        assert_eq!(state.quantity, 10.0);
        assert_eq!(state.by_custodia.get(&Some(1)), Some(&6.0));
        assert_eq!(state.by_custodia.get(&Some(2)), Some(&4.0));
        assert!((state.average_price().unwrap() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn conversion_uses_the_rate_of_each_date_and_realizes_in_the_output_currency() {
        // Compra 10 @ US$ 10 com dólar a 5 (custo R$ 500), vende 5 @ US$ 20 com dólar a 6.
        let rate = |date: NaiveDate| Some(if date < d(2026, 2, 1) { 5.0 } else { 6.0 });
        let state = replay(
            &[
                event(1, d(2026, 1, 1), EventKind::Buy, 10.0, 100.0, 0.0),
                event(2, d(2026, 2, 1), EventKind::Sell, 5.0, 100.0, 0.0),
            ],
            &rate,
        )
        .unwrap();
        assert!((state.cost - 250.0).abs() < 1e-9);
        // Recebe 600, custo baixado 250.
        assert!((state.realized_pl - 350.0).abs() < 1e-9);
    }

    #[test]
    fn a_missing_rate_gives_no_result() {
        let no_rate = |_: NaiveDate| None;
        assert!(replay(&[event(1, d(2026, 1, 1), EventKind::Buy, 1.0, 1.0, 0.0)], &no_rate).is_none());
    }

    #[test]
    fn split_factors_give_the_shares_of_today_for_a_past_date() {
        let events = [
            event(1, d(2026, 1, 1), EventKind::Buy, 100.0, 1000.0, 0.0),
            event(2, d(2026, 3, 1), EventKind::Split, 2.0, 0.0, 0.0),
            event(3, d(2026, 5, 1), EventKind::Bonus, 20.0, 0.0, 0.0),
        ];
        let factors = split_factors(&events);
        // Depois do split há 200; a bonificação de 20 vira razão 220/200 = 1,1.
        assert_eq!(factors.len(), 2);
        assert!((factors[1].1 - 1.1).abs() < 1e-9);
        assert!((factor_after(&factors, d(2026, 2, 1)) - 2.2).abs() < 1e-9);
        assert!((factor_after(&factors, d(2026, 4, 1)) - 1.1).abs() < 1e-9);
        assert!((factor_after(&factors, d(2026, 6, 1)) - 1.0).abs() < 1e-9);
    }
}
