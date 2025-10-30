//! Receive-side library for DropX transfers.
//!
//! This crate provides the API surface for initiating and controlling a file
//! reception session from a remote sender over QUIC (via iroh). It exposes:
//! - Receiver configuration presets (chunk sizing and parallelism).
//! - Receiver identity/profile used during handshake.
//! - Events and subscription mechanisms (see `receive_files` module) to observe
//!   connection and per-chunk progress.
//!
//! Typical flow:
//! 1. Build a `ReceiveFilesRequest` with a sender ticket, confirmation code,
//!    your `ReceiverProfile`, and an optional `ReceiverConfig`.
//! 2. Call `receive_files::receive_files` to obtain a `ReceiveFilesBubble`.
//! 3. Subscribe to events to observe connection and progress.
//! 4. Start the transfer with `ReceiveFilesBubble::start()`.
//! 5. Optionally cancel with `ReceiveFilesBubble::cancel()`.
//! 6. When finished, the session is closed and resources cleaned up.

mod receive_files;

pub use receive_files::*;

/// Identity and presentation for the receiving peer.
///
/// This profile is sent during the handshake so the sender can display who is
/// receiving the transfer.
pub struct ReceiverProfile {
    /// Human-readable display name for the receiver.
    pub name: String,
    /// Optional avatar image encoded as Base64 (e.g., PNG/JPEG).
    pub avatar_b64: Option<String>,
}

/// Tunable settings that influence how data is fetched from the sender.
///
/// - `chunk_size` controls the maximum serialized size (in bytes) of each
///   projection chunk. Larger chunks generally improve throughput at the cost
///   of memory spikes and tail latency.
/// - `parallel_streams` controls how many unidirectional streams are processed
///   concurrently. Increasing this may improve throughput on high-bandwidth,
///   high-latency links, but can contend for CPU and memory.
///
/// Use one of the presets (`high_performance`, `balanced`, `low_bandwidth`) or
/// construct/override as needed.
#[derive(Clone)]
pub struct ReceiverConfig {
    /// Target chunk size in bytes for incoming file projections.
    pub chunk_size: u64,
    /// Number of unidirectional streams to process concurrently.
    pub parallel_streams: u64,
}

impl Default for ReceiverConfig {
    /// Returns the balanced preset:
    /// - 512 KiB chunks
    /// - 4 parallel streams
    fn default() -> Self {
        Self {
            chunk_size: 1024 * 512, // 512KB chunks
            parallel_streams: 4,    // 4 parallel streams
        }
    }
}
impl ReceiverConfig {
    /// Preset optimized for higher bandwidth and modern hardware:
    /// - 512 KiB chunks
    /// - 8 parallel streams
    pub fn high_performance() -> Self {
        Self {
            chunk_size: 1024 * 512, // 512KB chunks
            parallel_streams: 8,    // 8 parallel streams
        }
    }

    /// Alias of `Default::default()` returning a balanced configuration.
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Preset tuned for constrained or lossy networks:
    /// - 64 KiB chunks
    /// - 2 parallel streams
    pub fn low_bandwidth() -> Self {
        Self {
            chunk_size: 1024 * 64, // 64KB chunks
            parallel_streams: 2,   // 2 parallel streams
        }
    }
}
