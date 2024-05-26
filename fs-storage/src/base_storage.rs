use data_error::Result;
use std::collections::BTreeMap;

pub trait BaseStorage<K, V>: AsRef<BTreeMap<K, V>> {
    /// Create or update an entry in the internal mapping.
    fn set(&mut self, id: K, value: V);

    /// Remove an entry from the internal mapping.
    fn remove(&mut self, id: &K) -> Result<()>;

    /// Determine if in-memory model
    /// or the underlying storage requires syncing.
    /// This is a quick method checking timestamps
    /// of modification of both model and storage.
    ///
    /// Returns:
    /// - `Ok(true)` if the on-disk data and in-memory data are not in sync.
    /// - `Ok(false)` if the on-disk data and in-memory data are in sync.
    /// - `Err(ArklibError::Storage)` in case of any error retrieving the file metadata.
    fn needs_syncing(&self) -> Result<bool>;

    /// Scan and load the key-value mapping
    /// from pre-configured location in the filesystem.
    fn read_fs(&mut self) -> Result<BTreeMap<K, V>>;

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
