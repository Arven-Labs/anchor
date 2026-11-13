//! Validação de endereço de wallet EVM (Ethereum, Base). Função pura: entra texto, sai o
//! endereço canônico (checksum EIP-55) ou `InvalidAddress`. O módulo "visualizador de
//! wallets" (ADR 0002) só lê endereços públicos; nada aqui toca chave privada.

use crate::error::AppError;

/// Aceita `0x` + 40 hex, com espaços nas pontas. Tudo minúsculo ou tudo maiúsculo vale
/// (não carrega checksum); com letras misturadas o checksum EIP-55 tem de bater. Devolve o
/// endereço no formato com checksum.
pub fn parse_address(_input: &str) -> Result<String, AppError> {
    todo!("Etapa 3 do plano do visualizador de wallets")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vetor do próprio texto do EIP-55.
    const CHECKSUMMED: &str = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";

    fn is_invalid_address(result: Result<String, AppError>) -> bool {
        matches!(result, Err(AppError::InvalidAddress(_)))
    }

    #[test]
    fn checksummed_address_is_accepted_unchanged() {
        assert_eq!(parse_address(CHECKSUMMED).unwrap(), CHECKSUMMED);
    }

    #[test]
    fn all_lowercase_address_is_accepted_and_gets_a_checksum() {
        assert_eq!(parse_address(&CHECKSUMMED.to_lowercase()).unwrap(), CHECKSUMMED);
    }

    #[test]
    fn all_uppercase_hex_is_accepted_and_gets_a_checksum() {
        let upper = format!("0x{}", CHECKSUMMED[2..].to_uppercase());
        assert_eq!(parse_address(&upper).unwrap(), CHECKSUMMED);
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(parse_address(&format!("  {CHECKSUMMED}\n")).unwrap(), CHECKSUMMED);
    }

    #[test]
    fn mixed_case_with_a_wrong_checksum_is_rejected() {
        // Só a caixa do primeiro "a" mudou: pode ser um erro de digitação.
        assert!(is_invalid_address(parse_address("0x5AAeb6053F3E94C9b9A09f33669435E7Ef1BeAed")));
    }

    #[test]
    fn zero_address_is_accepted() {
        let zero = "0x0000000000000000000000000000000000000000";
        assert_eq!(parse_address(zero).unwrap(), zero);
    }

    #[test]
    fn too_short_is_rejected() {
        assert!(is_invalid_address(parse_address("0x5aaeb6053f3e94c9b9a09f33669435e7ef1beae")));
    }

    #[test]
    fn too_long_is_rejected() {
        assert!(is_invalid_address(parse_address("0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed0")));
    }

    #[test]
    fn missing_0x_prefix_is_rejected() {
        assert!(is_invalid_address(parse_address("5aaeb6053f3e94c9b9a09f33669435e7ef1beaed")));
    }

    #[test]
    fn non_hex_character_is_rejected() {
        assert!(is_invalid_address(parse_address("0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaeg")));
    }

    #[test]
    fn empty_and_blank_are_rejected() {
        assert!(is_invalid_address(parse_address("")));
        assert!(is_invalid_address(parse_address("   ")));
    }

    #[test]
    fn ens_name_is_rejected() {
        // Nome ENS exigiria uma consulta de rede; fica fora do primeiro PR.
        assert!(is_invalid_address(parse_address("vitalik.eth")));
    }
}
