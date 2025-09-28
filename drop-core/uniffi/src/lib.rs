//! Crate entry point for the Drop-style file transfer API exposed via UniFFI.
//!
//! This crate re-exports sender and receiver modules and provides the
//! DropError type surfaced to foreign languages. Bindings are generated via
//! `uniffi::include_scaffolding!`.

mod receiver;
mod sender;

pub use receiver::*;
pub use sender::*;

/// High-level error type surfaced over FFI and to consumers.
///
/// Notes:
/// - The TODO variant is a temporary placeholder mapping lower-level errors
///   into a string message for diagnostics.
/// - Prefer introducing well-typed variants as the implementation evolves.
#[derive(Debug, thiserror::Error)]
pub enum DropError {
    /// Placeholder error with a human-readable message.
    #[error("TODO: \"{0}\".")]
    TODO(String),
}

// UniFFI picks up the UDL and generates the FFI scaffolding.
// Keep this at crate root to expose all exported items.
uniffi::include_scaffolding!("drop");
