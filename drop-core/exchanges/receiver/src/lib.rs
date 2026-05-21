//! Receive-side library for DropX transfers.
//!
//! This crate provides the API surface for initiating and controlling a file
//! reception session from a remote sender over QUIC (via iroh). It exposes:
//! - Receiver configuration presets (chunk sizing and parallelism).
//! - Receiver identity/profile used during handshake.
//! - Events and subscription mechanisms (see `receive_files` module) to observe
//!   connection and per-chunk progress.
//!
//! Two modes of operation:
//!
//! ## Standard Mode (Receiver connects to Sender)
//! 1. Build a `ReceiveFilesRequest` with a sender ticket, confirmation code,
//!    your `ReceiverProfile`, and an optional `ReceiverConfig`.
//! 2. Call `receive_files::receive_files` to obtain a `ReceiveFilesBubble`.
//! 3. Subscribe to events to observe connection and progress.
//! 4. Start the transfer with `ReceiveFilesBubble::start()`.
//! 5. Optionally cancel with `ReceiveFilesBubble::cancel()`.
//! 6. When finished, the session is closed and resources cleaned up.
//!
//! ## QR-to-Receive Mode (Sender connects to Receiver)
//! 1. Build a `ReadyToReceiveRequest` with your `ReceiverProfile` and config.
//! 2. Call `ready_to_receive::ready_to_receive` to obtain a
//!    `ReadyToReceiveBubble`.
//! 3. Display the ticket and confirmation code (e.g., as QR code) for sender.
//! 4. Subscribe to events to observe when sender connects and file reception.
//! 5. Optionally cancel with `ReadyToReceiveBubble::cancel()`.

pub mod ready_to_receive;
mod receive_files;

use std::{
    io::{BufReader, Bytes, Read},
    sync::{RwLock, atomic::AtomicBool},
};

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

/// Metadata and data carrier representing a file being received.
///
/// Note: Depending on your flow, you may receive file data via events
/// (see `receive_files` module) rather than pulling bytes directly from this
/// structure.
#[derive(Debug)]
pub struct ReceiverFile {
    /// Unique, sender-provided file identifier.
    pub id: String,
    /// Human-readable file name (as provided by sender).
    pub name: String,
    /// Backing data accessor for byte-wise reads.
    pub data: ReceiverFileData,
}

/// Backing data abstraction for a locally stored file used by the receiver.
///
/// This type supports:
/// - Lazy initialization of a byte iterator over the file.
/// - Byte-wise `read()` until EOF, returning `None` when complete.
/// - A simple `is_finished` flag to short-circuit further reads after EOF.
///
/// Caveats:
/// - `len()` currently counts bytes by iterating the file; this is O(n) and
///   re-reads the file. Prefer using file metadata for length if available.
/// - `read()` is not optimized for high-throughput stream reads; it is intended
///   for simple scenarios and examples. Use buffered I/O where performance
///   matters.
#[derive(Debug)]
pub struct ReceiverFileData {
    is_finished: AtomicBool,
    path: std::path::PathBuf,
    reader: RwLock<Option<Bytes<BufReader<std::fs::File>>>>,
}
impl ReceiverFileData {
    /// Create a new `ReceiverFileData` from a filesystem path.
    pub fn new(path: std::path::PathBuf) -> Self {
        Self {
            is_finished: AtomicBool::new(false),
            path,
            reader: RwLock::new(None),
        }
    }

    /// Return the file length in bytes using file metadata (O(1)).
    pub fn len(&self) -> u64 {
        std::fs::metadata(&self.path)
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// Returns true if the data has zero length.
    #[allow(clippy::len_without_is_empty)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Read the next byte from the file, returning `None` at EOF or after
    /// the stream has been marked finished.
    ///
    /// This initializes an internal iterator on first use and cleans it up
    /// when EOF is reached. Subsequent calls after completion return `None`.
    pub fn read(&self) -> Option<u8> {
        use std::io::BufReader;

        if self
            .is_finished
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return None;
        }
        if self.reader.read().unwrap().is_none() {
            let file = std::fs::File::open(&self.path).unwrap();
            self.reader
                .write()
                .unwrap()
                .replace(BufReader::new(file).bytes());
        }
        let next = self
            .reader
            .write()
            .unwrap()
            .as_mut()
            .unwrap()
            .next();
        if let Some(read_result) = next
            && let Ok(byte) = read_result
        {
            return Some(byte);
        }
        *self.reader.write().unwrap() = None;
        self.is_finished
            .store(true, std::sync::atomic::Ordering::Relaxed);
        None
    }
}
