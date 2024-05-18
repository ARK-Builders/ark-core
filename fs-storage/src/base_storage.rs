use data_error::Result;
use std::collections::BTreeMap;

pub trait BaseStorage<K, V>: AsRef<BTreeMap<K, V>> {
    /// Create or update an entry in the internal mapping.
    fn set(&mut self, id: K, value: V);

    /// Remove an entry from the internal mapping.
    fn remove(&mut self, id: &K) -> Result<()>;

    /// Check if the storage is up-to-date,
    /// i.e. that the internal mapping is consistent
    /// with the data in the filesystem.
    fn is_outdated(&self) -> Result<bool>;

    /// Scan and load the key-value mapping
    /// from pre-configured location in the filesystem.
    fn read_fs(&mut self) -> Result<BTreeMap<K, V>>;

    /// Persist the internal key-value mapping
    /// to pre-configured location in the filesystem.
    fn write_fs(&mut self) -> Result<()>;

    /// Remove all persisted data
    /// by pre-configured location in the file-system.
    fn erase(&self) -> Result<()>;
}
