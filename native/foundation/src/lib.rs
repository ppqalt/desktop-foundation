pub mod actions;
pub mod apps;
pub mod cache;
pub mod clipboard;
pub mod process;
pub mod state;
pub mod volume;

use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Database(rusqlite::Error),
    Json(serde_json::Error),
    Invalid(String),
    Process { command: String, detail: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Database(e) => write!(f, "Clipboard database: {e}; history was not reset"),
            Self::Json(e) => write!(f, "Invalid state JSON: {e}"),
            Self::Invalid(message) => f.write_str(message),
            Self::Process { command, detail } => write!(f, "{command}: {detail}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Database(e) => Some(e),
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
pub fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}
