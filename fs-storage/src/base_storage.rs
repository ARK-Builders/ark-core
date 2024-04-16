use data_error::Result;
use std::collections::BTreeMap;

pub trait BaseStorage<K, V>: AsRef<BTreeMap<K, V>> {
    fn set(&mut self, id: K, value: V);
    fn remove(&mut self, id: &K) -> Result<()>;

    /// Remove file at stored path
    fn erase(&self) -> Result<()>;

    /// Check if storage is updated
    ///
    /// This check can be used before reading the file.
    fn is_storage_updated(&self) -> Result<bool>;

    /// Read data from disk
    ///
    /// Data is read as key value pairs separated by a symbol and stored
    /// in a [BTreeMap] with a generic key K and V value. A handler
    /// is called on the data after reading it.
    fn read_fs(&mut self) -> Result<BTreeMap<K, V>>;

    /// Write data to file
    ///
    /// Data is a key-value mapping between [ResourceId] and a generic Value
    fn write_fs(&mut self) -> Result<()>;
}
