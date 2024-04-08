use std::fmt::Debug;
use std::fs::{self, File};
use std::hash::Hash;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::str::FromStr;
use std::time::SystemTime;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::base_storage::BaseStorage;
use data_error::{ArklibError, Result};

const STORAGE_VERSION: i32 = 2;
const STORAGE_VERSION_PREFIX: &str = "version ";

pub struct FileStorage<K, V>
where
    K: serde::Serialize,
    V: serde::Serialize,
{
    label: String,
    path: PathBuf,
    timestamp: SystemTime,
    value_by_id: BTreeMap<K, V>,
}

impl<K, V> FileStorage<K, V>
where
    K: serde::Serialize,
    V: serde::Serialize,
{
    /// Create a new file storage with a diagnostic label and file path
    pub fn new(label: String, path: &Path) -> Self {
        Self {
            label,
            path: PathBuf::from(path),
            timestamp: SystemTime::now(),
            value_by_id: BTreeMap::new(),
        }
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
    K: FromStr
        + Hash
        + Eq
        + Ord
        + Debug
        + Clone
        + serde::Serialize
        + serde::de::DeserializeOwned,
    V: Debug + Clone + serde::Serialize + serde::de::DeserializeOwned,
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

    fn is_file_updated(&self) -> Result<bool> {
        let file_timestamp = fs::metadata(&self.path)?.modified()?;
        Ok(self.timestamp < file_timestamp)
    }

    fn read_file(&mut self) -> Result<BTreeMap<K, V>> {
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
                let value_by_id: BTreeMap<K, V> = serde_json::from_str(&data)?;
                self.value_by_id = value_by_id.clone();

                self.timestamp = new_timestamp;
                Ok(value_by_id)
            }
            None => Err(ArklibError::Storage(
                self.label.clone(),
                "Storage file is missing header".to_owned(),
            )),
        }
    }

    fn write_file(&mut self) -> Result<()> {
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

        let value_map = self.value_by_id.clone();
        let data = serde_json::to_string(&value_map)?;
        writer.write_all(data.as_bytes())?;

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

    /// Remove file at stored path
    fn erase(&mut self) -> Result<()> {
        fs::remove_file(&self.path).map_err(|err| {
            ArklibError::Storage(self.label.clone(), err.to_string())
        })
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

        let mut data_to_write = BTreeMap::new();
        data_to_write.insert("key1".to_string(), "value1".to_string());
        data_to_write.insert("key2".to_string(), "value2".to_string());    

        // file_storage.set("key1".to_string(), "value1".to_string());
        // file_storage.set("key2".to_string(), "value2".to_string());
        file_storage.value_by_id = data_to_write.clone();

        file_storage
            .write_file()
            .expect("Failed to write data to disk");

        let data_read: BTreeMap<_, _> = file_storage
            .read_file()
            .expect("Failed to read data from disk");

        assert_eq!(data_read, data_to_write);
    }

    #[test]
    fn test_file_storage_auto_delete() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let storage_path = temp_dir.path().join("test_storage.txt");

        let mut file_storage =
            FileStorage::new("TestStorage".to_string(), &storage_path);

        let mut data_to_write = BTreeMap::new();
        data_to_write.insert("key1".to_string(), "value1".to_string());
        data_to_write.insert("key2".to_string(), "value2".to_string());
        
        file_storage.value_by_id = data_to_write.clone();

        file_storage
            .write_file()
            .expect("Failed to write data to disk");

        assert_eq!(storage_path.exists(), true);

        if let Err(err) = file_storage.erase() {
            panic!("Failed to delete file: {:?}", err);
        }
        assert_eq!(storage_path.exists(), false);
    }
}
