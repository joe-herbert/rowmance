use serde::Serialize;
use thiserror::Error;

/// Structured error type returned to the frontend via Tauri IPC.
/// Every error has a stable `code` string so the frontend can branch
/// without parsing the human-readable `message`.
#[derive(Debug, Serialize, Clone)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Internal domain errors that are converted to `AppError` at command boundaries.
#[derive(Debug, Error)]
pub enum RowmanceError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),

    #[error("Connection not active: {0}")]
    ConnectionNotActive(String),

    #[error("Read-only mode: mutating statements are not allowed")]
    ReadOnlyViolation,

    #[error("Keychain error: {0}")]
    Keychain(String),

    #[error("SSH error: {0}")]
    Ssh(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialisation error: {0}")]
    Serialisation(#[from] serde_json::Error),
}

impl From<RowmanceError> for AppError {
    fn from(err: RowmanceError) -> Self {
        match err {
            RowmanceError::Database(ref e) => {
                AppError::new("DB_ERROR", err.to_string()).with_detail(format!("{e:#}"))
            }
            RowmanceError::ConnectionNotFound(ref id) => AppError::new(
                "CONNECTION_NOT_FOUND",
                format!("No connection with id {id}"),
            ),
            RowmanceError::ConnectionNotActive(ref id) => AppError::new(
                "CONNECTION_NOT_ACTIVE",
                format!("Connection {id} is not connected"),
            ),
            RowmanceError::ReadOnlyViolation => AppError::new(
                "READ_ONLY_VIOLATION",
                "This connection is in read-only mode",
            ),
            RowmanceError::Keychain(ref msg) => AppError::new("KEYCHAIN_ERROR", msg.clone()),
            RowmanceError::Ssh(ref msg) => AppError::new("SSH_ERROR", msg.clone()),
            RowmanceError::Io(ref e) => AppError::new("IO_ERROR", e.to_string()),
            RowmanceError::Serialisation(ref e) => {
                AppError::new("SERIALISATION_ERROR", e.to_string())
            }
        }
    }
}
