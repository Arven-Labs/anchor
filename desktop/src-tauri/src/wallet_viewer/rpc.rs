//! Leitura de saldo nativo por JSON-RPC público. O parser é puro (testado com payload real);
//! a chamada de rede fica numa função fina, só testada ao vivo.

use alloy_primitives::U256;

use crate::error::AppError;
use crate::finance_core::http::{check_status, DEFAULT_TIMEOUT, USER_AGENT};

/// Uma rede EVM e os RPCs públicos que a atendem, em ordem de preferência. Medidos em
/// 2026-11-13 (ver SESSIONS.md); sem chave de API, que não pode ir no código.
pub struct Network {
    pub name: &'static str,
    pub endpoints: &'static [&'static str],
}

pub const ETHEREUM: Network =
    Network { name: "Ethereum", endpoints: &["https://ethereum-rpc.publicnode.com", "https://eth.drpc.org"] };

pub const BASE: Network =
    Network { name: "Base", endpoints: &["https://mainnet.base.org", "https://base-rpc.publicnode.com"] };

/// Corpo JSON-RPC de `eth_getBalance` para o endereço, no bloco `latest`.
pub fn balance_request_body(address: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getBalance",
        "params": [address, "latest"],
    })
    .to_string()
}

/// Lê a resposta de `eth_getBalance` e devolve o saldo em **wei**, sem perda (`U256`, nunca `f64`).
pub fn parse_balance_response(body: &str) -> Result<U256, AppError> {
    let value: serde_json::Value = serde_json::from_str(body)?;
    if let Some(error) = value.get("error") {
        let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
        return Err(AppError::Rpc(message.to_string()));
    }
    let result = value
        .get("result")
        .and_then(|r| r.as_str())
        .ok_or_else(|| AppError::Rpc("response has no string `result`".to_string()))?;
    let hex = result
        .strip_prefix("0x")
        .filter(|hex| !hex.is_empty())
        .ok_or_else(|| AppError::Rpc(format!("balance `{result}` is not a 0x-prefixed hex number")))?;
    U256::from_str_radix(hex, 16).map_err(|e| AppError::Rpc(format!("invalid balance `{result}`: {e}")))
}

/// Saldo nativo (em wei) do endereço. Tenta cada RPC da rede em ordem e devolve o primeiro que
/// responde; se todos falharem, o erro do último.
pub async fn fetch_balance(network: &Network, address: &str) -> Result<U256, AppError> {
    let body = balance_request_body(address);
    let mut last_error = AppError::ProviderUnavailable(format!("{}: no RPC endpoint configured", network.name));
    for endpoint in network.endpoints {
        match post_balance(network.name, endpoint, &body).await {
            Ok(balance) => return Ok(balance),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

async fn post_balance(source: &str, url: &str, body: &str) -> Result<U256, AppError> {
    let unavailable = |detail: reqwest::Error| AppError::ProviderUnavailable(format!("{source}: {detail}"));
    let client = reqwest::Client::builder().timeout(DEFAULT_TIMEOUT).user_agent(USER_AGENT).build().map_err(unavailable)?;
    let response = client
        .post(url)
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(unavailable)?;
    let status = response.status().as_u16();
    let text = response.text().await.map_err(unavailable)?;
    parse_balance_response(&check_status(source, status, text)?)
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
