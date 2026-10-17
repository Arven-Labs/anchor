// Catálogo de moedas para pares de câmbio — porta de
// `lume/api/app/sources/currency_catalog.py`. Ao contrário dos metais (`xau` →
// `GC=F` não tem relação mecânica), o par vira ticker do Yahoo só concatenando:
// `EUR` + `USD` → `EURUSD=X`. Por isso o catálogo lista *moedas* (ISO 4217),
// não pares: um par é válido se as duas metades estão aqui e são diferentes
// (17 × 16 = 272 pares, incluindo cross entre moedas não-BRL como `GBPJPY=X`).
//
// Curado para quem usa o Anchor no Brasil: majors do G10, BRL, CNY (maior
// parceiro comercial) e vizinhos do Mercosul/América Latina. Moedas de valor
// muito baixo (ex.: VND) ficam de fora: o Yahoo arredonda a cotação para 0,0
// na precisão que ele reporta.

use crate::error::AppError;

pub const CURRENCIES: [(&str, &str); 17] = [
    ("USD", "US Dollar"),
    ("EUR", "Euro"),
    ("GBP", "British Pound"),
    ("JPY", "Japanese Yen"),
    ("CHF", "Swiss Franc"),
    ("CAD", "Canadian Dollar"),
    ("AUD", "Australian Dollar"),
    ("NZD", "New Zealand Dollar"),
    ("CNY", "Chinese Yuan"),
    ("BRL", "Brazilian Real"),
    ("ARS", "Argentine Peso"),
    ("UYU", "Uruguayan Peso"),
    ("PYG", "Paraguayan Guarani"),
    ("CLP", "Chilean Peso"),
    ("COP", "Colombian Peso"),
    ("PEN", "Peruvian Sol"),
    ("MXN", "Mexican Peso"),
];

pub struct PairInfo {
    /// `EURUSD` — a chave gravada em `stock_quotes`/`stock_price_history`.
    pub code: String,
    /// `EURUSD=X` — o ticker do Yahoo.
    pub yahoo_symbol: String,
    /// `EUR/USD`.
    pub name: String,
    pub quote: String,
}

fn is_supported(code: &str) -> bool {
    CURRENCIES.iter().any(|(c, _)| *c == code)
}

/// Valida o par antes de qualquer chamada de rede (par inválido é `NoData`,
/// como o 404 que o Lume devolvia). Aceita qualquer caixa.
pub fn pair_info(base: &str, quote: &str) -> Result<PairInfo, AppError> {
    let base = base.trim().to_uppercase();
    let quote = quote.trim().to_uppercase();
    if !is_supported(&base) || !is_supported(&quote) {
        return Err(AppError::NoData(format!("unknown currency pair '{base}/{quote}'")));
    }
    if base == quote {
        return Err(AppError::NoData(format!("currency pair '{base}/{quote}' has the same currency on both sides")));
    }
    Ok(PairInfo {
        code: format!("{base}{quote}"),
        yahoo_symbol: format!("{base}{quote}=X"),
        name: format!("{base}/{quote}"),
        quote,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_becomes_a_yahoo_symbol_in_any_case() {
        let info = pair_info("eur", "Usd").unwrap();
        assert_eq!(info.code, "EURUSD");
        assert_eq!(info.yahoo_symbol, "EURUSD=X");
        assert_eq!(info.name, "EUR/USD");
        assert_eq!(info.quote, "USD");
    }

    #[test]
    fn cross_between_non_brl_currencies_is_valid() {
        assert_eq!(pair_info("GBP", "JPY").unwrap().yahoo_symbol, "GBPJPY=X");
    }

    #[test]
    fn same_currency_is_rejected() {
        assert!(matches!(pair_info("BRL", "brl"), Err(AppError::NoData(_))));
    }

    #[test]
    fn currency_outside_the_catalog_is_rejected() {
        assert!(matches!(pair_info("VND", "USD"), Err(AppError::NoData(_))));
        assert!(matches!(pair_info("USD", "XYZ"), Err(AppError::NoData(_))));
    }

    #[test]
    fn catalog_yields_272_pairs() {
        let valid = CURRENCIES
            .iter()
            .flat_map(|(b, _)| CURRENCIES.iter().map(move |(q, _)| (*b, *q)))
            .filter(|(b, q)| pair_info(b, q).is_ok())
            .count();
        assert_eq!(valid, 17 * 16);
    }
}
