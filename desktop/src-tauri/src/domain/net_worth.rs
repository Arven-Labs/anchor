/// Divisão-guarda pura — em ponto flutuante `x/0.0` não panica, vira
/// `inf`/`NaN` silenciosamente, que vazaria pro JSON do frontend sem aviso.
/// Mesma granularidade de "guard de uma linha ainda ganha função + teste"
/// de `domain::position_pricing`/`domain::liability`.
pub fn compute_leverage(total_debt: f64, net_worth: f64) -> Option<f64> {
    if net_worth <= 0.0 {
        return None;
    }
    Some(total_debt / net_worth)
}

/// Soma o saldo a receber só dos empréstimos concedidos com a flag
/// "entra no patrimônio" ligada (Fase 29). Pares `(incluir, saldo)`.
pub fn sum_receivable_in_net_worth(loans: &[(bool, f64)]) -> f64 {
    loans.iter().filter(|(include, _)| *include).map(|(_, saldo)| saldo).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sum_receivable_skips_loans_flagged_out_of_net_worth() {
        // Empréstimo à mãe (fora) + empréstimo a um amigo (dentro).
        assert_eq!(sum_receivable_in_net_worth(&[(false, 5000.0), (true, 800.0)]), 800.0);
    }

    #[test]
    fn sum_receivable_is_zero_without_loans() {
        assert_eq!(sum_receivable_in_net_worth(&[]), 0.0);
    }

    #[test]
    fn compute_leverage_guards_zero_net_worth() {
        assert_eq!(compute_leverage(500.0, 0.0), None);
    }

    #[test]
    fn compute_leverage_guards_negative_net_worth() {
        assert_eq!(compute_leverage(500.0, -100.0), None);
    }

    #[test]
    fn compute_leverage_divides_debt_by_net_worth() {
        assert_eq!(compute_leverage(500.0, 2000.0), Some(0.25));
    }
}
