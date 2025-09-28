//! Minimal models for files and profiles with a pluggable, thread-safe
//! byte source abstraction.
//!
//! This crate provides:
//! - `Data`: a trait for thread-safe, read-only byte sources.
//! - `File`: a lightweight wrapper around an `Arc<dyn Data>` with identity and
//!   a human-readable name.
//! - `Profile`: simple identity and avatar metadata for a user-like entity.
//!
//! Example (illustrative):
//! ```rust,ignore
//! use std::sync::{Arc, Mutex};
//!
//! // Bring items into scope when used from your crate:
//! // use drop_entities::{Data, File, Profile};
//!
//! // A simple in-memory Data implementation with a protected cursor.
//! struct InMemoryData {
//!     buf: Vec<u8>,
//!     pos: Mutex<usize>,
//! }
//! impl InMemoryData {
//!     fn new(bytes: impl Into<Vec<u8>>) -> Self {
//!         Self { buf: bytes.into(), pos: Mutex::new(0) }
//!     }
//! }
//! // Implement the trait from this crate for the type above.
//! impl drop_entities::Data for InMemoryData {
//!     fn len(&self) -> u64 { self.buf.len() as u64 }
//!     fn read(&self) -> Option<u8> {
//!         let mut p = self.pos.lock().unwrap();
//!         if *p >= self.buf.len() { return None; }
//!         let b = self.buf[*p];
//!         *p += 1;
//!         Some(b)
//!     }
//!     fn read_chunk(&self, size: u64) -> Vec<u8> {
//!         let mut p = self.pos.lock().unwrap();
//!         if *p >= self.buf.len() { return Vec::new(); }
//!         let end = (*p + size as usize).min(self.buf.len());
//!         let out = self.buf[*p..end].to_vec();
//!         *p = end;
//!         out
//!     }
//! }
//!
//! // Construct a File backed by the in-memory data.
//! let data = Arc::new(InMemoryData::new(b"hello"));
//! let file = drop_entities::File {
//!     id: "file-1".into(),
//!     name: "greeting.txt".into(),
//!     data: data.clone(),
//! };
//!
//! // Basic usage
//! assert_eq!(file.name, "greeting.txt");
//! assert_eq!(file.data.len(), 5);
//! assert_eq!(file.data.read_chunk(2), b"he".to_vec());
//!
//! // A simple profile
//! let profile = drop_entities::Profile { id: "42".into(), name: "Ada".into(), avatar_b64: None };
//! assert_eq!(profile.name, "Ada");
//! ```

mod data;
mod file;
mod profile;

/// Re-export of the core data source trait.
pub use data::Data;
/// Re-export of the file abstraction backed by `Data`.
pub use file::File;
/// Re-export of a simple identity/metadata profile.
pub use profile::Profile;
