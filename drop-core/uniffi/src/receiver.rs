//! Public receiver-facing types re-exported to foreign bindings.
//!
//! These are thin, typed wrappers around the lower-level `arkdrop_x_receiver`
//! crate.

mod receive_files;

pub use receive_files::*;

/// Describes the receiver's identity, shown to the sender during handshake.
pub struct ReceiverProfile {
    /// Display name shown to the sender.
    pub name: String,
    /// Optional avatar image (base64-encoded).
    pub avatar_b64: Option<String>,
}

/// Tuning parameters for the receive pipeline.
///
/// - `chunk_size`: desired bytes per chunk.
/// - `parallel_streams`: number of concurrent channels used by the transport.
pub struct ReceiverConfig {
    pub chunk_size: u64,
    pub parallel_streams: u64,
}
