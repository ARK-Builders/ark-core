//! DropX sender crate.
//!
//! This crate provides a high-level API to send one or more files to a peer
//! over iroh/QUIC. It exposes:
//! - Data model types for the sender (profile, files, configuration).
//! - A `send_files` function (re-exported from `send_files`) that initializes
//!   an iroh Endpoint, negotiates settings, and streams file data.
//! - A `SendFilesBubble` handle that lets you observe progress, subscribe to
//!   events, and cancel or query the transfer.
//!
//! Typical usage:
//! - Implement `SenderFileData` for your source (bytes in memory, file on disk,
//!   etc.).
//! - Construct `SenderFile` values for each file.
//! - Choose a `SenderConfig` (or the default).
//! - Call `send_files` to start a transfer and get a `SendFilesBubble`.
//!
//! See `send_files` module for the operational flow and events.

mod send_files;

use drop_entities::Data;
use std::sync::Arc;

pub use send_files::*;

/// Sender's profile metadata transmitted during the handshake.
///
/// This information is displayed to the receiver to identify the sender.
pub struct SenderProfile {
    /// Display name of the sender shown to the receiver.
    pub name: String,
    /// Optional base64-encoded avatar image for the sender.
    pub avatar_b64: Option<String>,
}

/// A single file to be sent.
///
/// The file contains a human-friendly `name` and a data source implementing
/// [`SenderFileData`]. The data source is read chunk-by-chunk during transfer.
pub struct SenderFile {
    /// File name presented to the receiver.
    pub name: String,

    /// Backing data source. Must be thread-safe, since reads can occur on
    /// background tasks.
    pub data: Arc<dyn SenderFileData>,
}

/// Trait for a readable file-like data source used by the sender.
///
/// Implement this trait to stream data from any origin (e.g., in-memory buffer,
/// filesystem, network, etc.). The implementor must be thread-safe (`Send +
/// Sync`) as reads can occur from async tasks.
///
/// Contract:
/// - `len` returns the total number of bytes available.
/// - `read_chunk(size)` returns the next chunk up to `size` bytes; an empty
///   vector signals EOF.
/// - `read` is a single-byte variant primarily to satisfy the
///   `drop_entities::Data` trait; it can be implemented in terms of your
///   internal reader if needed.
pub trait SenderFileData: Send + Sync {
    /// Total length in bytes.
    fn len(&self) -> u64;

    /// Read a single byte if available.
    fn read(&self) -> Option<u8>;

    /// Read up to `size` bytes. Return an empty vector to indicate EOF.
    fn read_chunk(&self, size: u64) -> Vec<u8>;
}

/// Internal adapter to bridge `SenderFileData` with `drop_entities::Data`.
///
/// This type is not exposed publicly; it allows the rest of the pipeline to
/// operate on the generic `drop_entities::File` type.
struct SenderFileDataAdapter {
    inner: Arc<dyn SenderFileData>,
}
impl Data for SenderFileDataAdapter {
    fn len(&self) -> u64 {
        return self.inner.len();
    }

    fn read(&self) -> Option<u8> {
        return self.inner.read();
    }

    fn read_chunk(&self, size: u64) -> Vec<u8> {
        return self.inner.read_chunk(size);
    }
}

/// Tuning knobs for file transfer performance.
///
/// These values are included in the handshake, and the effective settings may
/// be negotiated with the receiver. Prefer `balanced` for general usage.
#[derive(Clone, Debug)]
pub struct SenderConfig {
    /// Target chunk size in bytes sent over each unidirectional stream.
    pub chunk_size: u64,
    /// Maximum number of unidirectional streams used in parallel.
    pub parallel_streams: u64,
}
impl Default for SenderConfig {
    /// Balanced defaults: 512 KiB chunks, 4 parallel streams.
    fn default() -> Self {
        Self {
            chunk_size: 524288,  // 512KB chunks
            parallel_streams: 4, // 4 parallel streams
        }
    }
}
impl SenderConfig {
    /// Higher throughput at the cost of more memory and bandwidth.
    ///
    /// 512 KiB chunks, 8 parallel streams.
    pub fn high_performance() -> Self {
        Self {
            chunk_size: 524288,  // 512KB chunks
            parallel_streams: 8, // 8 parallel streams
        }
    }

    /// Alias for the default balanced configuration.
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Lower bandwidth footprint for constrained networks.
    ///
    /// 64 KiB chunks, 2 parallel streams.
    pub fn low_bandwidth() -> Self {
        Self {
            chunk_size: 65536,   // 64KB chunks
            parallel_streams: 2, // 2 parallel streams
        }
    }
}
