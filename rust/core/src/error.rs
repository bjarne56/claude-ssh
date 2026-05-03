use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("config: {0}")]
    Config(String),

    #[error("selector resolve failed: {0}")]
    Selector(String),

    #[error("securecrt parse: {0}")]
    SecureCrt(String),

    #[error("wezterm mux: {0}")]
    WezTerm(String),

    #[error("cast recorder: {0}")]
    Recorder(String),

    #[error("blocked: {0}")]
    Blocked(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;