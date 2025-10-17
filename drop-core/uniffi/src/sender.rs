//! Public sender-facing types re-exported to foreign bindings.
//!
//! These types form the high-level API that foreign bindings use. The concrete
//! file data is provided by the embedding app via the `SenderFileData` trait.

mod send_files;

use std::sync::Arc;

pub use send_files::*;

/// Describes the sender's identity, shown to the receiver during handshake.
pub struct SenderProfile {
    /// Display name shown to the receiver.
    pub name: String,
    /// Optional avatar image (base64-encoded).
    pub avatar_b64: Option<String>,
}

/// A single file to be sent with an associated streaming data source.
pub struct SenderFile {
    /// File name as presented to the receiver.
    pub name: String,
    /// Streaming data provider implemented by the embedding app.
    pub data: Arc<dyn SenderFileData>,
}

/// Streaming file data provider implemented by the embedding app.
///
/// Implementations must be Send + Sync because reads may be performed on
/// background threads. Implementations should be sequential/read-only.
pub trait SenderFileData: Send + Sync {
    /// Total number of bytes available.
    fn len(&self) -> u64;

    /// Returns true if the data has zero length.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Read the next byte, or None at EOF.
    fn read(&self) -> Option<u8>;
    /// Read up to `size` bytes; fewer may be returned at EOF.
    fn read_chunk(&self, size: i32) -> Vec<u8>;
}

/// Adapter that bridges this crate's `SenderFileData` trait to the
/// `dropx_sender::SenderFileData` trait expected by the lower-level crate.
struct SenderFileDataAdapter {
    inner: Arc<dyn SenderFileData>,
}
impl dropx_sender::SenderFileData for SenderFileDataAdapter {
    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn read(&self) -> Option<u8> {
        self.inner.read()
    }

    fn read_chunk(&self, size: u64) -> Vec<u8> {
        self.inner.read_chunk(size.try_into().unwrap())
    }
}

/// Tuning parameters for the send pipeline.
///
/// - `chunk_size`: bytes per chunk when streaming.
/// - `parallel_streams`: number of concurrent channels used by the transport.
pub struct SenderConfig {
    pub chunk_size: u64,
    pub parallel_streams: u64,
}
