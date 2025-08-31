//! A high-level representation of a file-like entity backed by `Data`.
//!
//! The [`File`] type pairs stable identity (`id`) and human-friendly metadata
//! (`name`) with a thread-safe byte source (`data`). It is intentionally
//! lightweight and cloneable, so it can be passed around systems that need
//! to reference content without copying the bytes themselves.

use std::{hash::Hash, sync::Arc};

use crate::Data;

/// A named, identifiable file backed by a `Data` source.
///
/// Cloning a `File` is cheap and clones the internal `Arc` to the underlying
/// [`Data`] implementation.
///
/// Notes:
/// - The custom `Debug` implementation intentionally omits the `data` field to
///   avoid large outputs and to prevent accidental leakage of internal state.
/// - The `Hash` implementation uses only the `id`, so two `File` values with
///   the same `id` will hash identically. Choose `id` values that are stable
///   and globally unique within your domain.
#[derive(Clone)]
pub struct File {
    /// Stable, unique identifier for this file (e.g., a UUID).
    pub id: String,

    /// Human-readable name (e.g., display filename).
    pub name: String,

    /// Thread-safe, read-only byte source for the file's contents.
    ///
    /// The `Data` trait is object-safe and can be shared across threads
    /// with the `Arc`. Implementations must be `Send + Sync`.
    pub data: Arc<dyn Data>,
}

impl std::fmt::Debug for File {
    /// Formats `File` for debugging without including the potentially large
    /// or sensitive `data` field.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("File")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl Hash for File {
    /// Hashes only the `id` of the file.
    ///
    /// This allows `File` instances to be used in hashed collections keyed
    /// by identity. Ensure `id` is unique to avoid collisions.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
