use data_error::Result;
use fs_storage::{base_storage::SyncStatus, monoid::Monoid};
use std::path::Path;

use crate::memory_limited_storage::MemoryLimitedStorage;

/// A generic cache implementation that stores values with LRU eviction in memory
/// and persistence to disk.
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
    V: Clone + serde::Serialize + serde::de::DeserializeOwned + Monoid<V>,
{
    /// Create a new cache with given capacity
    /// - `label`: Used for logging and error messages
    /// - `path`: Directory where cache files will be stored
    /// - `max_memory_items`: Maximum number of items to keep in memory
    pub fn new(
        label: String,
        path: &Path,
        max_memory_items: usize,
    ) -> Result<Self> {
        let storage = MemoryLimitedStorage::new(label, path, max_memory_items)?;

        Ok(Self { storage })
    }

    /// Get a value from the cache if it exists
    /// Returns None if not found
    pub fn get(&mut self, key: &K) -> Result<Option<V>> {
        self.storage.get(key)
    }

    /// Store a value in the cache
    /// Will persist to disk and maybe keep in memory based on LRU policy
    pub fn set(&mut self, key: K, value: V) -> Result<()> {
        self.storage.set(key, value)
    }

    /// Load most recent cached items into memory based on timestamps
    pub fn load_recent(&mut self) -> Result<()> {
        self.storage.load_fs()
    }

    /// Get number of items currently in memory
    // pub fn memory_items(&self) -> usize {
    //     self.storage.memory_items()
    // }

    /// Get sync status between memory and disk
    pub fn sync_status(&self) -> Result<SyncStatus> {
        self.storage.sync_status()
    }

    /// Sync changes to disk
    pub fn sync(&mut self) -> Result<()> {
        self.storage.sync()
    }
}
