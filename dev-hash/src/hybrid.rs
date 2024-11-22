use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

use blake3::Hasher as Blake3Hasher;
use core::{fmt::Display, str::FromStr};
use hex::encode;
use serde::{Deserialize, Serialize};

use data_error::Result;
use data_resource::ResourceId;

use std::hash::{Hash, Hasher};

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv_hash_bytes(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes.iter() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn fnv_hash_path<P: AsRef<Path>>(path: P) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.as_ref().hash(&mut hasher);
    let hash = hasher.finish();
    fnv_hash_bytes(hash.to_le_bytes().as_slice())
}

#[derive(
    Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
pub struct Hybrid(pub String);

impl FromStr for Hybrid {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        Ok(Hybrid(s.to_string()))
    }
}

impl Display for Hybrid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

const THRESHOLD: u64 = 1024 * 1024 * 1024;

impl ResourceId for Hybrid {
    fn from_path<P: AsRef<Path>>(file_path: P) -> Result<Self> {
        let size = fs::metadata(file_path.as_ref())?.len();

        if size < THRESHOLD {
            // Use Blake3 for small files
            log::debug!(
                "Computing BLAKE3 hash for file: {:?}",
                file_path.as_ref()
            );

            let file = fs::File::open(file_path)?;
            let mut reader = BufReader::new(file);
            let mut hasher = Blake3Hasher::new();
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
            Ok(Hybrid(encode(hash.as_bytes())))
        } else {
            // Use fnv   hashing for large files
            log::debug!(
                "Computing simple hash for file: {:?}",
                file_path.as_ref()
            );

            let hash = fnv_hash_path(file_path);
            Ok(Hybrid(format!("{}_{}", size, hash)))
        }
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let size = bytes.len() as u64;
        if size < THRESHOLD {
            // Use Blake3 for small files
            log::debug!("Computing BLAKE3 hash for bytes");

            let mut hasher = Blake3Hasher::new();
            hasher.update(bytes);
            let hash = hasher.finalize();
            Ok(Hybrid(encode(hash.as_bytes())))
        } else {
            // Use fnv hashing for large files
            log::debug!("Computing simple hash for bytes");

            let hash = fnv_hash_bytes(bytes);
            Ok(Hybrid(format!("{}_{}", size, hash)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity_check() {
        let file_path = Path::new("../test-assets/lena.jpg");
        let id = Hybrid::from_path(file_path)
            .expect("Failed to compute resource identifier");
        assert_eq!(
            id,
            Hybrid("172b4bf148e858b13dde0fc6613413bcb7552e5c4e5c45195ac6c80f20eb5ff5".to_string())
        );

        let raw_bytes = fs::read(file_path).expect("Failed to read file");
        let id = <Hybrid as ResourceId>::from_bytes(&raw_bytes)
            .expect("Failed to compute resource identifier");
        assert_eq!(
            id,
            Hybrid("172b4bf148e858b13dde0fc6613413bcb7552e5c4e5c45195ac6c80f20eb5ff5".to_string())
        );
    }
}
