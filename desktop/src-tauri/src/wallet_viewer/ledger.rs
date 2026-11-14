//! Ledger por USB (ADR 0002, Fase 36.2): **só lê** o endereço Ethereum de uma conta.
//!
//! O único pedido que este arquivo sabe montar é `GET_ADDRESS` do app Ethereum, sem pedir
//! confirmação na tela. Não há código para nenhuma outra instrução; o teste-guarda do módulo
//! (`tests.rs`) barra termos de assinatura aqui dentro.
//!
//! Duas camadas de enquadramento:
//! - **APDU** (a mensagem do app): `CLA INS P1 P2 Lc dados`; a resposta termina com 2 bytes de
//!   status (`0x9000` = sucesso).
//! - **Pacotes HID** de 64 bytes que carregam o APDU: canal `0x0101`, tag `0x05`, sequência de
//!   2 bytes; o 1º pacote leva também o tamanho total. Protocolo público da Ledger
//!   (`@ledgerhq/hw-transport-node-hid`). Referência lida: `truthid/desktop/src-tauri/src/ledger.rs`
//!   (não importado; aqui a leitura valida canal, tag e sequência).

use std::str::FromStr;
use std::sync::Mutex;

use alloy_primitives::Address;
use sea_orm::DatabaseConnection;

use crate::error::AppError;
use crate::wallet_viewer::store;

/// Vendor ID USB da Ledger SAS, o mesmo em todos os modelos.
pub const LEDGER_VENDOR_ID: u16 = 0x2c97;
/// Usage page da interface de APDU (as outras interfaces da Ledger são FIDO/U2F).
const APDU_USAGE_PAGE: u16 = 0xffa0;

const HID_PACKET_SIZE: usize = 64;
const CHANNEL: u16 = 0x0101;
const TAG_APDU: u8 = 0x05;
/// Cabeçalho do 1º pacote: canal (2) + tag (1) + sequência (2) + tamanho total (2).
const FIRST_HEADER: usize = 7;
/// Cabeçalho dos pacotes seguintes: sem o tamanho total.
const NEXT_HEADER: usize = 5;
const READ_TIMEOUT_MS: i32 = 5_000;

const ETH_CLA: u8 = 0xe0;
const INS_GET_ADDRESS: u8 = 0x02;
/// P1 = 0: não mostrar o endereço na tela da Ledger (ler endereço não exige confirmação).
const P1_NO_DISPLAY: u8 = 0x00;
/// P2 = 0: sem chain code na resposta.
const P2_NO_CHAIN_CODE: u8 = 0x00;
const HARDENED: u32 = 0x8000_0000;

/// Chave pública não comprimida (`0x04 || X || Y`) e endereço em ASCII hex (sem `0x`).
const PUBLIC_KEY_LEN: usize = 65;
const ADDRESS_ASCII_LEN: usize = 40;

/// Uma leitura por vez: duas leituras simultâneas intercalariam pacotes no mesmo dispositivo.
static DEVICE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// Nenhuma Ledger na lista de dispositivos USB.
    NotConnected,
    /// O sistema recusou abrir o dispositivo (outro app com ele aberto; no Linux, sem regra udev).
    AccessDenied,
    /// Ledger bloqueada (PIN).
    Locked,
    /// O app Ethereum não está aberto na Ledger.
    WrongApp,
    /// A Ledger não respondeu a tempo.
    Timeout,
    /// Resposta que não segue o protocolo; o texto diz o quê.
    Malformed(&'static str),
    /// Status word de erro que não tem classificação própria.
    Status(u16),
    /// Conta fora do intervalo de índice hardened (0 a 2^31 − 1).
    InvalidAccount,
    /// Falha da biblioteca HID ou do sistema.
    Hid(String),
}

impl LedgerError {
    pub fn user_message(&self) -> String {
        todo!()
    }
}

impl From<LedgerError> for AppError {
    fn from(error: LedgerError) -> Self {
        AppError::Device(error.user_message())
    }
}

/// Fronteira de I/O com o dispositivo: o `HidDevice` real e um dispositivo falso nos testes.
pub trait HidLink {
    /// Escreve um relatório HID (byte 0 = report ID, sempre 0 na Ledger).
    fn write(&self, packet: &[u8]) -> Result<usize, LedgerError>;
    /// Lê um pacote; `Ok(0)` significa que o tempo acabou.
    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, LedgerError>;
}

impl HidLink for hidapi::HidDevice {
    fn write(&self, packet: &[u8]) -> Result<usize, LedgerError> {
        hidapi::HidDevice::write(self, packet).map_err(|error| LedgerError::Hid(error.to_string()))
    }

    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, LedgerError> {
        hidapi::HidDevice::read_timeout(self, buf, timeout_ms).map_err(|error| LedgerError::Hid(error.to_string()))
    }
}

/// A interface HID que fala APDU (as outras são FIDO). Mesmo filtro do transporte oficial.
pub fn is_ledger_apdu_interface(vendor_id: u16, usage_page: u16, interface_number: i32) -> bool {
    let _ = (vendor_id, usage_page, interface_number, APDU_USAGE_PAGE);
    todo!()
}

/// Caminho `m/44'/60'/conta'/0/0` (o do Ledger Live) no formato do app Ethereum:
/// 1 byte de profundidade + 4 bytes big-endian por componente.
pub fn derivation_path(account: u32) -> Result<Vec<u8>, LedgerError> {
    let _ = (account, HARDENED);
    todo!()
}

/// O único APDU deste módulo: `GET_ADDRESS`, sem tela e sem chain code.
pub fn get_address_apdu(account: u32) -> Result<Vec<u8>, LedgerError> {
    let _ = (account, ETH_CLA, INS_GET_ADDRESS, P1_NO_DISPLAY, P2_NO_CHAIN_CODE);
    todo!()
}

/// Fatia um APDU em relatórios HID de 65 bytes (report ID 0 + pacote de 64).
pub fn frame_apdu(apdu: &[u8]) -> Vec<Vec<u8>> {
    let _ = (apdu, HID_PACKET_SIZE, CHANNEL, TAG_APDU, FIRST_HEADER, NEXT_HEADER);
    todo!()
}

/// Lê e remonta a resposta, conferindo canal, tag e sequência de cada pacote.
pub fn read_response(link: &dyn HidLink, timeout_ms: i32) -> Result<Vec<u8>, LedgerError> {
    let _ = (link, timeout_ms);
    todo!()
}

/// Separa o status word: `0x9000` devolve os dados; os outros viram erro classificado.
pub fn check_status(response: &[u8]) -> Result<Vec<u8>, LedgerError> {
    let _ = response;
    todo!()
}

/// Lê a resposta do `GET_ADDRESS` e confere que o endereço é o da chave pública devolvida.
/// Devolve o endereço com checksum EIP-55.
pub fn parse_address_response(data: &[u8]) -> Result<String, LedgerError> {
    let _ = (data, PUBLIC_KEY_LEN, ADDRESS_ASCII_LEN, Address::from_str("0x0"));
    todo!()
}

/// Troca completa com um dispositivo já aberto: monta, envia, lê e interpreta.
pub fn read_address(link: &dyn HidLink, account: u32) -> Result<String, LedgerError> {
    let _ = (link, account);
    todo!()
}

/// Abre a Ledger plugada e lê o endereço da conta. Bloqueante: chamar fora do runtime async.
pub fn read_address_from_device(account: u32) -> Result<String, LedgerError> {
    let _ = (account, &DEVICE_LOCK, READ_TIMEOUT_MS);
    todo!()
}

/// Porta de entrada do comando: com o módulo desligado recusa **antes** de tocar no USB;
/// ligado, roda a leitura (bloqueante) fora do runtime async. Não grava nada.
pub async fn read_address_if_enabled<F>(db: &DatabaseConnection, account: u32, read: F) -> Result<String, AppError>
where
    F: FnOnce(u32) -> Result<String, LedgerError> + Send + 'static,
{
    let _ = (db, account, read, store::require_enabled);
    todo!()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::*;
    use crate::test_support::memory_db;

    /// Chave pública da chave privada 1 (o gerador G do secp256k1) e o endereço dela.
    const G_X: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const G_Y: &str = "483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";
    const G_ADDRESS: &str = "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf";

    fn public_key() -> Vec<u8> {
        let mut key = vec![0x04];
        key.extend(hex::decode(G_X).unwrap());
        key.extend(hex::decode(G_Y).unwrap());
        key
    }

    /// Dados de uma resposta de `GET_ADDRESS` (sem status word).
    fn address_data(key: &[u8], ascii: &str) -> Vec<u8> {
        let mut data = vec![key.len() as u8];
        data.extend_from_slice(key);
        data.push(ascii.len() as u8);
        data.extend_from_slice(ascii.as_bytes());
        data
    }

    fn with_status(mut data: Vec<u8>, status: u16) -> Vec<u8> {
        data.extend_from_slice(&status.to_be_bytes());
        data
    }

    /// Pacotes como a Ledger os envia (64 bytes, sem report ID), montados à mão.
    fn device_packets(payload: &[u8]) -> Vec<Vec<u8>> {
        let mut packets = Vec::new();
        let mut offset = 0;
        let mut sequence: u16 = 0;
        while offset < payload.len() || sequence == 0 {
            let mut packet = vec![0x01, 0x01, 0x05];
            packet.extend_from_slice(&sequence.to_be_bytes());
            if sequence == 0 {
                packet.extend_from_slice(&(payload.len() as u16).to_be_bytes());
            }
            let take = (64 - packet.len()).min(payload.len() - offset);
            packet.extend_from_slice(&payload[offset..offset + take]);
            packet.resize(64, 0);
            packets.push(packet);
            offset += take;
            sequence += 1;
        }
        packets
    }

    #[derive(Default)]
    struct FakeLedger {
        written: RefCell<Vec<Vec<u8>>>,
        replies: RefCell<VecDeque<Vec<u8>>>,
    }

    impl FakeLedger {
        fn answering(packets: Vec<Vec<u8>>) -> Self {
            Self { written: RefCell::new(Vec::new()), replies: RefCell::new(packets.into()) }
        }
    }

    impl HidLink for FakeLedger {
        fn write(&self, packet: &[u8]) -> Result<usize, LedgerError> {
            self.written.borrow_mut().push(packet.to_vec());
            Ok(packet.len())
        }

        fn read_timeout(&self, buf: &mut [u8], _timeout_ms: i32) -> Result<usize, LedgerError> {
            match self.replies.borrow_mut().pop_front() {
                Some(packet) => {
                    buf[..packet.len()].copy_from_slice(&packet);
                    Ok(packet.len())
                }
                None => Ok(0), // nada mais a ler: o tempo acaba
            }
        }
    }

    // ------------------------------------------------------------ interface e caminho

    #[test]
    fn only_the_ledger_apdu_interface_is_picked() {
        assert!(is_ledger_apdu_interface(0x2c97, 0xffa0, 0));
        assert!(is_ledger_apdu_interface(0x2c97, 0, 0)); // Linux/hidraw às vezes não informa a usage page
        assert!(!is_ledger_apdu_interface(0x2c97, 0xf1d0, 1)); // FIDO/U2F
        assert!(!is_ledger_apdu_interface(0x1209, 0xffa0, 0)); // outro fabricante
    }

    #[test]
    fn account_zero_path_is_ledger_live_style() {
        let expected = hex::decode("058000002c8000003c800000000000000000000000").unwrap();
        assert_eq!(derivation_path(0).unwrap(), expected);
    }

    #[test]
    fn the_account_goes_hardened_in_the_third_component() {
        let path = derivation_path(3).unwrap();
        assert_eq!(&path[9..13], &[0x80, 0x00, 0x00, 0x03]);
    }

    #[test]
    fn an_account_that_cannot_be_hardened_is_refused() {
        assert_eq!(derivation_path(0x8000_0000), Err(LedgerError::InvalidAccount));
        assert_eq!(derivation_path(u32::MAX), Err(LedgerError::InvalidAccount));
        assert!(derivation_path(0x7fff_ffff).is_ok());
    }

    #[test]
    fn get_address_apdu_is_e0_02_without_display_and_without_chain_code() {
        let apdu = get_address_apdu(0).unwrap();
        assert_eq!(&apdu[..5], &[0xe0, 0x02, 0x00, 0x00, 21]);
        assert_eq!(&apdu[5..], derivation_path(0).unwrap().as_slice());
    }

    #[test]
    fn an_invalid_account_never_reaches_the_device() {
        let device = FakeLedger::default();
        assert_eq!(read_address(&device, 0x8000_0000), Err(LedgerError::InvalidAccount));
        assert!(device.written.borrow().is_empty());
    }

    // ------------------------------------------------------------ pacotes HID

    #[test]
    fn a_short_apdu_fits_in_one_report() {
        let apdu = get_address_apdu(0).unwrap();
        let reports = frame_apdu(&apdu);
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert_eq!(report.len(), 65);
        assert_eq!(&report[..8], &[0x00, 0x01, 0x01, 0x05, 0x00, 0x00, 0x00, 26]);
        assert_eq!(&report[8..8 + 26], apdu.as_slice());
        assert!(report[8 + 26..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn a_long_apdu_spills_into_a_second_report_without_the_length_field() {
        let apdu: Vec<u8> = (0..100u8).collect();
        let reports = frame_apdu(&apdu);
        assert_eq!(reports.len(), 2);
        assert_eq!(&reports[0][..8], &[0x00, 0x01, 0x01, 0x05, 0x00, 0x00, 0x00, 100]);
        assert_eq!(&reports[0][8..], &apdu[..57]);
        assert_eq!(&reports[1][..6], &[0x00, 0x01, 0x01, 0x05, 0x00, 0x01]);
        assert_eq!(&reports[1][6..6 + 43], &apdu[57..]);
        assert!(reports[1][6 + 43..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn a_response_in_two_packets_is_reassembled_in_order() {
        let payload: Vec<u8> = (0..109u8).collect();
        let device = FakeLedger::answering(device_packets(&payload));
        assert_eq!(read_response(&device, 1).unwrap(), payload);
    }

    #[test]
    fn a_packet_on_the_wrong_channel_is_malformed() {
        let mut packets = device_packets(&[0x90, 0x00]);
        packets[0][1] = 0x02;
        let device = FakeLedger::answering(packets);
        assert!(matches!(read_response(&device, 1), Err(LedgerError::Malformed(_))));
    }

    #[test]
    fn a_packet_with_the_wrong_tag_is_malformed() {
        let mut packets = device_packets(&[0x90, 0x00]);
        packets[0][2] = 0x02; // tag de ping, não de APDU
        let device = FakeLedger::answering(packets);
        assert!(matches!(read_response(&device, 1), Err(LedgerError::Malformed(_))));
    }

    #[test]
    fn an_out_of_order_packet_is_malformed() {
        let payload: Vec<u8> = (0..109u8).collect();
        let mut packets = device_packets(&payload);
        packets[1][4] = 0x02; // sequência 2 onde devia vir 1
        let device = FakeLedger::answering(packets);
        assert!(matches!(read_response(&device, 1), Err(LedgerError::Malformed(_))));
    }

    #[test]
    fn a_response_shorter_than_a_status_word_is_malformed() {
        let device = FakeLedger::answering(device_packets(&[0x90]));
        assert!(matches!(read_response(&device, 1), Err(LedgerError::Malformed(_))));
    }

    #[test]
    fn no_answer_is_a_timeout() {
        let device = FakeLedger::default();
        assert_eq!(read_response(&device, 1), Err(LedgerError::Timeout));
    }

    #[test]
    fn a_missing_second_packet_is_a_timeout_not_partial_data() {
        let payload: Vec<u8> = (0..109u8).collect();
        let mut packets = device_packets(&payload);
        packets.pop();
        let device = FakeLedger::answering(packets);
        assert_eq!(read_response(&device, 1), Err(LedgerError::Timeout));
    }

    // ------------------------------------------------------------ status word

    #[test]
    fn status_9000_returns_the_data_without_the_status() {
        assert_eq!(check_status(&[0xaa, 0xbb, 0x90, 0x00]).unwrap(), vec![0xaa, 0xbb]);
    }

    #[test]
    fn status_words_are_classified() {
        assert_eq!(check_status(&[0x6e, 0x00]), Err(LedgerError::WrongApp));
        assert_eq!(check_status(&[0x6d, 0x00]), Err(LedgerError::WrongApp));
        assert_eq!(check_status(&[0x6e, 0x01]), Err(LedgerError::WrongApp));
        assert_eq!(check_status(&[0x65, 0x11]), Err(LedgerError::WrongApp));
        assert_eq!(check_status(&[0x55, 0x15]), Err(LedgerError::Locked));
        assert_eq!(check_status(&[0x69, 0x82]), Err(LedgerError::Locked));
        assert_eq!(check_status(&[0x6a, 0x80]), Err(LedgerError::Status(0x6a80)));
    }

    // ------------------------------------------------------------ resposta do GET_ADDRESS

    #[test]
    fn a_consistent_answer_gives_the_checksummed_address() {
        let ascii = G_ADDRESS.trim_start_matches("0x");
        assert_eq!(parse_address_response(&address_data(&public_key(), ascii)).unwrap(), G_ADDRESS);
        // A Ledger pode mandar o endereço todo em minúsculas.
        let lower = ascii.to_lowercase();
        assert_eq!(parse_address_response(&address_data(&public_key(), &lower)).unwrap(), G_ADDRESS);
    }

    #[test]
    fn an_address_that_does_not_match_the_public_key_is_malformed() {
        let other = "5aaeb6053f3e94c9b9a09f33669435e7ef1beaed";
        assert!(matches!(
            parse_address_response(&address_data(&public_key(), other)),
            Err(LedgerError::Malformed(_))
        ));
    }

    #[test]
    fn truncated_or_odd_answers_are_malformed() {
        let ascii = G_ADDRESS.trim_start_matches("0x");
        let full = address_data(&public_key(), ascii);
        assert!(matches!(parse_address_response(&[]), Err(LedgerError::Malformed(_))));
        assert!(matches!(parse_address_response(&full[..full.len() - 1]), Err(LedgerError::Malformed(_))));
        // Chave comprimida (33 bytes) não é o que o GET_ADDRESS devolve.
        let compressed = address_data(&public_key()[..33], ascii);
        assert!(matches!(parse_address_response(&compressed), Err(LedgerError::Malformed(_))));
        // Endereço com caractere que não é hex.
        let not_hex = format!("z{}", &ascii[1..]);
        assert!(matches!(parse_address_response(&address_data(&public_key(), &not_hex)), Err(LedgerError::Malformed(_))));
        // Bytes sobrando depois do endereço (chain code que não pedimos).
        let mut trailing = full.clone();
        trailing.push(0);
        assert!(matches!(parse_address_response(&trailing), Err(LedgerError::Malformed(_))));
    }

    // ------------------------------------------------------------ troca completa

    #[test]
    fn reading_an_address_sends_get_address_and_returns_the_address() {
        let ascii = G_ADDRESS.trim_start_matches("0x");
        let answer = with_status(address_data(&public_key(), ascii), 0x9000);
        let device = FakeLedger::answering(device_packets(&answer));
        assert_eq!(read_address(&device, 0).unwrap(), G_ADDRESS);
        assert_eq!(*device.written.borrow(), frame_apdu(&get_address_apdu(0).unwrap()));
    }

    #[test]
    fn reading_with_the_dashboard_open_says_wrong_app() {
        let device = FakeLedger::answering(device_packets(&[0x6e, 0x00]));
        assert_eq!(read_address(&device, 0), Err(LedgerError::WrongApp));
    }

    // ------------------------------------------------------------ porta do comando

    #[tokio::test]
    async fn a_disabled_module_refuses_before_touching_the_device() {
        let db = memory_db().await;
        let touched = Arc::new(AtomicBool::new(false));
        let flag = touched.clone();
        let result = read_address_if_enabled(&db, 0, move |_| {
            flag.store(true, Ordering::SeqCst);
            Ok(G_ADDRESS.to_string())
        })
        .await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
        assert!(!touched.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn an_enabled_module_reads_and_maps_device_errors() {
        let db = memory_db().await;
        store::set_enabled(&db, true).await.unwrap();
        let ok = read_address_if_enabled(&db, 2, |account| Ok(format!("account {account}"))).await;
        assert_eq!(ok.unwrap(), "account 2");
        let missing = read_address_if_enabled(&db, 0, |_| Err(LedgerError::NotConnected)).await;
        match missing {
            Err(AppError::Device(message)) => assert!(message.contains("No Ledger found"), "{message}"),
            other => panic!("expected a device error, got {other:?}"),
        }
    }

    #[test]
    fn every_error_has_a_message_for_the_screen() {
        let errors = [
            LedgerError::NotConnected,
            LedgerError::AccessDenied,
            LedgerError::Locked,
            LedgerError::WrongApp,
            LedgerError::Timeout,
            LedgerError::Malformed("x"),
            LedgerError::Status(0x6a80),
            LedgerError::InvalidAccount,
            LedgerError::Hid("x".to_string()),
        ];
        for error in errors {
            assert!(!error.user_message().is_empty(), "{error:?}");
        }
        assert!(LedgerError::AccessDenied.user_message().contains("udev"));
        assert!(LedgerError::Status(0x6a80).user_message().contains("0x6a80"));
    }
}
