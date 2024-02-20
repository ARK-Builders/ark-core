use std::{str::Utf8Error, time::SystemTimeError};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ArklibError>;

#[derive(Error, Debug)]
pub enum ArklibError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Path error: {0}")]
    Path(String),
    #[error("There is some collision: {0}")]
    Collision(String),
    #[error("Parsing error")]
    Parse,
    #[error("Storage error: {0} {1}")]
    Storage(String, String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<Utf8Error> for ArklibError {
    fn from(_: Utf8Error) -> Self {
        Self::Parse
    }
}

impl From<serde_json::Error> for ArklibError {
    fn from(_: serde_json::Error) -> Self {
        Self::Parse
    }
}

impl From<SystemTimeError> for ArklibError {
    fn from(value: SystemTimeError) -> Self {
        Self::Other(anyhow::anyhow!(value.to_string()))
    }
}

impl From<Box<dyn std::error::Error>> for ArklibError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        Self::Other(anyhow::anyhow!(e.to_string()))
    }
}
