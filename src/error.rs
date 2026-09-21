use std::path::PathBuf;

/// Typed errors for the ProfileMux core. Application/UI layers may wrap these in
/// `anyhow::Error`; the core itself never does.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("operation `{operation}` is not supported by {browser}")]
    Unsupported { operation: String, browser: String },

    #[error("path not found: {0}")]
    NotFound(PathBuf),

    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("malformed metadata in {path}: {message}")]
    Malformed { path: PathBuf, message: String },

    #[error("profile selector `{selector}` is ambiguous: {candidates:?}")]
    AmbiguousSelector {
        selector: String,
        candidates: Vec<String>,
    },

    #[error("no profile matches selector `{0}`")]
    NoSuchProfile(String),

    #[error("no browser matches `{0}`")]
    NoSuchBrowser(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    pub fn unsupported(operation: &str, browser: &str) -> Self {
        Error::Unsupported {
            operation: operation.to_string(),
            browser: browser.to_string(),
        }
    }
}
