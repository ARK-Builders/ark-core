//! # Data Resource
//!
//! `data-resource` is a crate for managing resource identifiers. It provides different
//! implementations of resource identifiers ([`ResourceId`]) based on various hash algorithms.
//!
//! ## Features
//!
//! - `non-cryptographic-hash`: Enables the use of a non-cryptographic hash function to define `ResourceId`.
//! - `cryptographic-hash`: Enables the use of cryptographic hash functions to define `ResourceId`.
//!
//! By default, `cryptographic-hash` feature is enabled.

use core::{fmt::Display, str::FromStr};
use data_error::Result;
use serde::Serialize;
use std::{fmt::Debug, hash::Hash, path::Path};

// The `ResourceId` type is a wrapper around the hash value of the resource.
//
// To export another `ResourceId` type as the default for cryptographic or non-cryptographic hash,
// implement the `ResourceIdTrait` trait for your type and re-export it here behind the
// right feature flag.

#[cfg(not(feature = "non-cryptographic-hash"))]
mod blake3;
#[cfg(not(feature = "non-cryptographic-hash"))]
pub use blake3::ResourceId as Resource;
#[cfg(not(feature = "non-cryptographic-hash"))]
pub type ResourceId = <Resource as ResourceIdTrait>::HashType;

#[cfg(feature = "non-cryptographic-hash")]
mod crc32;
#[cfg(feature = "non-cryptographic-hash")]
pub use crc32::ResourceId as Resource;
#[cfg(feature = "non-cryptographic-hash")]
pub type ResourceId = <Resource as ResourceIdTrait>::HashType;

/// This trait defines a generic type representing a resource identifier.
///
/// Resources are identified by a hash value, which is computed from the resource's data.
/// The hash value is used to uniquely identify the resource.
///
/// Implementors of this trait must provide a way to compute the hash value from the resource's data.
pub trait ResourceIdTrait {
    /// Associated type representing the hash used by this resource identifier.
    type HashType: Debug
        + Display
        + FromStr
        + Clone
        + PartialEq
        + Eq
        + Ord
        + PartialOrd
        + Hash
        + Serialize;

    /// Computes the resource identifier from the given file path
    fn from_path<P: AsRef<Path>>(file_path: P) -> Result<Self::HashType>;

    /// Computes the resource identifier from the given bytes
    fn from_bytes(data: &[u8]) -> Result<Self::HashType>;
}
