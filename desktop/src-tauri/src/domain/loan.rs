// Fase 29 — empréstimos concedidos a terceiros. Só funções puras: juros
// informativos, saldo a receber e "extra recebido". Nada aqui é gravado, o
// saldo é sempre derivado (mesmo princípio de `domain::liability`).

use chrono::{Datelike, NaiveDate};

use crate::error::AppError;

/// Meses completos entre `inicio` e `hoje` (0 se `hoje` for antes de
/// `inicio`). Dia do mês ainda não atingido não conta o mês corrente.
fn full_months_between(inicio: NaiveDate, hoje: NaiveDate) -> i32 {
    if hoje <= inicio {
        return 0;
    }
    let mut months =
        (hoje.year() - inicio.year()) * 12 + hoje.month() as i32 - inicio.month() as i32;
    if hoje.day() < inicio.day() {
        months -= 1;
    }
    months.max(0)
}

/// Juros **simples** sobre o principal, por meses completos, à taxa mensal
/// em percentual cru (ex. `1.5` = 1,5% a.m.). Sem taxa (`None`) ou taxa ≤ 0
/// não há juros: o caso real (empréstimo informal em família) não tem taxa
/// contratada. É um número informativo, o usuário baixa o valor que quiser.
pub fn accrued_interest(
    principal: f64,
    taxa_mensal_percentual: Option<f64>,
    data_inicio: NaiveDate,
    hoje: NaiveDate,
) -> f64 {
    match taxa_mensal_percentual {
        Some(taxa) if taxa > 0.0 => {
            principal * (taxa / 100.0) * full_months_between(data_inicio, hoje) as f64
        }
        _ => 0.0,
    }
}

/// Saldo a receber = principal + juros − soma do que já foi baixado. Nunca
/// negativo: baixar mais do que o devido quita o empréstimo, não gera saldo
/// a pagar.
pub fn outstanding_balance(principal: f64, interest: f64, valores_baixados: &[f64]) -> f64 {
    (principal + interest - valores_baixados.iter().sum::<f64>()).max(0.0)
}

/// Quanto entrou a mais (ou a menos) do que foi baixado do saldo, somando
/// todas as baixas, pares `(valor_recebido, valor_baixado)`. Positivo =
/// "um pouquinho a mais" que a pessoa devolveu.
pub fn extra_received(repayments: &[(f64, f64)]) -> f64 {
    repayments.iter().map(|(recebido, baixado)| recebido - baixado).sum()
}

/// Validações de entrada compartilhadas por `create_loan` e
/// `register_loan_repayment`.
pub fn validate_positive(field: &str, value: f64) -> Result<(), AppError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(AppError::InvalidGuard(format!("{field} deve ser maior que zero")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn full_months_do_not_count_incomplete_month() {
        assert_eq!(full_months_between(d(2026, 1, 15), d(2026, 2, 14)), 0);
        assert_eq!(full_months_between(d(2026, 1, 15), d(2026, 2, 15)), 1);
        assert_eq!(full_months_between(d(2025, 11, 30), d(2026, 3, 31)), 4);
    }

    #[test]
    fn full_months_is_zero_when_today_is_before_start() {
        assert_eq!(full_months_between(d(2026, 5, 1), d(2026, 1, 1)), 0);
    }

    #[test]
    fn no_rate_means_no_interest() {
        assert_eq!(accrued_interest(1000.0, None, d(2026, 1, 1), d(2027, 1, 1)), 0.0);
        assert_eq!(accrued_interest(1000.0, Some(0.0), d(2026, 1, 1), d(2027, 1, 1)), 0.0);
    }

    #[test]
    fn simple_interest_over_full_months() {
        // 1.000 a 2% a.m. por 3 meses completos = 60.
        let interest = accrued_interest(1000.0, Some(2.0), d(2026, 1, 10), d(2026, 4, 10));
        assert!((interest - 60.0).abs() < 1e-9);
    }

    #[test]
    fn balance_subtracts_written_off_amounts() {
        assert_eq!(outstanding_balance(1000.0, 0.0, &[300.0, 200.0]), 500.0);
    }

    #[test]
    fn balance_includes_interest() {
        assert_eq!(outstanding_balance(1000.0, 60.0, &[500.0]), 560.0);
    }

    #[test]
    fn balance_never_goes_negative() {
        assert_eq!(outstanding_balance(1000.0, 0.0, &[1200.0]), 0.0);
    }

    #[test]
    fn extra_received_is_independent_of_written_off() {
        // Recebeu 1.050 e baixou 1.000: sobraram 50 a mais.
        assert_eq!(extra_received(&[(1050.0, 1000.0)]), 50.0);
        // Recebeu menos do que baixou: diferença negativa.
        assert_eq!(extra_received(&[(1050.0, 1000.0), (90.0, 100.0)]), 40.0);
    }

    #[test]
    fn validate_positive_rejects_zero_negative_and_nan() {
        assert!(validate_positive("principal", 1.0).is_ok());
        assert!(validate_positive("principal", 0.0).is_err());
        assert!(validate_positive("principal", -5.0).is_err());
        assert!(validate_positive("principal", f64::NAN).is_err());
    }
}
