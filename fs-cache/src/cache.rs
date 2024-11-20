use crate::memory_limited_storage::MemoryLimitedStorage;
use data_error::Result;
use std::path::Path;

pub struct Cache<K, V> {
    storage: MemoryLimitedStorage<K, V>,
}

impl<K, V> Cache<K, V>
where
    K: Ord
        + Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::fmt::Display
        + std::hash::Hash
        + std::str::FromStr,
    V: Clone + serde::Serialize + serde::de::DeserializeOwned,
{
    pub fn new(
        label: String,
        path: &Path,
        max_memory_bytes: usize,
    ) -> Result<Self> {
        log::debug!(
            "{} cache initialized with {} bytes limit",
            label,
            max_memory_bytes
        );
        Ok(Self {
            storage: MemoryLimitedStorage::new(label, path, max_memory_bytes)?,
        })
    }

    pub fn get(&mut self, key: &K) -> Result<Option<V>> {
        let result = self.storage.get(key)?;
        log::debug!(
            "{} cache: get key={} -> found={}",
            self.storage.label(),
            key,
            result.is_some()
        );
        Ok(result)
    }

    pub fn set(&mut self, key: K, value: V) -> Result<()> {
        log::debug!("{} cache: set key={}", self.storage.label(), key);
        self.storage.set(key, value)
    }
}
