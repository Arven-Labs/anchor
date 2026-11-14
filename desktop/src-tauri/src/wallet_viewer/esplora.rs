//! Saldo de Bitcoin por indexador Esplora público (API `/address/<endereço>`), endereço a
//! endereço: a xpub nunca sai da máquina, só os endereços derivados. O parser é puro (testado
//! com payload real); a varredura do gap limit roda sobre uma fronteira (`AddressSource`) que os
//! testes trocam por um falso; a chamada de rede fica numa função fina, só testada ao vivo.

use serde::{Deserialize, Serialize};

use crate::domain::bitcoin_xpub::{Chain, ParsedXpub};
use crate::error::AppError;
use crate::finance_core::http::{get_text, DEFAULT_TIMEOUT};

/// Endereços consecutivos sem nenhum uso, por cadeia, que encerram a varredura (convenção BIP44).
pub const GAP_LIMIT: u32 = 20;
/// Teto de endereços varridos por cadeia, para uma chave sempre "usada" não varrer sem fim.
pub const MAX_ADDRESSES_PER_CHAIN: u32 = 200;

/// O que o indexador diz de um endereço.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AddressStats {
    /// Saldo confirmado em satoshis (`funded - spent` na cadeia).
    pub confirmed_sat: u64,
    /// Variação ainda no mempool, em satoshis (pode ser negativa: gasto de saída confirmada).
    pub pending_sat: i64,
    /// Transações confirmadas mais as do mempool. Zero = endereço nunca usado.
    pub tx_count: u64,
}

/// Lê a resposta de `/address/<endereço>` do Esplora. Valores em satoshis, inteiros, nunca `f64`.
pub fn parse_address_response(body: &str) -> Result<AddressStats, AppError> {
    let response: Response = serde_json::from_str(body)?;
    let confirmed_sat = response
        .chain_stats
        .funded_txo_sum
        .checked_sub(response.chain_stats.spent_txo_sum)
        .ok_or_else(|| AppError::Rpc("the provider reports more spent than funded".to_string()))?;
    let out_of_range = || AppError::Rpc("the pending amount is out of range".to_string());
    let pending_funded = i64::try_from(response.mempool_stats.funded_txo_sum).map_err(|_| out_of_range())?;
    let pending_spent = i64::try_from(response.mempool_stats.spent_txo_sum).map_err(|_| out_of_range())?;
    let pending_sat = pending_funded.checked_sub(pending_spent).ok_or_else(out_of_range)?;
    let tx_count = response.chain_stats.tx_count.saturating_add(response.mempool_stats.tx_count);
    Ok(AddressStats { confirmed_sat, pending_sat, tx_count })
}

#[derive(Deserialize)]
struct Response {
    chain_stats: Counters,
    mempool_stats: Counters,
}

#[derive(Deserialize)]
struct Counters {
    funded_txo_sum: u64,
    spent_txo_sum: u64,
    tx_count: u64,
}

/// Fronteira de I/O da varredura: o real consulta o Esplora, o falso dos testes responde de um mapa.
pub trait AddressSource {
    async fn stats(&self, address: &str) -> Result<AddressStats, AppError>;
}

/// APIs Esplora públicas, em ordem de preferência. Medidas em 2026-11-14: as duas devolvem o
/// mesmo JSON em `/address/<endereço>`, sem chave de API.
const ENDPOINTS: &[(&str, &str)] =
    &[("mempool.space", "https://mempool.space/api"), ("blockstream.info", "https://blockstream.info/api")];

/// O Esplora de verdade. Tenta cada API em ordem e devolve a primeira que responde; se todas
/// falharem, o erro da última. Só testado ao vivo, como o RPC do Ethereum.
pub struct EsploraSource;

impl AddressSource for EsploraSource {
    async fn stats(&self, address: &str) -> Result<AddressStats, AppError> {
        let mut last_error = AppError::ProviderUnavailable("no Esplora endpoint configured".to_string());
        for (name, base) in ENDPOINTS {
            let url = format!("{base}/address/{address}");
            match get_text(name, &url, DEFAULT_TIMEOUT).await.and_then(|body| parse_address_response(&body)) {
                Ok(stats) => return Ok(stats),
                Err(error) => last_error = error,
            }
        }
        Err(last_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BitcoinBalance {
    pub confirmed_sat: u64,
    pub pending_sat: i64,
    /// Endereços com histórico (`tx_count > 0`) entre os varridos.
    pub used_addresses: u32,
    /// Endereços consultados, nas duas cadeias.
    pub scanned_addresses: u32,
    /// `true` se alguma cadeia bateu no teto `MAX_ADDRESSES_PER_CHAIN` sem fechar o gap: o saldo
    /// pode estar subestimado.
    pub truncated: bool,
}

/// Varre recebimento e troco até `GAP_LIMIT` endereços seguidos sem uso (ou o teto) e soma. Um
/// erro de qualquer consulta aborta tudo: nunca devolve saldo parcial como se fosse completo.
pub async fn scan_balance<S: AddressSource>(xpub: &ParsedXpub, source: &S) -> Result<BitcoinBalance, AppError> {
    let overflow = || AppError::Rpc("the total balance does not fit in 64 bits".to_string());
    let mut total = BitcoinBalance {
        confirmed_sat: 0,
        pending_sat: 0,
        used_addresses: 0,
        scanned_addresses: 0,
        truncated: false,
    };
    for chain in [Chain::Receive, Chain::Change] {
        let mut index = 0;
        let mut unused_in_a_row = 0;
        while unused_in_a_row < GAP_LIMIT {
            if index >= MAX_ADDRESSES_PER_CHAIN {
                total.truncated = true;
                break;
            }
            let stats = source.stats(&xpub.address(chain, index)?).await?;
            total.scanned_addresses += 1;
            total.confirmed_sat = total.confirmed_sat.checked_add(stats.confirmed_sat).ok_or_else(overflow)?;
            total.pending_sat = total.pending_sat.checked_add(stats.pending_sat).ok_or_else(overflow)?;
            if stats.tx_count > 0 {
                total.used_addresses += 1;
                unused_in_a_row = 0;
            } else {
                unused_in_a_row += 1;
            }
            index += 1;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;
    use crate::domain::bitcoin_xpub::parse_xpub;

    // Respostas reais do mempool.space medidas em 2026-11-14 (o blockstream.info devolve o mesmo JSON).
    // Endereço com saldo:
    const WITH_BALANCE: &str = r#"{"address":"bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq","chain_stats":{"funded_txo_count":126,"funded_txo_sum":18245768,"spent_txo_count":1,"spent_txo_sum":14293,"tx_count":127},"mempool_stats":{"funded_txo_count":0,"funded_txo_sum":0,"spent_txo_count":0,"spent_txo_sum":0,"tx_count":0}}"#;
    // Endereço usado e zerado (BIP84, receive 0):
    const USED_AND_EMPTY: &str = r#"{"address":"bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu","chain_stats":{"funded_txo_count":90,"funded_txo_sum":4083249,"spent_txo_count":90,"spent_txo_sum":4083249,"tx_count":180},"mempool_stats":{"funded_txo_count":0,"funded_txo_sum":0,"spent_txo_count":0,"spent_txo_sum":0,"tx_count":0}}"#;

    fn body(chain_funded: u64, chain_spent: u64, mempool_funded: u64, mempool_spent: u64, tx_count: u64) -> String {
        format!(
            r#"{{"address":"x","chain_stats":{{"funded_txo_count":1,"funded_txo_sum":{chain_funded},"spent_txo_count":0,"spent_txo_sum":{chain_spent},"tx_count":{tx_count}}},"mempool_stats":{{"funded_txo_count":0,"funded_txo_sum":{mempool_funded},"spent_txo_count":0,"spent_txo_sum":{mempool_spent},"tx_count":0}}}}"#
        )
    }

    // ------------------------------------------------------------ parser

    #[test]
    fn real_response_with_balance_is_funded_minus_spent_in_satoshis() {
        let stats = parse_address_response(WITH_BALANCE).unwrap();
        assert_eq!(stats.confirmed_sat, 18_245_768 - 14_293);
        assert_eq!(stats.pending_sat, 0);
        assert_eq!(stats.tx_count, 127);
    }

    #[test]
    fn real_response_of_a_used_and_emptied_address_is_zero_but_used() {
        let stats = parse_address_response(USED_AND_EMPTY).unwrap();
        assert_eq!(stats.confirmed_sat, 0);
        assert_eq!(stats.tx_count, 180);
    }

    #[test]
    fn a_never_used_address_has_no_transactions() {
        let stats = parse_address_response(&body(0, 0, 0, 0, 0)).unwrap();
        assert_eq!(stats, AddressStats::default());
    }

    #[test]
    fn mempool_is_reported_apart_from_the_confirmed_balance() {
        let stats = parse_address_response(&body(100_000, 0, 5_000, 0, 1)).unwrap();
        assert_eq!((stats.confirmed_sat, stats.pending_sat), (100_000, 5_000));
    }

    #[test]
    fn a_pending_spend_makes_the_pending_amount_negative() {
        let stats = parse_address_response(&body(100_000, 0, 0, 40_000, 1)).unwrap();
        assert_eq!((stats.confirmed_sat, stats.pending_sat), (100_000, -40_000));
    }

    #[test]
    fn an_address_with_only_a_mempool_transaction_counts_as_used() {
        let json = r#"{"address":"x","chain_stats":{"funded_txo_count":0,"funded_txo_sum":0,"spent_txo_count":0,"spent_txo_sum":0,"tx_count":0},"mempool_stats":{"funded_txo_count":1,"funded_txo_sum":700,"spent_txo_count":0,"spent_txo_sum":0,"tx_count":1}}"#;
        let stats = parse_address_response(json).unwrap();
        assert_eq!((stats.confirmed_sat, stats.pending_sat, stats.tx_count), (0, 700, 1));
    }

    #[test]
    fn spent_greater_than_funded_is_rejected() {
        assert!(matches!(parse_address_response(&body(10, 11, 0, 0, 1)), Err(AppError::Rpc(_))));
    }

    #[test]
    fn the_largest_u64_balance_parses_without_overflow() {
        let stats = parse_address_response(&body(u64::MAX, 0, 0, 0, 1)).unwrap();
        assert_eq!(stats.confirmed_sat, u64::MAX);
    }

    #[test]
    fn a_pending_amount_that_does_not_fit_in_i64_is_rejected() {
        assert!(matches!(parse_address_response(&body(0, 0, u64::MAX, 0, 1)), Err(AppError::Rpc(_))));
    }

    #[test]
    fn negative_or_fractional_amounts_are_rejected() {
        let negative = body(0, 0, 0, 0, 0).replace("\"funded_txo_sum\":0,\"spent_txo_count\":0", "\"funded_txo_sum\":-5,\"spent_txo_count\":0");
        assert!(parse_address_response(&negative).is_err());
        assert!(parse_address_response(&body(0, 0, 0, 0, 0).replace("\"funded_txo_sum\":0", "\"funded_txo_sum\":1.5")).is_err());
    }

    #[test]
    fn missing_fields_are_rejected() {
        assert!(parse_address_response(r#"{"address":"x"}"#).is_err());
        assert!(parse_address_response(r#"{"address":"x","chain_stats":{}}"#).is_err());
    }

    #[test]
    fn a_body_that_is_not_json_is_rejected() {
        // Um endereço inválido chega como 400 com texto puro: "Invalid Bitcoin address".
        assert!(matches!(parse_address_response("Invalid Bitcoin address"), Err(AppError::Json(_))));
        assert!(matches!(parse_address_response(""), Err(AppError::Json(_))));
    }

    // ------------------------------------------------------------ varredura (gap limit)

    // BIP84, abandon..about: o mesmo da camada de domínio.
    const ZPUB: &str = "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs";

    /// Responde de um mapa endereço -> estatística (padrão: nunca usado), conta as consultas e
    /// pode falhar num endereço.
    struct FakeSource {
        answers: HashMap<String, AddressStats>,
        every_address: Option<AddressStats>,
        failing: Option<String>,
        calls: RefCell<Vec<String>>,
    }

    impl FakeSource {
        fn new() -> Self {
            Self { answers: HashMap::new(), every_address: None, failing: None, calls: RefCell::new(Vec::new()) }
        }
        fn with(mut self, chain: Chain, index: u32, stats: AddressStats) -> Self {
            self.answers.insert(address(chain, index), stats);
            self
        }
        fn calls(&self) -> usize {
            self.calls.borrow().len()
        }
    }

    impl AddressSource for FakeSource {
        async fn stats(&self, address: &str) -> Result<AddressStats, AppError> {
            self.calls.borrow_mut().push(address.to_string());
            if self.failing.as_deref() == Some(address) {
                return Err(AppError::ProviderUnavailable("fake provider down".to_string()));
            }
            Ok(self.answers.get(address).copied().or(self.every_address).unwrap_or_default())
        }
    }

    fn address(chain: Chain, index: u32) -> String {
        parse_xpub(ZPUB).unwrap().address(chain, index).unwrap()
    }

    fn funded(confirmed_sat: u64) -> AddressStats {
        AddressStats { confirmed_sat, pending_sat: 0, tx_count: 1 }
    }

    /// Usado, mas sem saldo (recebeu e gastou tudo).
    fn used_empty() -> AddressStats {
        AddressStats { confirmed_sat: 0, pending_sat: 0, tx_count: 2 }
    }

    async fn scan(source: &FakeSource) -> Result<BitcoinBalance, AppError> {
        scan_balance(&parse_xpub(ZPUB).unwrap(), source).await
    }

    #[tokio::test]
    async fn funds_on_receive_and_change_addresses_are_summed() {
        let source = FakeSource::new()
            .with(Chain::Receive, 0, funded(1_000))
            .with(Chain::Receive, 1, funded(2_000))
            .with(Chain::Change, 0, funded(300));
        let balance = scan(&source).await.unwrap();
        assert_eq!(balance.confirmed_sat, 3_300);
        assert_eq!(balance.used_addresses, 3);
        assert!(!balance.truncated);
    }

    #[tokio::test]
    async fn a_wallet_with_no_history_scans_exactly_one_gap_per_chain() {
        let source = FakeSource::new();
        let balance = scan(&source).await.unwrap();
        assert_eq!(balance.confirmed_sat, 0);
        assert_eq!(balance.used_addresses, 0);
        assert_eq!(balance.scanned_addresses, 2 * GAP_LIMIT);
        assert_eq!(source.calls(), (2 * GAP_LIMIT) as usize);
    }

    #[tokio::test]
    async fn a_used_address_restarts_the_gap_even_with_zero_balance() {
        // Usado no índice 5 (zerado): a cadeia de recebimento vai até 5 + 20 = 25 (26 consultas);
        // o troco, sem uso, 20. Total 46.
        let source = FakeSource::new().with(Chain::Receive, 5, used_empty());
        let balance = scan(&source).await.unwrap();
        assert_eq!(balance.used_addresses, 1);
        assert_eq!(source.calls(), 26 + 20);
        assert_eq!(balance.scanned_addresses, 46);
    }

    #[tokio::test]
    async fn funds_inside_the_gap_are_found_and_funds_beyond_it_are_not() {
        // Índice 0 usado; 1..=20 vazios (o gap fecha); o 21 tem saldo mas fica de fora.
        // Este teste documenta o limite do gap limit, que a tela avisa.
        let beyond = FakeSource::new().with(Chain::Receive, 0, funded(1_000)).with(Chain::Receive, 21, funded(9_999));
        assert_eq!(scan(&beyond).await.unwrap().confirmed_sat, 1_000);
        // Com o saldo no 20 (o último dentro do gap) ele entra.
        let inside = FakeSource::new().with(Chain::Receive, 0, funded(1_000)).with(Chain::Receive, 20, funded(500));
        assert_eq!(scan(&inside).await.unwrap().confirmed_sat, 1_500);
    }

    #[tokio::test]
    async fn pending_amounts_are_summed_apart_from_confirmed() {
        let source = FakeSource::new()
            .with(Chain::Receive, 0, AddressStats { confirmed_sat: 1_000, pending_sat: -400, tx_count: 2 })
            .with(Chain::Change, 0, AddressStats { confirmed_sat: 0, pending_sat: 350, tx_count: 1 });
        let balance = scan(&source).await.unwrap();
        assert_eq!((balance.confirmed_sat, balance.pending_sat), (1_000, -50));
    }

    #[tokio::test]
    async fn a_chain_that_never_goes_quiet_stops_at_the_cap_and_is_marked_truncated() {
        let mut source = FakeSource::new();
        source.every_address = Some(funded(1));
        let balance = scan(&source).await.unwrap();
        assert!(balance.truncated);
        assert_eq!(source.calls(), (2 * MAX_ADDRESSES_PER_CHAIN) as usize);
        assert_eq!(balance.confirmed_sat, 2 * u64::from(MAX_ADDRESSES_PER_CHAIN));
    }

    #[tokio::test]
    async fn one_failed_lookup_aborts_the_whole_scan_with_no_partial_balance() {
        let mut source = FakeSource::new().with(Chain::Receive, 0, funded(1_000));
        source.failing = Some(address(Chain::Receive, 3));
        assert!(matches!(scan(&source).await, Err(AppError::ProviderUnavailable(_))));
    }

    #[tokio::test]
    async fn a_confirmed_sum_that_overflows_u64_is_an_error() {
        let source = FakeSource::new().with(Chain::Receive, 0, funded(u64::MAX)).with(Chain::Receive, 1, funded(1));
        assert!(scan(&source).await.is_err());
    }

    // ------------------------------------------------------------ ao vivo (rede)
    // `cargo test --lib -- --ignored wallet_viewer::esplora`. Vão aos dois Esplora públicos com a
    // conta pública do vetor do BIP84 (mnemônico de teste; nada aqui é do dono). Só afirmam o que
    // é estável: o saldo dessa conta muda. Não rodam no CI normal; servem para conferir de novo
    // quando um provedor mudar o formato ou o limite de taxa (P15 no PENDING.md).

    #[tokio::test]
    #[ignore]
    async fn live_each_provider_answers_for_a_known_used_address() {
        let used = address(Chain::Receive, 0);
        for (name, base) in ENDPOINTS {
            let body = get_text(name, &format!("{base}/address/{used}"), DEFAULT_TIMEOUT).await.unwrap();
            let stats = parse_address_response(&body).unwrap();
            assert!(stats.tx_count > 0, "{name}: the BIP84 receive-0 address has history");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_scan_of_the_bip84_vector_account() {
        let balance = scan_balance(&parse_xpub(ZPUB).unwrap(), &EsploraSource).await.unwrap();
        assert!(!balance.truncated);
        assert!(balance.scanned_addresses >= 2 * GAP_LIMIT, "{balance:?}");
        // Recebimento 0 e 1 têm histórico (180 e 8 transações em 2026-11-14).
        assert!(balance.used_addresses >= 2, "{balance:?}");
    }
}
