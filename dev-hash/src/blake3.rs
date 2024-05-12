use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

use blake3::Hasher;
use core::{fmt::Display, str::FromStr};
use hex::encode;
use serde::{Deserialize, Serialize};

use data_error::Result;
use data_resource::ResourceIdTrait;

/// Represents a resource identifier using the BLAKE3 algorithm.
///
/// Uses [`blake3`] crate to compute the hash value.
#[derive(
    Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
pub struct ResourceId(pub String);

impl FromStr for ResourceId {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        Ok(ResourceId(s.to_string()))
    }
}

impl Display for ResourceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ResourceIdTrait for ResourceId {
    fn from_path<P: AsRef<Path>>(file_path: P) -> Result<Self> {
        log::debug!("Computing BLAKE3 hash for file: {:?}", file_path.as_ref());

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
        let hash = hasher.finalize();
        Ok(ResourceId(encode(hash.as_bytes())))
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        log::debug!("Computing BLAKE3 hash for bytes");

        let mut hasher = Hasher::new();
        hasher.update(bytes);
        let hash = hasher.finalize();
        Ok(ResourceId(encode(hash.as_bytes())))
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
        assert_eq!(
            id,
            ResourceId("172b4bf148e858b13dde0fc6613413bcb7552e5c4e5c45195ac6c80f20eb5ff5".to_string())
        );

        let raw_bytes = fs::read(file_path).expect("Failed to read file");
        let id = ResourceId::from_bytes(&raw_bytes)
            .expect("Failed to compute resource identifier");
        assert_eq!(
            id,
            ResourceId("172b4bf148e858b13dde0fc6613413bcb7552e5c4e5c45195ac6c80f20eb5ff5".to_string())  
        );
    }
}
