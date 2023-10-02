use std::str::Utf8Error;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ArklibError>;

#[derive(Error, Debug)]
pub enum ArklibError {
    #[error("IO error")]
    Io(#[from] std::io::Error),
    #[error("Path error: {0}")]
    Path(String),
    #[error("There is some collision: {0}")]
    Collision(String),
    #[error("Parsing error")]
    Parse,
    #[error("Networking error")]
    Network,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<reqwest::Error> for ArklibError {
    fn from(_: reqwest::Error) -> Self {
        Self::Network
    }
}

impl From<Utf8Error> for ArklibError {
    fn from(_: Utf8Error) -> Self {
        Self::Parse
    }
}
