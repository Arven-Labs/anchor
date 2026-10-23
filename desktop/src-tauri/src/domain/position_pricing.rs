// Fase 13.1 — precifica uma posição pra alimentar o Resumo da Carteira.
// Função pura, mesmo espírito de `domain/twr.rs`/`domain/dividend_suggestion.rs`:
// o comando (`commands/transaction.rs::compute_positions`) busca os preços/
// avaliações no banco, esta função só decide qual fonte usar e faz a conta.
//
// Mesma lista de classes com cotação automática que `commands/profitability.rs`
// já usa pro cálculo de rentabilidade (decisão histórica documentada lá:
// Tesouro Direto/Renda Fixa/Imóvel/Empresa não listada ficam fora).
pub const ASSET_CLASSES_WITH_AUTO_QUOTE: [&str; 10] = [
    "acao_br",
    "fii",
    "etf_br",
    "etf_renda_fixa",
    "cripto",
    "bdr",
    "metal",
    "acao_internacional",
    "reit",
    "etf_us",
];
const MANUAL_VALUATION_CLASSES: [&str; 2] = ["imovel", "empresa_nao_listada"];
// Fundos: sem cotação de mercado, mas com valor de cota lançado pelo usuário (`fund_quota_values`), que entra no lugar
// do preço da cotação automática.
const MANUAL_QUOTA_CLASSES: [&str; 1] = ["fundo"];

#[derive(Debug, Clone, PartialEq)]
pub struct PricingResult {
    pub current_price: Option<f64>,
    pub market_value: Option<f64>,
    pub unrealized_pl: Option<f64>,
    pub unrealized_pl_pct: Option<f64>,
    pub price_source: String,
}

// `latest_ticker_price` só é considerado pras classes com cotação automática
// (mesmo se um valor "vazasse" de outra fonte, não faz sentido usá-lo fora
// delas). `latest_manual_valuation` só entra pra imóvel/empresa não listada
// — é o valor total da última reavaliação (`asset_valuations.value`), não
// um preço por unidade, então `current_price` fica `None` nesse caso (não
// dá pra dividir por quantidade sem presumir que 1 unidade = 1 ativo).
// Fallback final: preço médio de compra (sinaliza "sem cotação ao vivo" via
// `price_source = "avg_buy_price"`) — é o único caminho pra Tesouro Direto/
// Renda Fixa hoje, e também cobre imóvel/empresa sem nenhuma reavaliação
// cadastrada ainda.
pub fn price_position(
    asset_class: &str,
    quantity: f64,
    average_buy_price: Option<f64>,
    latest_ticker_price: Option<f64>,
    latest_manual_valuation: Option<f64>,
) -> PricingResult {
    let is_auto_quote = ASSET_CLASSES_WITH_AUTO_QUOTE.contains(&asset_class);
    let is_manual_valuation_class = MANUAL_VALUATION_CLASSES.contains(&asset_class);
    let is_manual_quota_class = MANUAL_QUOTA_CLASSES.contains(&asset_class);

    let (current_price, market_value, price_source) =
        if is_auto_quote && latest_ticker_price.is_some() {
            let price = latest_ticker_price.unwrap();
            (Some(price), Some(price * quantity), "market".to_string())
        } else if is_manual_quota_class && latest_ticker_price.is_some() {
            let price = latest_ticker_price.unwrap();
            (Some(price), Some(price * quantity), "manual_quota".to_string())
        } else if is_manual_valuation_class && latest_manual_valuation.is_some() {
            (None, latest_manual_valuation, "manual_valuation".to_string())
        } else if let Some(avg_price) = average_buy_price {
            (
                Some(avg_price),
                Some(avg_price * quantity),
                "avg_buy_price".to_string(),
            )
        } else {
            (None, None, "none".to_string())
        };

    let cost_basis = average_buy_price.map(|avg| avg * quantity);
    let (unrealized_pl, unrealized_pl_pct) = match (market_value, cost_basis) {
        (Some(mv), Some(cb)) => {
            let pl = mv - cb;
            let pl_pct = (cb != 0.0).then_some(pl / cb.abs());
            (Some(pl), pl_pct)
        }
        _ => (None, None),
    };

    PricingResult {
        current_price,
        market_value,
        unrealized_pl,
        unrealized_pl_pct,
        price_source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_quote_class_uses_market_price() {
        let result = price_position("acao_br", 10.0, Some(20.0), Some(25.0), None);

        assert_eq!(result.current_price, Some(25.0));
        assert_eq!(result.market_value, Some(250.0));
        assert_eq!(result.price_source, "market");
        assert_eq!(result.unrealized_pl, Some(50.0));
        assert_eq!(result.unrealized_pl_pct, Some(0.25));
    }

    #[test]
    fn auto_quote_class_without_price_falls_back_to_avg_buy_price() {
        let result = price_position("acao_br", 10.0, Some(20.0), None, None);

        assert_eq!(result.current_price, Some(20.0));
        assert_eq!(result.market_value, Some(200.0));
        assert_eq!(result.price_source, "avg_buy_price");
        assert_eq!(result.unrealized_pl, Some(0.0));
        assert_eq!(result.unrealized_pl_pct, Some(0.0));
    }

    #[test]
    fn real_estate_uses_latest_manual_valuation_as_total_market_value() {
        let result = price_position("imovel", 1.0, Some(300_000.0), None, Some(360_000.0));

        assert_eq!(result.current_price, None);
        assert_eq!(result.market_value, Some(360_000.0));
        assert_eq!(result.price_source, "manual_valuation");
        assert_eq!(result.unrealized_pl, Some(60_000.0));
    }

    #[test]
    fn real_estate_without_valuation_falls_back_to_avg_buy_price() {
        let result = price_position("empresa_nao_listada", 1.0, Some(50_000.0), None, None);

        assert_eq!(result.market_value, Some(50_000.0));
        assert_eq!(result.price_source, "avg_buy_price");
    }

    #[test]
    fn fixed_income_classes_never_use_ticker_or_manual_valuation() {
        let result = price_position("tesouro_direto", 5.0, Some(100.0), Some(999.0), Some(999.0));

        assert_eq!(result.current_price, Some(100.0));
        assert_eq!(result.market_value, Some(500.0));
        assert_eq!(result.price_source, "avg_buy_price");
    }

    #[test]
    fn nothing_available_returns_all_none() {
        let result = price_position("tesouro_direto", 5.0, None, None, None);

        assert_eq!(result.current_price, None);
        assert_eq!(result.market_value, None);
        assert_eq!(result.unrealized_pl, None);
        assert_eq!(result.unrealized_pl_pct, None);
        assert_eq!(result.price_source, "none");
    }

    #[test]
    fn zero_quantity_still_prices_but_yields_zero_value() {
        let result = price_position("acao_br", 0.0, Some(20.0), Some(25.0), None);

        assert_eq!(result.market_value, Some(0.0));
        assert_eq!(result.unrealized_pl, Some(0.0));
    }
}
