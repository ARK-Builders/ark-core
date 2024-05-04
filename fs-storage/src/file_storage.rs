use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::time::SystemTime;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::base_storage::BaseStorage;
use crate::monoid::Monoid;
use data_error::{ArklibError, Result};

const STORAGE_VERSION: i32 = 2;
const STORAGE_VERSION_PREFIX: &str = "version ";

pub struct FileStorage<K, V> {
    label: String,
    path: PathBuf,
    timestamp: SystemTime,
    data: BTreeMap<K, V>,
}

impl<K, V> FileStorage<K, V>
where
    K: Ord + Clone + serde::Serialize + serde::de::DeserializeOwned,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Default
        + std::cmp::PartialOrd,
{
    /// Create a new file storage with a diagnostic label and file path
    pub fn new(label: String, path: &Path) -> Self {
        let mut file_storage = Self {
            label,
            path: PathBuf::from(path),
            timestamp: SystemTime::now(),
            data: BTreeMap::new(),
        };

        // Load the data from the file
        file_storage.data = match file_storage.read_fs() {
            Ok(data) => data,
            Err(_) => BTreeMap::new(),
        };
        file_storage
    }

    /// Verify the version stored in the file header
    fn verify_version(&self, header: &str) -> Result<()> {
        if !header.starts_with(STORAGE_VERSION_PREFIX) {
            return Err(ArklibError::Storage(
                self.label.clone(),
                "Unknown storage version prefix".to_owned(),
            ));
        }

        let version = header[STORAGE_VERSION_PREFIX.len()..]
            .parse::<i32>()
            .map_err(|_err| {
                ArklibError::Storage(
                    self.label.clone(),
                    "Failed to parse storage version".to_owned(),
                )
            })?;

        if version != STORAGE_VERSION {
            return Err(ArklibError::Storage(
                self.label.clone(),
                format!(
                    "Storage version mismatch: expected {}, found {}",
                    STORAGE_VERSION, version
                ),
            ));
        }

        Ok(())
    }
}

impl<K, V> BaseStorage<K, V> for FileStorage<K, V>
where
    K: Ord + Clone + serde::Serialize + serde::de::DeserializeOwned,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Default
        + std::cmp::PartialOrd,
{
    fn set(&mut self, id: K, value: V) {
        self.data.insert(id, value);
        self.timestamp = std::time::SystemTime::now();
        self.write_fs()
            .expect("Failed to write data to disk");
    }

    fn remove(&mut self, id: &K) -> Result<()> {
        self.data.remove(id).ok_or_else(|| {
            ArklibError::Storage(self.label.clone(), "Key not found".to_owned())
        })?;
        self.timestamp = std::time::SystemTime::now();
        self.write_fs()
            .expect("Failed to remove data from disk");
        Ok(())
    }

    fn is_storage_updated(&self) -> Result<bool> {
        let file_timestamp = fs::metadata(&self.path)?.modified()?;
        let file_time_secs = file_timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let self_time_secs = self
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Ok(file_time_secs > self_time_secs)
    }

    fn read_fs(&mut self) -> Result<BTreeMap<K, V>> {
        let file = fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let new_timestamp = fs::metadata(&self.path)?.modified()?;
        match lines.next() {
            Some(header) => {
                let header = header?;
                self.verify_version(&header)?;
                let mut data = String::new();
                for line in lines {
                    let line = line?;
                    if line.is_empty() {
                        continue;
                    }
                    data.push_str(&line);
                }
                let data: BTreeMap<K, V> = serde_json::from_str(&data)?;
                self.timestamp = new_timestamp;
                Ok(data)
            }
            None => Err(ArklibError::Storage(
                self.label.clone(),
                "Storage file is missing header".to_owned(),
            )),
        }
    }

    fn write_fs(&mut self) -> Result<()> {
        let parent_dir = self.path.parent().ok_or_else(|| {
            ArklibError::Storage(
                self.label.clone(),
                "Failed to get parent directory".to_owned(),
            )
        })?;
        fs::create_dir_all(parent_dir)?;
        let file = File::create(&self.path)?;
        let mut writer = BufWriter::new(file);

        writer.write_all(
            format!("{}{}\n", STORAGE_VERSION_PREFIX, STORAGE_VERSION)
                .as_bytes(),
        )?;

        let value_map = self.data.clone();
        let value_data = serde_json::to_string(&value_map)?;
        writer.write_all(value_data.as_bytes())?;

        let new_timestamp = fs::metadata(&self.path)?.modified()?;
        if new_timestamp == self.timestamp {
            return Err("Timestamp didn't update".into());
        }
        self.timestamp = new_timestamp;

        log::info!(
            "{} {} entries have been written",
            self.label,
            value_map.len()
        );
        Ok(())
    }
  
      fn erase(&self) -> Result<()> {
        fs::remove_file(&self.path).map_err(|err| {
            ArklibError::Storage(self.label.clone(), err.to_string())
        })
    }
}

impl<K, V> AsRef<BTreeMap<K, V>> for FileStorage<K, V> {
    fn as_ref(&self) -> &BTreeMap<K, V> {
        &self.data
    }
}

impl<K, V> Monoid<V> for FileStorage<K, V>
where
    V: Clone + std::cmp::PartialOrd + Default,
{
    fn neutral() -> V {
        V::default()
    }

    fn combine(a: &V, b: &V) -> V {
        if a > b {
            a.clone()
        } else {
            b.clone()
        }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use tempdir::TempDir;

    use crate::{base_storage::BaseStorage, file_storage::FileStorage};

    #[test]
    fn test_file_storage_write_read() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let storage_path = temp_dir.path().join("test_storage.txt");

        let mut file_storage =
            FileStorage::new("TestStorage".to_string(), &storage_path);

        file_storage.set("key1".to_string(), "value1".to_string());
        file_storage.set("key2".to_string(), "value2".to_string());

        assert!(file_storage.remove(&"key1".to_string()).is_ok());
        let data_read: BTreeMap<_, _> = file_storage
            .read_fs()
            .expect("Failed to read data from disk");

        assert_eq!(data_read.len(), 1);
        assert_eq!(data_read.get("key2").map(|v| v.as_str()), Some("value2"))
    }

    #[test]
    fn test_file_storage_auto_delete() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let storage_path = temp_dir.path().join("test_storage.txt");

        let mut file_storage =
            FileStorage::new("TestStorage".to_string(), &storage_path);

        file_storage.set("key1".to_string(), "value1".to_string());
        file_storage.set("key1".to_string(), "value2".to_string());

        assert_eq!(storage_path.exists(), true);

        if let Err(err) = file_storage.erase() {
            panic!("Failed to delete file: {:?}", err);
        }
        assert_eq!(storage_path.exists(), false);
    }

    #[test]
    fn test_file_storage_is_storage_updated() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let storage_path = temp_dir.path().join("teststorage.txt");

        let mut file_storage =
            FileStorage::new("TestStorage".to_string(), &storage_path);

        file_storage.set("key1".to_string(), "value1".to_string());
        assert_eq!(file_storage.is_storage_updated().unwrap(), false);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // External data manipulation
        let mut mirror_storage =
            FileStorage::new("TestStorage".to_string(), &storage_path);

        mirror_storage.set("key1".to_string(), "value3".to_string());
        assert_eq!(mirror_storage.is_storage_updated().unwrap(), false);

        assert_eq!(file_storage.is_storage_updated().unwrap(), true);
    }
}
