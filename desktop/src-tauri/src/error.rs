use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    InvalidGuard(String),
    #[error("green and red boundaries must differ to define a direction")]
    EqualThresholds,
    #[error("no threshold configured for indicator '{0}'")]
    UnknownIndicator(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("data collector is already running")]
    CollectorBusy,
    #[error("data collector failed: {0}")]
    CollectorFailed(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("keyring error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("could not find a TruthID Desktop instance running on this machine")]
    TruthIdNotFound,
    #[error("TruthID error: {0}")]
    TruthId(String),
    #[error("invalid address: {0}")]
    InvalidAddress(String),
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Finance API: not found — {0}")]
    FinanceApiNotFound(String),
    #[error("Finance API error: {0}")]
    FinanceApi(String),
    #[error("Arweave gateway error: {0}")]
    Arweave(String),
    #[error("decryption failed: {0}")]
    Decryption(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AppError {
    fn code(&self) -> &'static str {
        match self {
            AppError::InvalidGuard(_) => "INVALID_GUARD",
            AppError::EqualThresholds => "EQUAL_THRESHOLDS",
            AppError::UnknownIndicator(_) => "UNKNOWN_INDICATOR",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::CollectorBusy => "COLLECTOR_BUSY",
            AppError::CollectorFailed(_) => "COLLECTOR_FAILED",
            AppError::InvalidInput(_) => "INVALID_INPUT",
            AppError::Database(_) => "DATABASE_ERROR",
            AppError::Keyring(_) => "KEYRING_ERROR",
            AppError::Http(_) => "HTTP_ERROR",
            AppError::TruthIdNotFound => "TRUTHID_NOT_FOUND",
            AppError::TruthId(_) => "TRUTHID_ERROR",
            AppError::InvalidAddress(_) => "INVALID_ADDRESS",
            AppError::Rpc(_) => "RPC_ERROR",
            AppError::Io(_) => "IO_ERROR",
            AppError::FinanceApiNotFound(_) => "FINANCE_API_NOT_FOUND",
            AppError::FinanceApi(_) => "FINANCE_API_ERROR",
            AppError::Arweave(_) => "ARWEAVE_ERROR",
            AppError::Decryption(_) => "DECRYPTION_ERROR",
            AppError::Json(_) => "JSON_ERROR",
        }
    }
}

// Tauri serializes command errors to the frontend as JSON, so this shapes
// them as `{ code, message }` instead of a bare string the UI can't switch on.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}
