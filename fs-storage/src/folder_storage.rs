use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::time::SystemTime;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::base_storage::{BaseStorage, SyncStatus};
use crate::monoid::Monoid;
// use crate::utils::read_version_2_fs;
use data_error::{ArklibError, Result};

/*
Note on `FolderStorage` Versioning:

`FolderStorage` is a basic key-value storage system that persists data to disk.

In version 2, `FolderStorage` stored data in a plaintext format.
Starting from version 3, data is stored in JSON format.

For backward compatibility, we provide a helper function `read_version_2_fs` to read version 2 format.
*/
const STORAGE_VERSION: i32 = 3;

/// Represents a file storage system that persists data to disk.
pub struct FolderStorage<K, V>
where
    K: Ord,
{
    /// Label for logging
    label: String,
    /// Path to the underlying file where data is persisted
    path: PathBuf,
    /// Last modified time of internal mapping. This becomes equal to
    /// `written_to_disk` only when data is written or read from disk.
    // modified: SystemTime,
    ram_timestamps: BTreeMap<K, SystemTime>,
    /// Last time the data was written to disk. This becomes equal to
    /// `modified` only when data is written or read from disk.
    // written_to_disk: SystemTime,
    disk_timestamps: BTreeMap<K, SystemTime>,
    data: FolderStorageData<K, V>,
}

/// A struct that represents the data stored in a [`FolderStorage`] instance.
///
///
/// This is the data that is serialized and deserialized to and from disk.
#[derive(Serialize, Deserialize)]
pub struct FolderStorageData<K, V>
where
    K: Ord,
{
    version: i32,
    entries: BTreeMap<K, V>,
}

impl<K, V> AsRef<BTreeMap<K, V>> for FolderStorageData<K, V>
where
    K: Ord,
{
    fn as_ref(&self) -> &BTreeMap<K, V> {
        &self.entries
    }
}

impl<K, V> FolderStorage<K, V>
where
    K: Ord
        + Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr
        + std::fmt::Display,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr
        + Monoid<V>,
{
    /// Create a new file storage with a diagnostic label and file path
    /// The storage will be initialized using the disk data, if the path exists
    ///
    /// Note: if the file storage already exists, the data will be read from the file
    /// without overwriting it.
    pub fn new(label: String, path: &Path) -> Result<Self> {
        let mut storage = Self {
            label,
            path: PathBuf::from(path),
            ram_timestamps: BTreeMap::new(),
            disk_timestamps: BTreeMap::new(),
            data: FolderStorageData {
                version: STORAGE_VERSION,
                entries: BTreeMap::new(),
            },
        };

        if Path::exists(path) {
            storage.read_fs()?;
        }

        Ok(storage)
    }

    /// Load mapping from file
    fn load_fs_data(&self) -> Result<FolderStorageData<K, V>> {
        if !self.path.exists() {
            return Err(ArklibError::Storage(
                self.label.clone(),
                "File does not exist".to_owned(),
            ));
        }

        if !self.path.is_dir() {
            return Err(ArklibError::Storage(
                self.label.clone(),
                "Path is not a directory".to_owned(),
            ));
        }

        let mut data = FolderStorageData {
            version: STORAGE_VERSION,
            entries: BTreeMap::new(),
        };

        // read_version_2_fs : unimplemented!()

        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path.extension().map_or(false, |ext| ext == "bin")
            {
                let key = path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .parse::<K>()
                    .map_err(|_| {
                        ArklibError::Storage(
                            self.label.clone(),
                            "Failed to parse key from filename".to_owned(),
                        )
                    })?;

                let mut file = File::open(&path)?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;

                let value: V = bincode::deserialize(&buffer).map_err(|e| {
                    ArklibError::Storage(
                        self.label.clone(),
                        format!("Failed to deserialize value: {}", e),
                    )
                })?;
                data.entries.insert(key, value);
            }
        }
        Ok(data)
    }
}

impl<K, V> BaseStorage<K, V> for FolderStorage<K, V>
where
    K: Ord
        + Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr
        + std::fmt::Display,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr
        + Monoid<V>,
{
    /// Set a key-value pair in the internal mapping
    fn set(&mut self, key: K, value: V) {
        self.data.entries.insert(key.clone(), value);
        self.ram_timestamps.insert(key, SystemTime::now());
    }

    /// Remove an entry from the internal mapping given a key
    fn remove(&mut self, id: &K) -> Result<()> {
        self.data.entries.remove(id).ok_or_else(|| {
            ArklibError::Storage(self.label.clone(), "Key not found".to_owned())
        })?;
        // self.ram_timestamps.remove(id);
        // OR
        self.ram_timestamps
            .insert(id.clone(), SystemTime::now());
        Ok(())
    }

    /// Compare the timestamp of the storage file
    /// with the timestamp of the in-memory storage and the last written
    /// to time to determine if either of the two requires syncing.
    fn sync_status(&self) -> Result<SyncStatus> {
        unimplemented!()
    }

    /// Sync the in-memory storage with the storage on disk
    fn sync(&mut self) -> Result<()> {
        match self.sync_status()? {
            SyncStatus::InSync => Ok(()),
            SyncStatus::MappingStale => self.read_fs().map(|_| ()),
            SyncStatus::StorageStale => self.write_fs().map(|_| ()),
            SyncStatus::Diverge => {
                let data = self.load_fs_data()?;
                self.merge_from(&data)?;
                self.write_fs()?;
                Ok(())
            }
        }
    }

    /// Read the data from file
    fn read_fs(&mut self) -> Result<&BTreeMap<K, V>> {
        let data = self.load_fs_data()?;
        self.data = data;
        self.disk_timestamps.clear();
        for key in self.data.entries.keys() {
            let file_path = self.path.join(format!("{}.bin", key));
            if let Ok(metadata) = fs::metadata(&file_path) {
                if let Ok(modified) = metadata.modified() {
                    self.disk_timestamps.insert(key.clone(), modified);
                }
            }
        }
        Ok(&self.data.entries)
    }

    /// Get a value from the internal mapping
    fn get(&self, id: &K) -> Option<&V> {
        self.data.entries.get(id)
    }

    /// Write the data to file
    ///
    /// Update the modified timestamp in file metadata to avoid OS timing issues
    /// https://github.com/ARK-Builders/ark-rust/pull/63#issuecomment-2163882227
    fn write_fs(&mut self) -> Result<()> {
        fs::create_dir_all(&self.path)?;

        for (key, value) in &self.data.entries {
            let file_path = self.path.join(format!("{}.bin", key));
            let encoded: Vec<u8> = bincode::serialize(value).map_err(|e| {
                ArklibError::Storage(
                    self.label.clone(),
                    format!("Failed to serialize value: {}", e),
                )
            })?;

            let mut file = File::create(&file_path)?;
            file.write_all(&encoded)?;
            file.flush()?;

            let new_timestamp = SystemTime::now();
            file.set_modified(new_timestamp)?;
            file.sync_all()?;

            self.disk_timestamps
                .insert(key.clone(), new_timestamp);
        }

        // Remove files for keys that no longer exist
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path.extension().map_or(false, |ext| ext == "bin")
            {
                let key = path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .parse::<K>()
                    .map_err(|_| {
                        ArklibError::Storage(
                            self.label.clone(),
                            "Failed to parse key from filename".to_owned(),
                        )
                    })?;
                if !self.data.entries.contains_key(&key) {
                    fs::remove_file(path)?;
                }
            }
        }

        log::info!(
            "{} {} entries have been written",
            self.label,
            self.data.entries.len()
        );
        Ok(())
    }

    /// Erase the file from disk
    fn erase(&self) -> Result<()> {
        fs::remove_dir(&self.path).map_err(|err| {
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
            self.ram_timestamps
                .insert(key.clone(), SystemTime::now());
        }
        Ok(())
    }
}

impl<K, V> AsRef<BTreeMap<K, V>> for FolderStorage<K, V>
where
    K: Ord,
{
    fn as_ref(&self) -> &BTreeMap<K, V> {
        &self.data.entries
    }
}
