//! Trezor por USB (ADR 0002 e adendo, Fase 36.3): **só lê** o endereço Ethereum de uma conta.
//!
//! Usa o `trezor-client` (só a feature `ethereum`). A crate também sabe assinar; este arquivo
//! só chama `init_device` e `ethereum_get_address`. Duas provas disso:
//! - o teste-guarda do módulo (`tests.rs`) barra termos de assinatura aqui dentro;
//! - os testes abaixo usam um transporte falso que registra **tudo** o que sai para o aparelho e
//!   falham se sair qualquer mensagem fora de Initialize, EthereumGetAddress, ButtonAck e
//!   PassphraseAck.
//!
//! PIN e passphrase são digitados no aparelho (Model T, Safe 3, Safe 5): a crate responde ao
//! pedido de passphrase com "digite no aparelho", e o Anchor nunca vê nenhum dos dois.
//!
//! O transporte USB da crate lê sem limite de tempo (timeout 0 do libusb). Por isso a leitura
//! tem um teto do lado async e uma trava que **recusa** (em vez de esperar) quando uma leitura
//! anterior ainda está presa no USB.

use std::sync::{Mutex, TryLockError};
use std::time::Duration;

use sea_orm::DatabaseConnection;
use trezor_client::client::Trezor;
use trezor_client::protos::failure::FailureType;
use trezor_client::transport::webusb::WebUsbTransport;
use trezor_client::{Error as ClientError, Model};

use crate::domain::wallet_address::parse_address;
use crate::error::AppError;
use crate::wallet_viewer::store;

/// Teto de uma leitura: o PIN e a passphrase são digitados no aparelho, com calma.
pub const READ_LIMIT: Duration = Duration::from_secs(120);
const HARDENED: u32 = 0x8000_0000;

/// Uma leitura por vez. Quem chega com a trava ocupada recebe `Busy` na hora.
static DEVICE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrezorReadError {
    NotConnected,
    MultipleDevices,
    /// O sistema recusou abrir o aparelho (Trezor Suite com ele aberto; no Linux, sem regra udev).
    AccessDenied,
    /// PIN errado ou cancelado, ou matriz de PIN no computador (Trezor One), que não tratamos.
    Locked,
    /// Aparelho sem carteira configurada.
    NotInitialized,
    /// O usuário cancelou no aparelho.
    Cancelled,
    /// Uma leitura anterior ainda segura o aparelho.
    Busy,
    /// Passou do teto de tempo.
    Timeout,
    /// Conta fora do intervalo de índice hardened (0 a 2^31 − 1).
    InvalidAccount,
    /// O aparelho devolveu um endereço inválido.
    Malformed,
    /// Qualquer outra falha, com o texto do aparelho ou da biblioteca.
    Other(String),
}

impl TrezorReadError {
    pub fn user_message(&self) -> String {
        match self {
            TrezorReadError::NotConnected => "No Trezor found. Plug it in and unlock it.".to_string(),
            TrezorReadError::MultipleDevices => "More than one Trezor is plugged in. Leave only one.".to_string(),
            TrezorReadError::AccessDenied => "Access to the Trezor was denied. Close Trezor Suite; on Linux, \
                 install Trezor's udev rules."
                .to_string(),
            TrezorReadError::Locked => "The Trezor is locked. Enter the PIN on the device (a Trezor One must be \
                 unlocked in Trezor Suite first)."
                .to_string(),
            TrezorReadError::NotInitialized => {
                "This Trezor has no wallet yet. Set it up in Trezor Suite first.".to_string()
            }
            TrezorReadError::Cancelled => "The request was cancelled on the Trezor.".to_string(),
            TrezorReadError::Busy => "Another Trezor read is still waiting for the device. Confirm on the \
                 Trezor, or unplug it and try again."
                .to_string(),
            TrezorReadError::Timeout => "The Trezor did not answer in time. Confirm on the device, or unplug \
                 it and try again."
                .to_string(),
            TrezorReadError::InvalidAccount => {
                "The account number must be between 0 and 2147483647.".to_string()
            }
            TrezorReadError::Malformed => "The Trezor returned an address that is not valid.".to_string(),
            TrezorReadError::Other(message) => format!("Trezor error: {message}"),
        }
    }
}

impl From<TrezorReadError> for AppError {
    fn from(error: TrezorReadError) -> Self {
        AppError::Device(error.user_message())
    }
}

/// Caminho `m/44'/60'/conta'/0/0` (o do Trezor Suite para Ethereum), no formato `address_n`.
pub fn derivation_path(account: u32) -> Result<Vec<u32>, TrezorReadError> {
    if account >= HARDENED {
        return Err(TrezorReadError::InvalidAccount);
    }
    Ok(vec![44 | HARDENED, 60 | HARDENED, account | HARDENED, 0, 0])
}

/// Escolhe o aparelho entre os encontrados no USB: ignora os que estão em modo bootloader;
/// precisa sobrar exatamente um. Devolve o índice na lista.
pub fn pick_device(models: &[Model]) -> Result<usize, TrezorReadError> {
    let mut usable = models.iter().enumerate().filter(|(_, model)| **model != Model::TrezorBootloader);
    let (index, _) = usable.next().ok_or(TrezorReadError::NotConnected)?;
    if usable.next().is_some() {
        return Err(TrezorReadError::MultipleDevices);
    }
    Ok(index)
}

/// Traduz o erro do `trezor-client` para o que a tela precisa dizer.
pub fn classify(error: &ClientError) -> TrezorReadError {
    match error {
        ClientError::NoDeviceFound => TrezorReadError::NotConnected,
        ClientError::DeviceNotUnique => TrezorReadError::MultipleDevices,
        // A crate devolve `UnsupportedNetwork` quando o aparelho pede a matriz de PIN no
        // computador (Trezor One), que ela não trata (`client/common.rs`, `handle_interaction`).
        ClientError::UnsupportedNetwork | ClientError::UnexpectedInteractionRequest(_) => TrezorReadError::Locked,
        ClientError::FailureResponse(failure) => match failure.code() {
            FailureType::Failure_ActionCancelled => TrezorReadError::Cancelled,
            FailureType::Failure_PinInvalid | FailureType::Failure_PinExpected | FailureType::Failure_PinCancelled => {
                TrezorReadError::Locked
            }
            FailureType::Failure_NotInitialized => TrezorReadError::NotInitialized,
            other => TrezorReadError::Other(format!("{other:?}: {}", failure.message())),
        },
        ClientError::TransportConnect(inner) => {
            let text = inner.to_string().to_lowercase();
            if text.contains("access") || text.contains("permission") || text.contains("denied") || text.contains("busy")
            {
                TrezorReadError::AccessDenied
            } else {
                TrezorReadError::Other(inner.to_string())
            }
        }
        other => TrezorReadError::Other(other.to_string()),
    }
}

/// Troca completa com um aparelho já conectado: inicia a sessão e pede o endereço.
pub fn read_address(trezor: &mut Trezor, account: u32) -> Result<String, TrezorReadError> {
    let path = derivation_path(account)?;
    trezor.init_device(None).map_err(|error| classify(&error))?;
    let address = trezor.ethereum_get_address(path).map_err(|error| classify(&error))?;
    parse_address(&address).map_err(|_| TrezorReadError::Malformed)
}

/// Roda `read` com a trava; se ela estiver ocupada, recusa na hora com `Busy`.
pub fn exclusive<T>(
    lock: &Mutex<()>,
    read: impl FnOnce() -> Result<T, TrezorReadError>,
) -> Result<T, TrezorReadError> {
    let _guard = match lock.try_lock() {
        Ok(guard) => guard,
        // Uma leitura anterior entrou em pânico: o `()` guardado não tem estado a corromper.
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return Err(TrezorReadError::Busy),
    };
    read()
}

/// Acha o Trezor no USB (sem procurar o emulador por UDP), conecta e lê o endereço.
/// Bloqueante: chamar fora do runtime async.
pub fn read_address_from_device(account: u32) -> Result<String, TrezorReadError> {
    // Conta inválida não precisa nem abrir o USB.
    derivation_path(account)?;
    exclusive(&DEVICE_LOCK, || {
        let devices =
            WebUsbTransport::find_devices(false).map_err(|error| classify(&ClientError::TransportConnect(error)))?;
        let models: Vec<Model> = devices.iter().map(|device| device.model).collect();
        let index = pick_device(&models)?;
        let device = devices.into_iter().nth(index).ok_or(TrezorReadError::NotConnected)?;
        let mut trezor = device.connect().map_err(|error| classify(&error))?;
        read_address(&mut trezor, account)
    })
}

/// Porta de entrada do comando: com o módulo desligado recusa **antes** de tocar no USB;
/// ligado, roda a leitura fora do runtime async, com teto de tempo. Não grava nada.
pub async fn read_address_if_enabled<F>(
    db: &DatabaseConnection,
    account: u32,
    limit: Duration,
    read: F,
) -> Result<String, AppError>
where
    F: FnOnce(u32) -> Result<String, TrezorReadError> + Send + 'static,
{
    store::require_enabled(db).await?;
    let task = tokio::task::spawn_blocking(move || read(account));
    match tokio::time::timeout(limit, task).await {
        // A thread pode continuar presa no USB; a trava faz a próxima leitura recusar com `Busy`.
        Err(_) => Err(TrezorReadError::Timeout.into()),
        Ok(Err(error)) => Err(AppError::Device(format!("The Trezor read was interrupted: {error}"))),
        Ok(Ok(result)) => result.map_err(AppError::from),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use protobuf::Message;
    use trezor_client::client::trezor_with_transport;
    use trezor_client::protos::{self, MessageType};
    use trezor_client::transport::error::Error as TransportError;
    use trezor_client::transport::{ProtoMessage, Transport};

    use super::*;
    use crate::test_support::memory_db;

    // Vetor do texto do EIP-55.
    const ADDRESS: &str = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";

    /// O que o Anchor pode mandar ao aparelho. Qualquer outra coisa reprova o teste.
    const ALLOWED: &[MessageType] = &[
        MessageType::MessageType_Initialize,
        MessageType::MessageType_EthereumGetAddress,
        MessageType::MessageType_ButtonAck,
        MessageType::MessageType_PassphraseAck,
    ];

    type Sent = Arc<Mutex<Vec<(MessageType, Vec<u8>)>>>;

    /// Aparelho falso: responde em ordem e registra o que recebeu.
    struct FakeTrezor {
        replies: VecDeque<ProtoMessage>,
        sent: Sent,
    }

    impl Transport for FakeTrezor {
        fn session_begin(&mut self) -> Result<(), TransportError> {
            Ok(())
        }

        fn session_end(&mut self) -> Result<(), TransportError> {
            Ok(())
        }

        fn write_message(&mut self, message: ProtoMessage) -> Result<(), TransportError> {
            self.sent.lock().unwrap().push((message.message_type(), message.into_payload()));
            Ok(())
        }

        fn read_message(&mut self) -> Result<ProtoMessage, TransportError> {
            self.replies.pop_front().ok_or(TransportError::DeviceReadTimeout)
        }
    }

    fn fake(replies: Vec<ProtoMessage>) -> (Trezor, Sent) {
        let sent: Sent = Arc::default();
        let transport = FakeTrezor { replies: replies.into(), sent: sent.clone() };
        (trezor_with_transport(Model::Trezor, Box::new(transport)), sent)
    }

    fn reply(message_type: MessageType, message: &impl Message) -> ProtoMessage {
        ProtoMessage::new(message_type, message.write_to_bytes().unwrap())
    }

    fn features() -> ProtoMessage {
        let mut features = protos::Features::new();
        features.set_major_version(2);
        features.set_minor_version(8);
        features.set_patch_version(0);
        reply(MessageType::MessageType_Features, &features)
    }

    fn address(text: &str) -> ProtoMessage {
        let mut message = protos::EthereumAddress::new();
        message.set_address(text.to_string());
        reply(MessageType::MessageType_EthereumAddress, &message)
    }

    fn failure(code: FailureType, text: &str) -> ProtoMessage {
        let mut message = protos::Failure::new();
        message.set_code(code);
        message.set_message(text.to_string());
        reply(MessageType::MessageType_Failure, &message)
    }

    fn sent_types(sent: &Sent) -> Vec<MessageType> {
        sent.lock().unwrap().iter().map(|(message_type, _)| *message_type).collect()
    }

    fn assert_only_reads(sent: &Sent) {
        for message_type in sent_types(sent) {
            assert!(ALLOWED.contains(&message_type), "the Anchor sent {message_type:?} to the Trezor");
        }
    }

    fn sent_payload(sent: &Sent, wanted: MessageType) -> Vec<u8> {
        let sent = sent.lock().unwrap();
        let (_, payload) = sent.iter().find(|(message_type, _)| *message_type == wanted).expect("message not sent");
        payload.clone()
    }

    // ------------------------------------------------------------ caminho

    #[test]
    fn account_zero_path_is_trezor_suite_style() {
        assert_eq!(derivation_path(0).unwrap(), vec![0x8000_002c, 0x8000_003c, 0x8000_0000, 0, 0]);
        assert_eq!(derivation_path(3).unwrap()[2], 0x8000_0003);
    }

    #[test]
    fn an_account_that_cannot_be_hardened_is_refused_without_talking_to_the_device() {
        assert_eq!(derivation_path(0x8000_0000), Err(TrezorReadError::InvalidAccount));
        assert!(derivation_path(0x7fff_ffff).is_ok());
        let (mut trezor, sent) = fake(vec![features(), address(ADDRESS)]);
        assert_eq!(read_address(&mut trezor, u32::MAX), Err(TrezorReadError::InvalidAccount));
        assert!(sent.lock().unwrap().is_empty());
        // Também pela porta do aparelho real: recusa antes de abrir o USB.
        assert_eq!(read_address_from_device(0x8000_0000), Err(TrezorReadError::InvalidAccount));
    }

    // ------------------------------------------------------------ troca com o aparelho falso

    #[test]
    fn a_plain_read_sends_only_initialize_and_get_address() {
        let (mut trezor, sent) = fake(vec![features(), address(&ADDRESS.to_lowercase())]);
        assert_eq!(read_address(&mut trezor, 0).unwrap(), ADDRESS);
        assert_eq!(
            sent_types(&sent),
            vec![MessageType::MessageType_Initialize, MessageType::MessageType_EthereumGetAddress]
        );
        let request =
            protos::EthereumGetAddress::parse_from_bytes(&sent_payload(&sent, MessageType::MessageType_EthereumGetAddress))
                .unwrap();
        assert_eq!(request.address_n, derivation_path(0).unwrap());
        assert_eq!(request.show_display, None);
    }

    #[test]
    fn a_passphrase_request_is_answered_on_the_device_without_a_passphrase() {
        let passphrase = reply(MessageType::MessageType_PassphraseRequest, &protos::PassphraseRequest::new());
        let (mut trezor, sent) = fake(vec![features(), passphrase, address(ADDRESS)]);
        assert_eq!(read_address(&mut trezor, 0).unwrap(), ADDRESS);
        assert_only_reads(&sent);
        let ack =
            protos::PassphraseAck::parse_from_bytes(&sent_payload(&sent, MessageType::MessageType_PassphraseAck)).unwrap();
        assert_eq!(ack.on_device, Some(true));
        assert_eq!(ack.passphrase, None);
    }

    #[test]
    fn a_button_request_is_acknowledged() {
        let button = reply(MessageType::MessageType_ButtonRequest, &protos::ButtonRequest::new());
        let (mut trezor, sent) = fake(vec![features(), button, address(ADDRESS)]);
        assert_eq!(read_address(&mut trezor, 0).unwrap(), ADDRESS);
        assert!(sent_types(&sent).contains(&MessageType::MessageType_ButtonAck));
        assert_only_reads(&sent);
    }

    #[test]
    fn a_pin_matrix_request_says_locked() {
        let pin = reply(MessageType::MessageType_PinMatrixRequest, &protos::PinMatrixRequest::new());
        let (mut trezor, sent) = fake(vec![features(), pin]);
        assert_eq!(read_address(&mut trezor, 0), Err(TrezorReadError::Locked));
        assert_only_reads(&sent);
    }

    #[test]
    fn device_failures_are_classified() {
        let cases = [
            (FailureType::Failure_ActionCancelled, TrezorReadError::Cancelled),
            (FailureType::Failure_PinInvalid, TrezorReadError::Locked),
            (FailureType::Failure_PinCancelled, TrezorReadError::Locked),
            (FailureType::Failure_NotInitialized, TrezorReadError::NotInitialized),
        ];
        for (code, expected) in cases {
            let (mut trezor, sent) = fake(vec![features(), failure(code, "x")]);
            assert_eq!(read_address(&mut trezor, 0), Err(expected), "{code:?}");
            assert_only_reads(&sent);
        }
        let (mut trezor, _) = fake(vec![features(), failure(FailureType::Failure_DataError, "bad path")]);
        match read_address(&mut trezor, 0) {
            Err(TrezorReadError::Other(text)) => assert!(text.contains("bad path"), "{text}"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn an_invalid_address_from_the_device_is_malformed() {
        for bad in ["0x12", "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAeD"] {
            let (mut trezor, _) = fake(vec![features(), address(bad)]);
            assert_eq!(read_address(&mut trezor, 0), Err(TrezorReadError::Malformed), "{bad}");
        }
    }

    #[test]
    fn library_errors_are_classified() {
        assert_eq!(classify(&ClientError::NoDeviceFound), TrezorReadError::NotConnected);
        assert_eq!(classify(&ClientError::DeviceNotUnique), TrezorReadError::MultipleDevices);
        assert_eq!(classify(&ClientError::UnsupportedNetwork), TrezorReadError::Locked);
        let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Permission denied");
        assert_eq!(
            classify(&ClientError::TransportConnect(TransportError::IO(denied))),
            TrezorReadError::AccessDenied
        );
    }

    // ------------------------------------------------------------ escolha do aparelho

    #[test]
    fn exactly_one_non_bootloader_device_is_picked() {
        assert_eq!(pick_device(&[]), Err(TrezorReadError::NotConnected));
        assert_eq!(pick_device(&[Model::TrezorBootloader]), Err(TrezorReadError::NotConnected));
        assert_eq!(pick_device(&[Model::Trezor, Model::TrezorLegacy]), Err(TrezorReadError::MultipleDevices));
        assert_eq!(pick_device(&[Model::TrezorBootloader, Model::Trezor]), Ok(1));
        assert_eq!(pick_device(&[Model::Trezor]), Ok(0));
    }

    // ------------------------------------------------------------ trava, teto e porta do comando

    #[test]
    fn a_busy_lock_refuses_at_once_and_a_free_one_runs() {
        let lock = Mutex::new(());
        {
            let _held = lock.lock().unwrap();
            assert_eq!(exclusive(&lock, || Ok(1)), Err(TrezorReadError::Busy));
        }
        assert_eq!(exclusive(&lock, || Ok(1)), Ok(1));
    }

    #[tokio::test]
    async fn a_read_past_the_limit_is_a_timeout() {
        let db = memory_db().await;
        store::set_enabled(&db, true).await.unwrap();
        let result = read_address_if_enabled(&db, 0, Duration::from_millis(50), |_| {
            std::thread::sleep(Duration::from_millis(500));
            Ok(ADDRESS.to_string())
        })
        .await;
        match result {
            Err(AppError::Device(message)) => assert!(message.contains("did not answer"), "{message}"),
            other => panic!("expected a timeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_disabled_module_refuses_before_touching_the_device() {
        let db = memory_db().await;
        let touched = Arc::new(AtomicBool::new(false));
        let flag = touched.clone();
        let result = read_address_if_enabled(&db, 0, READ_LIMIT, move |_| {
            flag.store(true, Ordering::SeqCst);
            Ok(ADDRESS.to_string())
        })
        .await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
        assert!(!touched.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn an_enabled_module_reads_and_maps_device_errors() {
        let db = memory_db().await;
        store::set_enabled(&db, true).await.unwrap();
        let ok = read_address_if_enabled(&db, 2, READ_LIMIT, |account| Ok(format!("account {account}"))).await;
        assert_eq!(ok.unwrap(), "account 2");
        let missing = read_address_if_enabled(&db, 0, READ_LIMIT, |_| Err(TrezorReadError::NotConnected)).await;
        match missing {
            Err(AppError::Device(message)) => assert!(message.contains("No Trezor found"), "{message}"),
            other => panic!("expected a device error, got {other:?}"),
        }
    }

    #[test]
    fn every_error_has_a_message_for_the_screen() {
        let errors = [
            TrezorReadError::NotConnected,
            TrezorReadError::MultipleDevices,
            TrezorReadError::AccessDenied,
            TrezorReadError::Locked,
            TrezorReadError::NotInitialized,
            TrezorReadError::Cancelled,
            TrezorReadError::Busy,
            TrezorReadError::Timeout,
            TrezorReadError::InvalidAccount,
            TrezorReadError::Malformed,
            TrezorReadError::Other("x".to_string()),
        ];
        for error in errors {
            assert!(!error.user_message().is_empty(), "{error:?}");
        }
        assert!(TrezorReadError::AccessDenied.user_message().contains("udev"));
        assert!(TrezorReadError::Locked.user_message().contains("PIN"));
    }
}
