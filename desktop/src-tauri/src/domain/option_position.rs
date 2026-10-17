use chrono::NaiveDate;

// Fase 18, 3ª fatia (Sessão 116) — resultado de uma posição em opção. Funções
// puras: o comando busca as linhas e os preços, aqui só fica a conta.
//
// Comprada: paga o prêmio e ganha se o preço subir. Lançada: recebe o prêmio e
// ganha se o preço cair. Os dois lados são o mesmo `sinal · (preço − prêmio)`.

pub const LONG: &str = "long";
pub const SHORT: &str = "short";

pub const CLOSED: &str = "closed";
pub const EXPIRED: &str = "expired";
pub const EXERCISED: &str = "exercised";
pub const CLOSE_KINDS: [&str; 3] = [CLOSED, EXPIRED, EXERCISED];

/// +1 comprada, −1 lançada.
pub fn sign(side: &str) -> f64 {
    if side == SHORT {
        -1.0
    } else {
        1.0
    }
}

/// Resultado em R$ de `quantity` opções cujo prêmio foi `premium`, ao preço
/// `price` (encerramento ou último negócio), já descontadas as taxas.
pub fn result(
    side: &str,
    premium: f64,
    quantity: f64,
    price: f64,
    open_fee: Option<f64>,
    close_fee: Option<f64>,
) -> f64 {
    sign(side) * (price - premium) * quantity - open_fee.unwrap_or(0.0) - close_fee.unwrap_or(0.0)
}

/// Dias corridos até o vencimento; negativo depois dele.
pub fn days_to_expiry(expiration: NaiveDate, today: NaiveDate) -> i64 {
    (expiration - today).num_days()
}

/// Posição aberta com o vencimento já passado: a tela pede o encerramento.
pub fn is_expired(expiration: NaiveDate, today: NaiveDate, is_open: bool) -> bool {
    is_open && expiration < today
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn a_long_position_gains_when_the_price_rises() {
        // Comprou 100 a R$ 2,00; vale R$ 3,50: +150.
        assert!(close(result(LONG, 2.0, 100.0, 3.5, None, None), 150.0));
        assert!(close(result(LONG, 2.0, 100.0, 0.0, None, None), -200.0)); // virou pó
    }

    #[test]
    fn a_short_position_gains_when_the_price_falls() {
        // Lançou 100 a R$ 2,00; recompra a R$ 0,50: +150. Virar pó = ganha o prêmio todo.
        assert!(close(result(SHORT, 2.0, 100.0, 0.5, None, None), 150.0));
        assert!(close(result(SHORT, 2.0, 100.0, 0.0, None, None), 200.0));
        assert!(close(result(SHORT, 2.0, 100.0, 5.0, None, None), -300.0));
    }

    #[test]
    fn fees_reduce_the_result_on_both_sides() {
        assert!(close(result(LONG, 2.0, 100.0, 3.0, Some(5.0), Some(7.0)), 100.0 - 12.0));
        assert!(close(result(SHORT, 2.0, 100.0, 1.0, Some(5.0), None), 100.0 - 5.0));
    }

    #[test]
    fn an_unknown_side_counts_as_long() {
        assert_eq!(sign("anything"), 1.0);
        assert_eq!(sign(SHORT), -1.0);
    }

    #[test]
    fn days_and_expiry_flag() {
        let today = date(2026, 10, 17);
        assert_eq!(days_to_expiry(date(2026, 11, 20), today), 34);
        assert_eq!(days_to_expiry(today, today), 0);
        assert_eq!(days_to_expiry(date(2026, 10, 16), today), -1);
        // O vencimento de hoje ainda não passou; fechada nunca é "vencida".
        assert!(!is_expired(today, today, true));
        assert!(is_expired(date(2026, 10, 16), today, true));
        assert!(!is_expired(date(2026, 10, 16), today, false));
    }
}
