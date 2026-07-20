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

    #[allow(dead_code)]
    #[error("Read-only mode: mutating statements are not allowed")]
    ReadOnlyViolation,

    #[error("Pool error: {0}")]
    Pool(String),

    #[allow(dead_code)]
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
            RowmanceError::ConnectionNotActive(ref name) => AppError::new(
                "CONNECTION_NOT_ACTIVE",
                format!("Connection {name} is not connected"),
            ),
            RowmanceError::ReadOnlyViolation => AppError::new(
                "READ_ONLY_VIOLATION",
                "This connection is in read-only mode",
            ),
            RowmanceError::Pool(ref msg) => AppError::new("POOL_ERROR", msg.clone()),
            RowmanceError::Keychain(ref msg) => AppError::new("KEYCHAIN_ERROR", msg.clone()),
            RowmanceError::Ssh(ref msg) => AppError::new("SSH_ERROR", msg.clone()),
            RowmanceError::Io(ref e) => AppError::new("IO_ERROR", e.to_string()),
            RowmanceError::Serialisation(ref e) => {
                AppError::new("SERIALISATION_ERROR", e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_new_sets_fields() {
        let err = AppError::new("TEST_CODE", "something went wrong");
        assert_eq!(err.code, "TEST_CODE");
        assert_eq!(err.message, "something went wrong");
        assert!(err.detail.is_none());
    }

    #[test]
    fn app_error_with_detail_sets_detail() {
        let err = AppError::new("E", "msg").with_detail("context here");
        assert_eq!(err.detail.as_deref(), Some("context here"));
    }

    #[test]
    fn connection_not_active_maps_to_correct_code() {
        let err: AppError = RowmanceError::ConnectionNotActive("abc".into()).into();
        assert_eq!(err.code, "CONNECTION_NOT_ACTIVE");
        assert!(err.message.contains("abc"));
    }

    #[test]
    fn connection_not_found_maps_to_correct_code() {
        let err: AppError = RowmanceError::ConnectionNotFound("xyz".into()).into();
        assert_eq!(err.code, "CONNECTION_NOT_FOUND");
        assert!(err.message.contains("xyz"));
    }

    #[test]
    fn read_only_violation_maps_to_correct_code() {
        let err: AppError = RowmanceError::ReadOnlyViolation.into();
        assert_eq!(err.code, "READ_ONLY_VIOLATION");
    }

    #[test]
    fn keychain_error_maps_to_correct_code() {
        let err: AppError = RowmanceError::Keychain("no keychain".into()).into();
        assert_eq!(err.code, "KEYCHAIN_ERROR");
    }

    #[test]
    fn ssh_error_maps_to_correct_code() {
        let err: AppError = RowmanceError::Ssh("handshake failed".into()).into();
        assert_eq!(err.code, "SSH_ERROR");
    }

    #[test]
    fn io_error_maps_to_correct_code() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: AppError = RowmanceError::Io(io_err).into();
        assert_eq!(err.code, "IO_ERROR");
    }
}
