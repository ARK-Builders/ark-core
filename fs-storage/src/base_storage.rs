use std::collections::BTreeMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::str::FromStr;

use data_error::Result;

pub trait BaseStorage<K, V>
where
    K: FromStr + Hash + Eq + Ord + Debug + Clone,
    V: Debug + Clone,
{
    fn get(&self, id: &K) -> Option<V>;
    fn set(&mut self, id: K, value: V) -> Result<()>;
    fn remove(&mut self, id: &K) -> Result<()>;
    fn erase(&mut self) -> Result<()>;

    /// Check if underlying file has been updated
    ///
    /// This check can be used before reading the file.
    fn is_file_updated(&self) -> Result<bool>;

    /// Read data from disk
    ///
    /// Data is read as key value pairs separated by a symbol and stored
    /// in a [BTreeMap] with a generic key K and V value. A handler
    /// is called on the data after reading it.
    fn read_file(&mut self) -> Result<BTreeMap<K, V>>;

    /// Write data to file
    ///
    /// Data is a key-value mapping between [ResourceId] and a generic Value
    fn write_file(&mut self) -> Result<()>;
}

pub struct BaseStorageImpl<K, V>
where
    K: FromStr + Hash + Eq + Ord + Debug + Clone,
    V: Debug + Clone,
{
    pub label: String,
    pub value_by_id: BTreeMap<K, V>,
    pub timestamp: std::time::SystemTime,
}

impl<K, V> BaseStorageImpl<K, V>
where
    K: FromStr + Hash + Eq + Ord + Debug + Clone,
    V: Debug + Clone,
{
    pub fn new(label: &str) -> Self {
        BaseStorageImpl {
            label: label.to_string(),
            value_by_id: BTreeMap::new(),
            timestamp: std::time::SystemTime::now(),
        }
    }
}

impl<K, V> BaseStorage<K, V> for BaseStorageImpl<K, V>
where
    K: FromStr + Hash + Eq + Ord + Debug + Clone,
    V: Debug + Clone,
{
    fn get(&self, id: &K) -> Option<V> {
        self.value_by_id.get(id).cloned()
    }

    fn set(&mut self, id: K, value: V) -> Result<()> {
        self.value_by_id.insert(id, value);
        self.timestamp = std::time::SystemTime::now();
        Ok(())
    }

    fn remove(&mut self, id: &K) -> Result<()> {
        self.value_by_id.remove(id);
        self.timestamp = std::time::SystemTime::now();
        Ok(())
    }

    fn erase(&mut self) -> Result<()> {
        Ok(())
    }

    fn is_file_updated(&self) -> Result<bool> {
        Ok(false)
    }

    fn read_file(&mut self) -> Result<BTreeMap<K, V>> {
        todo!()
    }

    fn write_file(&mut self) -> Result<()> {
        todo!()
    }
}
