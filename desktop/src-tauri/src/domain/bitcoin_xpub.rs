//! Chave pública estendida de Bitcoin (xpub legado, zpub SegWit nativo, só mainnet) e a derivação
//! dos endereços dela. Função pura: entra texto, sai a chave validada ou `InvalidAddress`. O
//! visualizador de wallets (ADR 0002) só lê; nada aqui aceita ou guarda chave privada.

use bitcoin::bip32::{ChildNumber, Xpub};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, CompressedPublicKey, Network};
use serde::Serialize;

use crate::error::AppError;

/// Bytes de versão do BIP32 (mainnet) e do SLIP-132 para `zpub`.
const XPUB_VERSION: [u8; 4] = [0x04, 0x88, 0xB2, 0x1E];
const ZPUB_VERSION: [u8; 4] = [0x04, 0xB2, 0x47, 0x46];
/// Tamanho da chave estendida serializada (sem o checksum).
const EXTENDED_KEY_LEN: usize = 78;
const MAX_TEXT_LEN: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum XpubKind {
    /// `xpub...`: endereços P2PKH (`1...`).
    Legacy,
    /// `zpub...`: endereços P2WPKH (`bc1q...`).
    NativeSegwit,
}

/// As duas cadeias de uma conta (BIP44): `0` recebimento, `1` troco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Receive,
    Change,
}

#[derive(Debug, Clone)]
pub struct ParsedXpub {
    /// A chave como foi validada (sem espaços nas pontas).
    pub canonical: String,
    pub kind: XpubKind,
    key: bitcoin::bip32::Xpub,
}

/// Aceita só xpub ou zpub de mainnet, com checksum válido, 78 bytes e ponto na curva. Recusa
/// chave privada (`xprv`, `zprv`), testnet, `ypub` e qualquer outra coisa. A mensagem de erro
/// nunca repete o texto digitado (pode ser uma chave privada colada por engano).
pub fn parse_xpub(input: &str) -> Result<ParsedXpub, AppError> {
    let text = input.trim();
    let invalid = |why: &str| AppError::InvalidAddress(format!("not a valid xpub or zpub: {why}"));

    // Uma xpub em base58check tem 111 ou 112 caracteres; o teto evita decodificar texto enorme
    // (a decodificação base58 é quadrática).
    if text.is_empty() || text.len() > MAX_TEXT_LEN {
        return Err(invalid("expected an xpub or zpub"));
    }
    let mut bytes =
        bitcoin::base58::decode_check(text).map_err(|_| invalid("bad characters or checksum"))?;
    if bytes.len() != EXTENDED_KEY_LEN {
        return Err(invalid("wrong length"));
    }

    let version = [bytes[0], bytes[1], bytes[2], bytes[3]];
    let kind = if version == XPUB_VERSION {
        XpubKind::Legacy
    } else if version == ZPUB_VERSION {
        XpubKind::NativeSegwit
    } else {
        return Err(invalid("only mainnet xpub and zpub are supported (not ypub, testnet or private keys)"));
    };

    // A crate só conhece a versão `xpub`: a zpub é a mesma chave com outros 4 bytes de versão.
    bytes[..4].copy_from_slice(&XPUB_VERSION);
    let key = Xpub::decode(&bytes).map_err(|_| invalid("not a valid public key"))?;
    Ok(ParsedXpub { canonical: text.to_string(), kind, key })
}

/// Satoshis como texto decimal exato em BTC (8 casas, sinal só se negativo). Inteiros, nunca `f64`.
pub fn format_btc(satoshis: i128) -> String {
    let sign = if satoshis < 0 { "-" } else { "" };
    let abs = satoshis.unsigned_abs();
    format!("{sign}{}.{:08}", abs / 100_000_000, abs % 100_000_000)
}

impl ParsedXpub {
    /// Endereço `m/<cadeia>/<índice>` a partir da chave da conta. Índice endurecido (>= 2^31) não
    /// existe em derivação pública e vira erro.
    pub fn address(&self, chain: Chain, index: u32) -> Result<String, AppError> {
        let cannot = |_| AppError::InvalidAddress("cannot derive that address index".to_string());
        let chain_number = match chain {
            Chain::Receive => 0,
            Chain::Change => 1,
        };
        let path = [
            ChildNumber::from_normal_idx(chain_number).map_err(cannot)?,
            ChildNumber::from_normal_idx(index).map_err(cannot)?,
        ];
        let child = self.key.derive_pub(&Secp256k1::verification_only(), &path).map_err(cannot)?;
        let public_key = CompressedPublicKey(child.public_key);
        let address = match self.kind {
            XpubKind::Legacy => Address::p2pkh(public_key, Network::Bitcoin),
            XpubKind::NativeSegwit => Address::p2wpkh(&public_key, Network::Bitcoin),
        };
        Ok(address.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // BIP84, mnemônico "abandon ... about", conta 0 (vetor do próprio BIP84).
    const ZPUB: &str = "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs";
    const ZPUB_RECEIVE_0: &str = "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu";
    const ZPUB_RECEIVE_1: &str = "bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g";
    const ZPUB_CHANGE_0: &str = "bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el";

    // BIP44, mesmo mnemônico, `m/44'/0'/0'`; endereços conferidos com uma implementação independente.
    const XPUB: &str = "xpub6BosfCnifzxcFwrSzQiqu2DBVTshkCXacvNsWGYJVVhhawA7d4R5WSWGFNbi8Aw6ZRc1brxMyWMzG3DSSSSoekkudhUd9yLb6qx39T9nMdj";
    const XPUB_RECEIVE_0: &str = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA";
    const XPUB_RECEIVE_1: &str = "1Ak8PffB2meyfYnbXZR9EGfLfFZVpzJvQP";
    const XPUB_CHANGE_0: &str = "1J3J6EvPrv8q6AC3VCjWV45Uf3nssNMRtH";

    // Mesma chave de conta com outra versão, ou com o corpo adulterado e o checksum refeito.
    const YPUB: &str = "ypub6XR9pJPUsVBFKweLeV85HtwdxjjmKEuUr6djm9mNdkh47X7ASsD6byaXFotRAKByFoWgSzCuoTjaYdrv2yoJroLAPtBuHFjVm5vNmhyNehE";
    const TPUB: &str = "tpubDCxX2sYFS5bDkSe5GKKYHjBW7tgyN1R3UchpLJvdbf54ohxeGRtd8MbDUe1cguVHe4vnK68DsuD5MXjxi9EXx16rb9EnNsaF5KT99CinaJz";
    const VPUB: &str = "vpub5YvMuJNjRSYon44z9QmCfdf8SqJRVNvz6m55Qy5iVjZQxDfUgtiQjnc7CC1fAbED2tAGCZRERUfvtn2DstZGU6HMns6dXXH2wujSc2wfi2x";
    // Chave privada: master do vetor 1 do BIP32, e a mesma com a versão de zprv.
    const XPRV: &str = "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi";
    const ZPRV: &str = "zprvAWgYBBk7JR8GjzqSzmunMCS7dAbwpYTCs1YUMDXqduMA5JFHZ3iX5s2UkAR6vBdcCYYa1S5o1fVLrKsrnpCQ4WpUd6aVUWP1bS2Yy5DoaKv";
    const ZPUB_SHORT: &str = "E6sRwo3p9q3amDdBv53NQXZnRh7NnFFKdu5ypB1Dat8JuHL1wk2g8NP8JTLQdFh6vkEo9FsQcDsQPuovnyXEzUTopazGU1eukzSpp5kjNTaKSm";
    const ZPUB_LONG: &str = "5QET7WpNdTkRzc9uBAC4sstsbkvkSu1T36bERY8eYAEjsGMWN5TYfBs4biYsHwgjHhhgo2g83TbJ7KkHdo12HVn5PLstNNBsYGRAAizNg7rWV3W7s";
    const ZPUB_OFF_CURVE: &str = "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH2wDEa95H44VEpYzCHiSFgt5JeXEKXbPCUTxQWprrjMLeytW9yL";
    const ZPUB_BAD_KEY_PREFIX: &str = "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH7gEgvBzDifAbvYayrx8HtGoW24Uc9YPd96ebccg1n1BeBLMZZa";
    const ZPUB_BAD_CHECKSUM: &str = "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYr";

    fn is_invalid(result: Result<ParsedXpub, AppError>) -> bool {
        matches!(result, Err(AppError::InvalidAddress(_)))
    }

    #[test]
    fn zpub_is_native_segwit_and_derives_the_bip84_addresses() {
        let key = parse_xpub(ZPUB).unwrap();
        assert_eq!(key.kind, XpubKind::NativeSegwit);
        assert_eq!(key.address(Chain::Receive, 0).unwrap(), ZPUB_RECEIVE_0);
        assert_eq!(key.address(Chain::Receive, 1).unwrap(), ZPUB_RECEIVE_1);
        assert_eq!(key.address(Chain::Change, 0).unwrap(), ZPUB_CHANGE_0);
    }

    #[test]
    fn xpub_is_legacy_and_derives_p2pkh_addresses() {
        let key = parse_xpub(XPUB).unwrap();
        assert_eq!(key.kind, XpubKind::Legacy);
        assert_eq!(key.address(Chain::Receive, 0).unwrap(), XPUB_RECEIVE_0);
        assert_eq!(key.address(Chain::Receive, 1).unwrap(), XPUB_RECEIVE_1);
        assert_eq!(key.address(Chain::Change, 0).unwrap(), XPUB_CHANGE_0);
    }

    #[test]
    fn receive_and_change_chains_give_different_addresses() {
        let key = parse_xpub(ZPUB).unwrap();
        assert_ne!(key.address(Chain::Receive, 0).unwrap(), key.address(Chain::Change, 0).unwrap());
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_and_the_canonical_form_is_kept() {
        let key = parse_xpub(&format!("  {ZPUB}\n")).unwrap();
        assert_eq!(key.canonical, ZPUB);
    }

    #[test]
    fn a_checksum_typo_is_rejected() {
        assert!(is_invalid(parse_xpub(ZPUB_BAD_CHECKSUM)));
    }

    #[test]
    fn secret_extended_keys_are_rejected_and_never_echoed() {
        for private in [XPRV, ZPRV] {
            match parse_xpub(private) {
                Err(AppError::InvalidAddress(message)) => {
                    assert!(!message.contains(&private[..20]), "the message must not repeat the key: {message}");
                }
                other => panic!("expected InvalidAddress, got {other:?}"),
            }
        }
    }

    #[test]
    fn unsupported_versions_are_rejected() {
        // ypub (SegWit embrulhado) fica fora do 36.4a; tpub e vpub são testnet.
        for key in [YPUB, TPUB, VPUB] {
            assert!(is_invalid(parse_xpub(key)), "{key}");
        }
    }

    #[test]
    fn wrong_length_is_rejected() {
        assert!(is_invalid(parse_xpub(ZPUB_SHORT)));
        assert!(is_invalid(parse_xpub(ZPUB_LONG)));
    }

    #[test]
    fn a_key_that_is_not_a_curve_point_is_rejected() {
        assert!(is_invalid(parse_xpub(ZPUB_OFF_CURVE)));
        assert!(is_invalid(parse_xpub(ZPUB_BAD_KEY_PREFIX)));
    }

    #[test]
    fn other_texts_are_rejected() {
        let huge = "z".repeat(10_000);
        for text in [
            "",
            "   ",
            "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu", // endereço, não chave
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed", // endereço Ethereum
            "zpub0OIl", // fora do alfabeto base58
            huge.as_str(),
        ] {
            assert!(is_invalid(parse_xpub(text)), "{}", &text[..text.len().min(20)]);
        }
    }

    #[test]
    fn satoshis_are_shown_as_exact_btc_with_eight_decimals() {
        assert_eq!(format_btc(0), "0.00000000");
        assert_eq!(format_btc(1), "0.00000001");
        assert_eq!(format_btc(18_231_475), "0.18231475");
        assert_eq!(format_btc(123_456_789), "1.23456789");
        assert_eq!(format_btc(2_100_000_000_000_000), "21000000.00000000");
        assert_eq!(format_btc(-50), "-0.00000050");
        assert_eq!(format_btc(i128::from(u64::MAX)), "184467440737.09551615");
    }

    #[test]
    fn hardened_indexes_are_an_error_not_a_panic() {
        let key = parse_xpub(ZPUB).unwrap();
        assert!(key.address(Chain::Receive, 0x8000_0000).is_err());
        assert!(key.address(Chain::Receive, u32::MAX).is_err());
        // O último índice não endurecido ainda deriva.
        assert!(key.address(Chain::Receive, 0x7fff_ffff).is_ok());
    }
}
