use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Claude projects directory not found at {0}")]
    ProjectsDirNotFound(String),

    #[error("No conversation history found")]
    NoHistoryFound,

    #[error("User cancelled selection")]
    SelectionCancelled,

    #[error("{0}")]
    Usage(String),

    #[error("Failed to run Claude CLI: {0}")]
    ClaudeExecutionError(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
