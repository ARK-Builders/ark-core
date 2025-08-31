//! Types representing in-memory file data used by higher-level transfer logic.
//!
//! Projections are useful for tests, small payloads, or scenarios where file
//! contents are already available in memory.

use serde::{Deserialize, Serialize};

/// In-memory representation of a file's contents identified by a logical ID.
///
/// This is commonly paired with [`crate::handshake::HandshakeFile`], where
/// `id` corresponds to the same `id` advertised by the sender during the
/// handshake.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileProjection {
    /// Logical identifier for the file, typically matching
    /// [`crate::handshake::HandshakeFile::id`].
    pub id: String,
    /// Raw file bytes.
    pub data: Vec<u8>,
}
