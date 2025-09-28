//! Basic identity and avatar metadata for a user-like entity.
//!
//! This module defines [`Profile`], a small, cloneable type that carries
//! stable identity and display information. It is suitable for indexing
//! and caching by identity.

use std::hash::Hash;

/// Lightweight profile metadata with a stable identifier.
///
/// The `id` field determines hashing behavior (see `Hash` impl).
#[derive(Clone, Debug)]
pub struct Profile {
    /// Stable, unique identifier for this profile (e.g., a user ID).
    pub id: String,

    /// Human-readable display name.
    pub name: String,

    /// Optional Base64-encoded avatar image.
    ///
    /// This is intended to be small, embedded metadata. For large or
    /// high-resolution avatars, consider storing them externally and
    /// referencing by URL instead of embedding the bytes here.
    pub avatar_b64: Option<String>,
}

impl Hash for Profile {
    /// Hashes only the `id` of the profile.
    ///
    /// This allows `Profile` instances to be used in hashed collections
    /// keyed by identity. Ensure `id` is unique to avoid collisions.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
