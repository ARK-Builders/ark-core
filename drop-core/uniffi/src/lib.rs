mod receiver;
mod sender;

pub use receiver::*;
pub use sender::*;

#[derive(Debug, thiserror::Error)]
pub enum DropError {
    #[error("TODO: \"{0}\".")]
    TODO(String),
}

uniffi::include_scaffolding!("drop");
