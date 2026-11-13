//! Leitura de saldo nativo por JSON-RPC público. O parser é puro (testado com payload real);
//! a chamada de rede fica numa função fina, só testada ao vivo.

use alloy_primitives::U256;

use crate::error::AppError;

/// Corpo JSON-RPC de `eth_getBalance` para o endereço, no bloco `latest`.
pub fn balance_request_body(_address: &str) -> String {
    todo!("Etapa 3 do plano do visualizador de wallets")
}

/// Lê a resposta de `eth_getBalance` e devolve o saldo em **wei**, sem perda (`U256`, nunca `f64`).
pub fn parse_balance_response(_body: &str) -> Result<U256, AppError> {
    todo!("Etapa 3 do plano do visualizador de wallets")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Respostas reais medidas em 2026-11-13 para o endereço de queima 0x…dEaD.
    const ETH_DEAD: &str = r#"{"jsonrpc":"2.0","id":1,"result":"0x2ad5dbef146015eaecc"}"#;
    const BASE_DEAD: &str = r#"{"jsonrpc":"2.0","id":1,"result":"0x3fb108eaf0db205a"}"#;
    const RPC_ERROR: &str = r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error"},"id":1}"#;

    #[test]
    fn real_ethereum_response_is_parsed_in_wei() {
        assert_eq!(
            parse_balance_response(ETH_DEAD).unwrap(),
            "12642774792265118166732".parse::<U256>().unwrap()
        );
    }

    #[test]
    fn real_base_response_is_parsed_in_wei() {
        assert_eq!(parse_balance_response(BASE_DEAD).unwrap(), U256::from(4_589_459_300_423_508_058_u64));
    }

    #[test]
    fn two_ether_is_exactly_two_times_ten_to_the_eighteenth() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":"0x1bc16d674ec80000"}"#;
        assert_eq!(parse_balance_response(body).unwrap(), U256::from(2_000_000_000_000_000_000_u64));
    }

    #[test]
    fn zero_balance_is_zero() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":"0x0"}"#;
        assert_eq!(parse_balance_response(body).unwrap(), U256::ZERO);
    }

    #[test]
    fn maximum_u256_is_parsed_without_overflow() {
        let body = format!(r#"{{"jsonrpc":"2.0","id":1,"result":"0x{}"}}"#, "f".repeat(64));
        assert_eq!(parse_balance_response(&body).unwrap(), U256::MAX);
    }

    #[test]
    fn a_value_wider_than_256_bits_is_rejected() {
        let body = format!(r#"{{"jsonrpc":"2.0","id":1,"result":"0x1{}"}}"#, "0".repeat(64));
        assert!(matches!(parse_balance_response(&body), Err(AppError::Rpc(_))));
    }

    #[test]
    fn json_rpc_error_object_becomes_an_rpc_error_with_its_message() {
        match parse_balance_response(RPC_ERROR) {
            Err(AppError::Rpc(message)) => assert!(message.contains("Internal error"), "{message}"),
            other => panic!("expected Rpc error, got {other:?}"),
        }
    }

    #[test]
    fn empty_hex_after_the_prefix_is_rejected() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":"0x"}"#;
        assert!(matches!(parse_balance_response(body), Err(AppError::Rpc(_))));
    }

    #[test]
    fn result_without_0x_prefix_is_rejected() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":"2ad5"}"#;
        assert!(matches!(parse_balance_response(body), Err(AppError::Rpc(_))));
    }

    #[test]
    fn non_hex_result_is_rejected() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":"0xzz"}"#;
        assert!(matches!(parse_balance_response(body), Err(AppError::Rpc(_))));
    }

    #[test]
    fn missing_result_and_non_string_result_are_rejected() {
        assert!(matches!(parse_balance_response(r#"{"jsonrpc":"2.0","id":1}"#), Err(AppError::Rpc(_))));
        assert!(matches!(
            parse_balance_response(r#"{"jsonrpc":"2.0","id":1,"result":12}"#),
            Err(AppError::Rpc(_))
        ));
    }

    #[test]
    fn body_that_is_not_json_is_rejected() {
        // Um 525 do Cloudflare chega como texto, não como JSON.
        assert!(matches!(parse_balance_response("error code: 525"), Err(AppError::Json(_))));
        assert!(matches!(parse_balance_response(""), Err(AppError::Json(_))));
    }

    #[test]
    fn request_body_asks_for_the_latest_balance_of_that_address() {
        let address = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
        let value: serde_json::Value = serde_json::from_str(&balance_request_body(address)).unwrap();
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["method"], "eth_getBalance");
        assert_eq!(value["params"], serde_json::json!([address, "latest"]));
    }
}
