// Previdência privada: o que o informe de Bens e Direitos pede de um plano em 31/12 de um ano: o custo e o saldo. É
// **informativo**: nada aqui entra na apuração de IR, no patrimônio nem na Rentabilidade, e o app não afirma qual grupo e
// código usar (o dono digita, copiando do programa da Receita).
//
// Modelo do projeto (`position_pricing.rs`): cada real aportado é uma cota de preço 1 (quantidade = valor aportado), o
// resgate é uma venda de cotas, e o saldo é o último extrato lançado à mão (`asset_valuations`). Logo, o custo em uma data é
// a quantidade líquida de cotas (compras menos vendas) até ela.
//
// NÃO confirmado: se o custo de um plano com resgate parcial deve ser a quantidade líquida (o que usamos) ou outra regra da
// Receita para separar principal de rendimento; a Sessão 399 não achou fonte. O dono confere contra o informe da seguradora.

use serde::Serialize;

/// Um aporte (`compra`) ou resgate (`venda`) do plano, em cotas de R$ 1. Os outros tipos de transação são ignorados.
pub struct Movement<'a> {
    pub date: &'a str,
    pub transaction_type: &'a str,
    pub quantity: f64,
}

/// Um saldo de extrato lançado à mão.
pub struct Statement<'a> {
    pub date: &'a str,
    pub value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BalanceSource {
    /// O último extrato lançado até 31/12.
    Statement,
    /// Sem extrato até 31/12: o saldo mostrado é o custo.
    Cost,
    /// O plano não tinha cotas em 31/12.
    NotHeld,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct YearEndPosition {
    pub year: i32,
    pub cost: f64,
    pub balance: f64,
    pub balance_source: BalanceSource,
    /// Data do extrato usado (`AAAA-MM-DD`); `None` quando o saldo não vem de um extrato.
    pub balance_date: Option<String>,
}

/// A posição do plano em 31/12 de `year`. Compara datas `AAAA-MM-DD` como texto (a ordem lexicográfica é a cronológica).
pub fn year_end_position(year: i32, movements: &[Movement], statements: &[Statement]) -> YearEndPosition {
    let year_end = format!("{year}-12-31");
    let net_quotas: f64 = movements
        .iter()
        .filter(|m| m.date <= year_end.as_str())
        .map(|m| match m.transaction_type {
            "compra" => m.quantity,
            "venda" => -m.quantity,
            _ => 0.0,
        })
        .sum();
    // Menos de meio centavo de cota é resíduo de ponto flutuante (ou resgate além do aportado), não um plano mantido.
    if net_quotas < 0.005 {
        return YearEndPosition { year, cost: 0.0, balance: 0.0, balance_source: BalanceSource::NotHeld, balance_date: None };
    }
    match statements.iter().filter(|s| s.date <= year_end.as_str()).max_by(|a, b| a.date.cmp(b.date)) {
        Some(s) => YearEndPosition {
            year,
            cost: net_quotas,
            balance: s.value,
            balance_source: BalanceSource::Statement,
            balance_date: Some(s.date.to_string()),
        },
        None => YearEndPosition {
            year,
            cost: net_quotas,
            balance: net_quotas,
            balance_source: BalanceSource::Cost,
            balance_date: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    fn buy(date: &str, quantity: f64) -> Movement<'_> {
        Movement { date, transaction_type: "compra", quantity }
    }

    fn sell(date: &str, quantity: f64) -> Movement<'_> {
        Movement { date, transaction_type: "venda", quantity }
    }

    fn two_contributions() -> Vec<Movement<'static>> {
        vec![buy("2026-01-05", 500.0), buy("2026-02-05", 500.0)]
    }

    #[test]
    fn the_balance_is_the_last_statement_up_to_the_year_end() {
        let statements = [Statement { date: "2026-03-01", value: 1080.0 }];
        let p = year_end_position(2026, &two_contributions(), &statements);

        assert!(close(p.cost, 1000.0));
        assert!(close(p.balance, 1080.0));
        assert_eq!(p.balance_source, BalanceSource::Statement);
        assert_eq!(p.balance_date.as_deref(), Some("2026-03-01"));
    }

    #[test]
    fn a_plan_not_yet_contributed_to_is_not_held() {
        let statements = [Statement { date: "2026-03-01", value: 1080.0 }];
        let p = year_end_position(2025, &two_contributions(), &statements);

        assert!(close(p.cost, 0.0));
        assert!(close(p.balance, 0.0));
        assert_eq!(p.balance_source, BalanceSource::NotHeld);
        assert_eq!(p.balance_date, None);
    }

    #[test]
    fn a_partial_redemption_lowers_the_cost_and_keeps_the_last_statement() {
        let mut movements = two_contributions();
        movements.push(sell("2026-03-10", 300.0));
        let statements = [Statement { date: "2026-03-01", value: 1080.0 }];
        let p = year_end_position(2026, &movements, &statements);

        assert!(close(p.cost, 700.0));
        assert!(close(p.balance, 1080.0));
        assert_eq!(p.balance_source, BalanceSource::Statement);
    }

    #[test]
    fn without_a_statement_the_balance_shown_is_the_cost() {
        let p = year_end_position(2026, &two_contributions(), &[]);

        assert!(close(p.cost, 1000.0));
        assert!(close(p.balance, 1000.0));
        assert_eq!(p.balance_source, BalanceSource::Cost);
        assert_eq!(p.balance_date, None);
    }

    #[test]
    fn a_statement_after_the_year_end_is_ignored() {
        let statements = [Statement { date: "2026-03-01", value: 1080.0 }, Statement { date: "2027-01-10", value: 1200.0 }];
        let p = year_end_position(2026, &two_contributions(), &statements);

        assert!(close(p.balance, 1080.0));
        assert_eq!(p.balance_date.as_deref(), Some("2026-03-01"));
    }

    #[test]
    fn the_latest_statement_wins_whatever_the_input_order() {
        let statements = [Statement { date: "2026-06-30", value: 1150.0 }, Statement { date: "2026-03-01", value: 1080.0 }];
        let p = year_end_position(2026, &two_contributions(), &statements);

        assert!(close(p.balance, 1150.0));
        assert_eq!(p.balance_date.as_deref(), Some("2026-06-30"));
    }

    #[test]
    fn the_year_end_day_counts_and_the_next_day_does_not() {
        let movements = vec![buy("2026-12-31", 100.0), buy("2027-01-01", 900.0)];
        let p = year_end_position(2026, &movements, &[]);

        assert!(close(p.cost, 100.0));
        let statements = [Statement { date: "2026-12-31", value: 105.0 }, Statement { date: "2027-01-01", value: 999.0 }];
        let p = year_end_position(2026, &movements, &statements);
        assert!(close(p.balance, 105.0));
    }

    #[test]
    fn a_fully_redeemed_plan_is_not_held_even_with_an_old_statement() {
        let mut movements = two_contributions();
        movements.push(sell("2026-03-10", 1000.0));
        let statements = [Statement { date: "2026-03-01", value: 1080.0 }];
        let p = year_end_position(2026, &movements, &statements);

        assert!(close(p.cost, 0.0));
        assert!(close(p.balance, 0.0));
        assert_eq!(p.balance_source, BalanceSource::NotHeld);
    }

    #[test]
    fn other_transaction_types_are_ignored() {
        let mut movements = two_contributions();
        movements.push(Movement { date: "2026-04-01", transaction_type: "provento", quantity: 50.0 });
        let p = year_end_position(2026, &movements, &[]);

        assert!(close(p.cost, 1000.0));
    }

    #[test]
    fn redeeming_more_than_was_contributed_never_gives_a_negative_cost() {
        let movements = vec![buy("2026-01-05", 100.0), sell("2026-02-05", 150.0)];
        let p = year_end_position(2026, &movements, &[]);

        assert!(close(p.cost, 0.0));
        assert_eq!(p.balance_source, BalanceSource::NotHeld);
    }
}
