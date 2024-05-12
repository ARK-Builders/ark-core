use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

use core::{fmt::Display, str::FromStr};
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};

use data_error::Result;
use data_resource::ResourceIdTrait;

/// Represents a resource identifier using the CRC32 algorithm.
///
/// Uses [`crc32fast`] crate to compute the hash value.
#[derive(
    Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
pub struct ResourceId(pub u32);

impl FromStr for ResourceId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        Ok(ResourceId(u32::from_str(s)?))
    }
}

impl Display for ResourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ResourceIdTrait for ResourceId {
    fn from_path<P: AsRef<Path>>(file_path: P) -> Result<Self> {
        log::debug!("Computing CRC32 hash for file: {:?}", file_path.as_ref());

        let file = fs::File::open(file_path)?;
        let mut reader = BufReader::new(file);
        let mut hasher = Hasher::new();
        let mut buffer = Vec::new();
        loop {
            let bytes_read = reader.read_until(b'\n', &mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer);
            buffer.clear();
        }
        Ok(ResourceId(hasher.finalize()))
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        log::debug!("Computing CRC32 hash for bytes");

        let mut hasher = Hasher::new();
        hasher.update(bytes);
        Ok(ResourceId(hasher.finalize()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity_check() {
        let file_path = Path::new("../test-assets/lena.jpg");
        let id = ResourceId::from_path(file_path)
            .expect("Failed to compute resource identifier");
        assert_eq!(id, ResourceId(875183434));

        let raw_bytes = fs::read(file_path).expect("Failed to read file");
        let id = ResourceId::from_bytes(&raw_bytes)
            .expect("Failed to compute resource identifier");
        assert_eq!(id, ResourceId(875183434));
    }
}
