use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::time::SystemTime;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::base_storage::{BaseStorage, SyncStatus};
use crate::monoid::Monoid;
use crate::utils::read_version_2_fs;
use data_error::{ArklibError, Result};

/*
Note on `FolderStorage` Versioning:

`FolderStorage` is a basic key-value storage system that persists data to disk.

In version 2, `FolderStorage` stored data in a plaintext format.
Starting from version 3, data is stored in JSON format.

For backward compatibility, we provide a helper function `read_version_2_fs` to read version 2 format.
*/
const STORAGE_VERSION: i32 = 3;
const MAX_ENTRIES_PER_FILE: usize = 1000;

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
    modified: SystemTime,
    /// Last time the data was written to disk. This becomes equal to
    /// `modified` only when data is written or read from disk.
    written_to_disk: SystemTime,
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
        + std::str::FromStr,
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
        let time = SystemTime::now();
        let mut storage = Self {
            label,
            path: PathBuf::from(path),
            modified: time,
            written_to_disk: time,
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

        let mut data = FolderStorageData {
            version: STORAGE_VERSION,
            entries: BTreeMap::new(),
        };

        let index_path = self.path.join("index.json");
        if index_path.exists() {
            let index_file = File::open(&index_path)?;
            let index: BTreeMap<K, usize> =
                serde_json::from_reader(index_file)?;

            for (_key, file_index) in index {
                let file_path = self
                    .path
                    .join(format!("data_{}.json", file_index));
                if file_path.exists() {
                    // First check if the file starts with "version: 2"
                    let file_content =
                        std::fs::read_to_string(file_path.clone())?;
                    if file_content.starts_with("version: 2") {
                        // Attempt to parse the file using the legacy version 2 storage format of FolderStorage.
                        match read_version_2_fs(&file_path) {
                            Ok(legacy_data) => {
                                log::info!(
                                    "Version 2 storage format detected for {}",
                                    self.label
                                );
                                data.version = 2;
                                data.entries.extend(legacy_data);
                                continue;
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

                    let file = fs::File::open(&file_path)?;
                    let file_data: FolderStorageData<K, V> =
                        serde_json::from_reader(file).map_err(|err| {
                            ArklibError::Storage(
                                self.label.clone(),
                                err.to_string(),
                            )
                        })?;

                    if file_data.version != STORAGE_VERSION {
                        return Err(ArklibError::Storage(
                            self.label.clone(),
                            format!(
                                "Storage version mismatch: expected {}, got {}",
                                STORAGE_VERSION, file_data.version
                            ),
                        ));
                    }

                    data.entries.extend(file_data.entries);
                }
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
        + std::str::FromStr,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr
        + Monoid<V>,
{
    /// Set a key-value pair in the internal mapping
    fn set(&mut self, key: K, value: V) {
        self.data.entries.insert(key, value);
        self.modified = std::time::SystemTime::now();
    }

    /// Remove an entry from the internal mapping given a key
    fn remove(&mut self, id: &K) -> Result<()> {
        self.data.entries.remove(id).ok_or_else(|| {
            ArklibError::Storage(self.label.clone(), "Key not found".to_owned())
        })?;
        self.modified = std::time::SystemTime::now();
        Ok(())
    }

    /// Compare the timestamp of the storage file
    /// with the timestamp of the in-memory storage and the last written
    /// to time to determine if either of the two requires syncing.
    fn sync_status(&self) -> Result<SyncStatus> {
        let file_updated = fs::metadata(&self.path)?.modified()?;

        // Determine the synchronization status based on the modification times
        // Conditions:
        // 1. If both the in-memory storage and the storage on disk have been modified
        //    since the last write, then the storage is diverged.
        // 2. If only the in-memory storage has been modified since the last write,
        //    then the storage on disk is stale.
        // 3. If only the storage on disk has been modified since the last write,
        //    then the in-memory storage is stale.
        // 4. If neither the in-memory storage nor the storage on disk has been modified
        //    since the last write, then the storage is in sync.
        let status = match (
            self.modified > self.written_to_disk,
            file_updated > self.written_to_disk,
        ) {
            (true, true) => SyncStatus::Diverge,
            (true, false) => SyncStatus::StorageStale,
            (false, true) => SyncStatus::MappingStale,
            (false, false) => SyncStatus::InSync,
        };

        log::info!("{} sync status is {}", self.label, status);
        Ok(status)
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
        self.modified = fs::metadata(&self.path)?.modified()?;
        self.written_to_disk = self.modified;
        self.data = data;

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
        let parent_dir = self.path.parent().ok_or_else(|| {
            ArklibError::Storage(
                self.label.clone(),
                "Failed to get parent directory".to_owned(),
            )
        })?;
        fs::create_dir_all(parent_dir)?;

        let mut current_file_index = 0;
        let mut current_file_entries = 0;

        for (key, value) in &self.data.entries {
            if current_file_entries >= MAX_ENTRIES_PER_FILE {
                current_file_index += 1;
                current_file_entries = 0;
            }

            let file_path = self
                .path
                .join(format!("data_{}.json", current_file_index));
            let mut file_data: BTreeMap<K, V> = if file_path.exists() {
                let file = File::open(&file_path)?;
                serde_json::from_reader(file)?
            } else {
                BTreeMap::new()
            };

            file_data.insert(key.clone(), value.clone());
            current_file_entries += 1;

            let mut file = File::create(&file_path)?;
            file.write_all(
                serde_json::to_string_pretty(&file_data)?.as_bytes(),
            )?;
            file.flush()?;

            let new_timestamp = SystemTime::now();
            file.set_modified(new_timestamp)?;
            file.sync_all()?;
        }

        // Write the index file
        // index stores K -> key, V -> file index in which key value pair is stored
        let index: BTreeMap<K, usize> = self
            .data
            .entries
            .keys()
            .enumerate()
            .map(|(i, k)| (k.clone(), i / MAX_ENTRIES_PER_FILE))
            .collect();
        let index_path = self.path.join("index.json");
        let mut index_file = File::create(index_path)?;
        index_file
            .write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;
        index_file.flush()?;
        index_file.sync_all()?;

        let new_timestamp = SystemTime::now();
        self.modified = new_timestamp;
        self.written_to_disk = new_timestamp;

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
        }
        self.modified = std::time::SystemTime::now();
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
