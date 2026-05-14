use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum BuiError {
    #[error("git command failed: {0}")]
    GitFailed(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, BuiError>;
