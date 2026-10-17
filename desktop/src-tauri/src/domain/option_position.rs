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

/// `(receita, custo)` de uma posição **encerrada**, no vocabulário da apuração
/// de imposto: `receita − custo` é exatamente `result`. Comprada: paga o prêmio
/// (+ taxa de abertura) e recebe o preço de encerramento (− taxa de saída).
/// Lançada: recebe o prêmio (− taxa de abertura) e paga o preço de encerramento
/// (+ taxa de saída).
pub fn proceeds_and_cost(
    side: &str,
    premium: f64,
    quantity: f64,
    close_price: f64,
    open_fee: Option<f64>,
    close_fee: Option<f64>,
) -> (f64, f64) {
    let (open_fee, close_fee) = (open_fee.unwrap_or(0.0), close_fee.unwrap_or(0.0));
    if side == SHORT {
        (premium * quantity - open_fee, close_price * quantity + close_fee)
    } else {
        (close_price * quantity - close_fee, premium * quantity + open_fee)
    }
}

/// Quanto uma posição **aberta** vale no patrimônio e se esse valor veio do
/// prêmio (sem preço atualizado) em vez do mercado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptionValue {
    pub value: f64,
    /// Sem preço guardado: valeu o prêmio, como a carteira faz sem cotação.
    pub at_cost: bool,
}

/// Valor no patrimônio: comprada é ativo (`+preço · quantidade`), lançada é
/// passivo (`−preço · quantidade`, o que custaria recomprá-la). O preço é o
/// último guardado (`mark_price`) ou, na falta dele, o prêmio. Posição aberta
/// já **vencida** vale 0: a opção expirou e falta só encerrá-la.
pub fn net_worth_value(
    side: &str,
    quantity: f64,
    premium: f64,
    mark_price: Option<f64>,
    expired: bool,
) -> OptionValue {
    if expired {
        return OptionValue { value: 0.0, at_cost: false };
    }
    let price = mark_price.unwrap_or(premium);
    OptionValue { value: sign(side) * price * quantity, at_cost: mark_price.is_none() }
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
    fn proceeds_minus_cost_is_the_result_on_both_sides() {
        for (side, close_price) in [(LONG, 3.5), (LONG, 0.0), (SHORT, 0.5), (SHORT, 5.0)] {
            let (proceeds, cost) = proceeds_and_cost(side, 2.0, 100.0, close_price, Some(5.0), Some(7.0));
            let expected = result(side, 2.0, 100.0, close_price, Some(5.0), Some(7.0));
            assert!(close(proceeds - cost, expected), "{side} {close_price}");
        }
        // Lançada: receita é o prêmio (− taxa de abertura); custo é a recompra (+ taxa de saída).
        assert_eq!(proceeds_and_cost(SHORT, 2.0, 100.0, 0.0, Some(5.0), None), (195.0, 0.0));
        // Comprada que vira pó: sem receita, custo é o prêmio pago.
        assert_eq!(proceeds_and_cost(LONG, 2.0, 100.0, 0.0, None, None), (0.0, 200.0));
    }

    #[test]
    fn a_long_position_is_an_asset_and_a_short_one_is_a_liability() {
        // Com preço guardado: 100 opções a R$ 3,50.
        let long = net_worth_value(LONG, 100.0, 2.0, Some(3.5), false);
        assert_eq!(long, OptionValue { value: 350.0, at_cost: false });
        let short = net_worth_value(SHORT, 100.0, 2.0, Some(3.5), false);
        assert_eq!(short, OptionValue { value: -350.0, at_cost: false });
    }

    #[test]
    fn without_a_stored_price_the_premium_is_used_and_flagged_at_cost() {
        assert_eq!(net_worth_value(LONG, 100.0, 2.0, None, false), OptionValue { value: 200.0, at_cost: true });
        assert_eq!(net_worth_value(SHORT, 100.0, 1.5, None, false), OptionValue { value: -150.0, at_cost: true });
    }

    #[test]
    fn an_open_position_past_its_expiry_is_worth_nothing() {
        for (side, mark) in [(LONG, Some(3.5)), (SHORT, None)] {
            assert_eq!(net_worth_value(side, 100.0, 2.0, mark, true), OptionValue { value: 0.0, at_cost: false });
        }
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
