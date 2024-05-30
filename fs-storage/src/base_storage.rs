use data_error::Result;
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Clone)]
/// Represents the synchronization status of the storage.
pub enum SyncStatus {
    /// No synchronization needed.
    NoSync,
    /// Load key-value mapping from the file system.
    UpSync,
    /// Write key-value mapping to the file system.
    DownSync,
    /// Changes from both in-memory and file system need to be merged before syncing.
    FullSync,
}

/// A trait for a key-value mapping that is persisted to the file system.
pub trait BaseStorage<K, V>: AsRef<BTreeMap<K, V>> {
    /// Create or update an entry in the internal mapping.
    fn set(&mut self, id: K, value: V);

    /// Remove an entry from the internal mapping.
    fn remove(&mut self, id: &K) -> Result<()>;

    /// Determine if the in-memory model or the underlying storage requires syncing.
    ///
    /// Returns:
    /// - `Ok(SyncStatus)` indicating the type of syncing required.
    /// - `Err(ArklibError::Storage)` in case of any error retrieving the file metadata.
    fn needs_syncing(&self) -> Result<SyncStatus>;

    /// Scan and load the key-value mapping
    /// from pre-configured location in the filesystem.
    fn read_fs(&mut self) -> Result<&BTreeMap<K, V>>;

    /// Persist the internal key-value mapping
    /// to pre-configured location in the filesystem.
    fn write_fs(&mut self) -> Result<()>;

    /// Remove all persisted data
    /// by pre-configured location in the file-system.
    fn erase(&self) -> Result<()>;

    /// Merge two storages instances
    /// and write the result to the filesystem.
    fn merge_from(&mut self, other: impl AsRef<BTreeMap<K, V>>) -> Result<()>;
}
