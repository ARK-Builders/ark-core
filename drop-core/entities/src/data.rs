//! Core data abstraction for byte-oriented, read-only streams.
//!
//! This module defines the `Data` trait, which models a thread-safe,
//! sequential source of bytes that can be consumed one byte at a time
//! or in chunks. Typical implementors include in-memory buffers,
//! files, and network-backed sources.
//!
//! Concurrency notes:
//! - All methods take `&self`, and the trait is `Send + Sync`, so implementors
//!   must ensure internal synchronization when maintaining read cursors or
//!   other mutable state.
//! - The default expectation is that reads advance an internal cursor (i.e.,
//!   are consuming). If an implementation is non-consuming, document that
//!   clearly for your type.

/// A thread-safe, sequential source of bytes.
///
/// Implementors must be `Send + Sync` so instances can be shared across
/// threads (e.g., behind `Arc`) while remaining safe to read concurrently.
/// Reads are expected to be consuming: each call advances an internal cursor.
///
/// Contract:
/// - `len()` returns the total length of the underlying data in bytes. It
///   should not change over the lifetime of the object.
/// - `read()` returns the next byte from the current position, or `None` if the
///   end of data has been reached.
/// - `read_chunk(size)` attempts to read up to `size` bytes from the current
///   position and returns them. It may return fewer bytes if fewer are
///   available, and an empty `Vec` when at end-of-stream.
pub trait Data: Send + Sync {
    /// Total length of the data in bytes.
    ///
    /// This is the full size of the underlying content, not the number
    /// of unread bytes. For streaming-like sources, this should reflect
    /// the known total length at creation time.
    fn len(&self) -> u64;

    /// Checks if the data is empty (length is 0).
    ///
    /// Default implementation returns `true` if `len() == 0`.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reads the next byte from the current position.
    ///
    /// Returns:
    /// - `Some(u8)` with the next byte if available.
    /// - `None` if the end of data has been reached.
    ///
    /// Implementations should advance their internal cursor when a byte
    /// is returned. This method should not block indefinitely.
    fn read(&self) -> Option<u8>;

    /// Reads up to `size` bytes from the current position.
    ///
    /// - Returns a vector with at most `size` bytes.
    /// - May return fewer bytes if fewer remain, and an empty vector at
    ///   end-of-stream.
    /// - Calling with `size == 0` should return an empty vector.
    ///
    /// Implementations should advance their internal cursor by the number
    /// of bytes returned.
    fn read_chunk(&self, size: u64) -> Vec<u8>;
}
