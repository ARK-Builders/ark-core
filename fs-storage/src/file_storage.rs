use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::time::SystemTime;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::base_storage::BaseStorage;
use crate::monoid::Monoid;
use crate::utils::read_version_2_fs;
use data_error::{ArklibError, Result};

/*
Note on `FileStorage` Versioning:

`FileStorage` is a basic key-value storage system that persists data to disk.

In version 2, `FileStorage` stored data in a plaintext format.
Starting from version 3, data is stored in JSON format.

For backward compatibility, we provide a helper function `read_version_2_fs` to read version 2 format.
*/
const STORAGE_VERSION: i32 = 3;

/// Represents a file storage system that persists data to disk.
pub struct FileStorage<K, V>
where
    K: Ord,
{
    label: String,
    path: PathBuf,
    modified: SystemTime,
    data: FileStorageData<K, V>,
}

/// A struct that represents the data stored in a [`FileStorage`] instance.
///
///
/// This is the data that is serialized and deserialized to and from disk.
#[derive(Serialize, Deserialize)]
pub struct FileStorageData<K, V>
where
    K: Ord,
{
    version: i32,
    entries: BTreeMap<K, V>,
}

impl<K, V> FileStorage<K, V>
where
    K: Ord
        + Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr
        + Monoid<V>,
{
    /// Create a new file storage with a diagnostic label and file path
    pub fn new(label: String, path: &Path) -> Self {
        Self {
            label,
            path: PathBuf::from(path),
            modified: SystemTime::now(),
            data: FileStorageData {
                version: STORAGE_VERSION,
                entries: BTreeMap::new(),
            },
        }
    }
}

impl<K, V> BaseStorage<K, V> for FileStorage<K, V>
where
    K: Ord
        + Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr
        + Monoid<V>,
{
    /// Set a key-value pair in the storage
    fn set(&mut self, key: K, value: V) {
        self.data.entries.insert(key, value);
        self.modified = std::time::SystemTime::now();
    }

    /// Remove a key-value pair from the storage given a key
    fn remove(&mut self, id: &K) -> Result<()> {
        self.data.entries.remove(id).ok_or_else(|| {
            ArklibError::Storage(self.label.clone(), "Key not found".to_owned())
        })?;
        self.modified = std::time::SystemTime::now();
        self.write_fs()
            .expect("Failed to remove data from disk");
        Ok(())
    }

    /// Compare the timestamp of the storage file
    /// with the timestamp of the in-memory storage update
    /// to determine if either of the two requires syncing.
    fn needs_syncing(&self) -> Result<bool> {
        match fs::metadata(&self.path) {
            Ok(metadata) => {
                let get_duration_since_epoch = |time: SystemTime| {
                    time.duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                };

                let fs_modified =
                    get_duration_since_epoch(metadata.modified()?);
                let self_modified = get_duration_since_epoch(self.modified);

                Ok(fs_modified != self_modified)
            }
            Err(e) => {
                Err(ArklibError::Storage(self.label.clone(), e.to_string()))
            }
        }
    }

    /// Read the data from the storage file
    fn read_fs(&mut self) -> Result<BTreeMap<K, V>> {
        if !self.path.exists() {
            return Err(ArklibError::Storage(
                self.label.clone(),
                "File does not exist".to_owned(),
            ));
        }

        // First check if the file starts with "version: 2"
        let file_content = std::fs::read_to_string(&self.path)?;
        if file_content.starts_with("version: 2") {
            // Attempt to parse the file using the legacy version 2 storage format of FileStorage.
            match read_version_2_fs(&self.path) {
                Ok(data) => {
                    log::info!(
                        "Version 2 storage format detected for {}",
                        self.label
                    );
                    self.modified = fs::metadata(&self.path)?.modified()?;
                    return Ok(data);
                }
                Err(_) => {
                    return Err(ArklibError::Storage(
                        self.label.clone(),
                        "Storage seems to be version 2, but failed to parse"
                            .to_owned(),
                    ));
                }
            };
        }

        let file = fs::File::open(&self.path)?;
        let data: FileStorageData<K, V> = serde_json::from_reader(file)
            .map_err(|err| {
                ArklibError::Storage(self.label.clone(), err.to_string())
            })?;
        let version = data.version;
        if version != STORAGE_VERSION {
            return Err(ArklibError::Storage(
                self.label.clone(),
                format!(
                    "Storage version mismatch: expected {}, got {}",
                    STORAGE_VERSION, version
                ),
            ));
        }
        self.modified = fs::metadata(&self.path)?.modified()?;

        Ok(data.entries)
    }

    /// Write the data to the storage file
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
        let value_data = serde_json::to_string_pretty(&self.data)?;
        writer.write_all(value_data.as_bytes())?;

        let new_timestamp = fs::metadata(&self.path)?.modified()?;
        if new_timestamp == self.modified {
            return Err("Timestamp has not been updated".into());
        }
        self.modified = new_timestamp;

        log::info!(
            "{} {} entries have been written",
            self.label,
            self.data.entries.len()
        );
        Ok(())
    }

    /// Erase the storage file from disk
    fn erase(&self) -> Result<()> {
        fs::remove_file(&self.path).map_err(|err| {
            ArklibError::Storage(self.label.clone(), err.to_string())
        })
    }

    /// Merge the data from another storage instance into this storage instance
    fn merge_from(&mut self, other: impl AsRef<BTreeMap<K, V>>) -> Result<()>
    where
        V: Monoid<V>,
    {
        let other_entries = other.as_ref();
        for (key, value) in other_entries {
            if let Some(existing_value) = self.data.entries.get(key) {
                let resolved_value = V::combine(existing_value, value);
                self.set(key.clone(), resolved_value);
            } else {
                self.set(key.clone(), value.clone())
            }
        }
        self.modified = std::time::SystemTime::now();
        Ok(())
    }
}

impl<K, V> AsRef<BTreeMap<K, V>> for FileStorage<K, V>
where
    K: Ord,
{
    fn as_ref(&self) -> &BTreeMap<K, V> {
        &self.data.entries
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
        assert!(file_storage.write_fs().is_ok());
        assert_eq!(storage_path.exists(), true);

        if let Err(err) = file_storage.erase() {
            panic!("Failed to delete file: {:?}", err);
        }
        assert!(!storage_path.exists());
    }

    #[test]
    fn test_file_storage_is_storage_updated() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let storage_path = temp_dir.path().join("teststorage.txt");

        let mut file_storage =
            FileStorage::new("TestStorage".to_string(), &storage_path);
        file_storage.write_fs().unwrap();
        assert_eq!(file_storage.needs_syncing().unwrap(), false);
        std::thread::sleep(std::time::Duration::from_secs(1));
        file_storage.set("key1".to_string(), "value1".to_string());
        assert_eq!(file_storage.needs_syncing().unwrap(), true);
        file_storage.write_fs().unwrap();
        assert_eq!(file_storage.needs_syncing().unwrap(), false);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // External data manipulation
        let mut mirror_storage =
            FileStorage::new("TestStorage".to_string(), &storage_path);
        assert_eq!(mirror_storage.needs_syncing().unwrap(), true);
        std::thread::sleep(std::time::Duration::from_secs(1));
        mirror_storage.read_fs().unwrap();
        assert_eq!(mirror_storage.needs_syncing().unwrap(), false);

        mirror_storage.set("key1".to_string(), "value3".to_string());
        assert_eq!(mirror_storage.needs_syncing().unwrap(), true);
        mirror_storage.write_fs().unwrap();
        assert_eq!(mirror_storage.needs_syncing().unwrap(), false);

        assert_eq!(file_storage.needs_syncing().unwrap(), true);
        file_storage.read_fs().unwrap();
        assert_eq!(file_storage.needs_syncing().unwrap(), false);
        assert_eq!(mirror_storage.needs_syncing().unwrap(), false);
    }

    #[test]
    fn test_monoid_combine() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let storage_path1 = temp_dir.path().join("teststorage1.txt");
        let storage_path2 = temp_dir.path().join("teststorage2.txt");

        let mut file_storage_1 =
            FileStorage::new("TestStorage1".to_string(), &storage_path1);

        let mut file_storage_2 =
            FileStorage::new("TestStorage2".to_string(), &storage_path2);

        file_storage_1.set("key1".to_string(), 2);
        file_storage_1.set("key2".to_string(), 6);

        file_storage_2.set("key1".to_string(), 3);
        file_storage_2.set("key3".to_string(), 9);

        file_storage_1
            .merge_from(&file_storage_2)
            .unwrap();
        assert_eq!(file_storage_1.as_ref().get("key1"), Some(&3));
        assert_eq!(file_storage_1.as_ref().get("key2"), Some(&6));
        assert_eq!(file_storage_1.as_ref().get("key3"), Some(&9));
    }
}
