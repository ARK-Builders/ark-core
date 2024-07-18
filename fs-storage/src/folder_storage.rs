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

const MAX_ENTRIES_PER_FILE: usize = 1000;
const STORAGE_VERSION: i32 = 1;

pub struct FolderStorage<K, V>
where
    K: Ord,
{
    /// Label for logging
    label: String,
    /// Path to the underlying file where data is persisted
    path: PathBuf,
    /// Tracks the last known modification time of each file in memory.
    /// This becomes equal to `last_disk_updated` only when data is written or read from disk.
    disk_timestamps: BTreeMap<K, SystemTime>,
    /// Tracks the last known modification time of each file on disk.
    /// This becomes equal to `last_ram_updated` only when data is written or read from disk.
    ram_timestamps: BTreeMap<K, SystemTime>,
    current_file_index: usize,
    current_file_entries: usize,
    // Maps keys to file indices
    index: BTreeMap<K, usize>,
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
        + std::fmt::Debug,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr
        + Monoid<V>,
{
    pub fn new(label: String, path: &Path) -> Result<Self> {
        let mut storage = Self {
            label,
            path: path.to_path_buf(),
            disk_timestamps: BTreeMap::new(),
            ram_timestamps: BTreeMap::new(),
            current_file_index: 0,
            current_file_entries: 0,
            index: BTreeMap::new(),
            data: FolderStorageData {
                version: STORAGE_VERSION,
                entries: BTreeMap::new(),
            },
        };
        storage.load_index()?;

        if Path::exists(path) {
            storage.read_fs()?;
        }

        Ok(storage)
    }

    fn load_index(&mut self) -> Result<()> {
        let index_path: PathBuf = self.path.join("index.json");
        if index_path.exists() {
            let mut file = File::open(index_path)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            self.index = serde_json::from_str(&contents)?;
            self.current_file_index =
                *self.index.values().max().unwrap_or(&0) + 1; // Correct?
        }
        Ok(())
    }

    fn save_index(&self) -> Result<()> {
        let index_path = self.path.join("index.json");
        let mut file = File::create(index_path)?;
        let contents = serde_json::to_string(&self.index)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }

    fn get_file_path(&self, file_index: usize) -> PathBuf {
        self.path
            .join(format!("data_{}.json", file_index))
    }

    /// Load mapping from file
    fn load_fs_data(&self) -> Result<FolderStorageData<K, V>> {
        if !self.path.exists() {
            return Err(ArklibError::Storage(
                self.label.clone(),
                "File does not exist".to_owned(),
            ));
        }

        for entry in fs::read_dir(self.path.clone())? {
            let path = entry?.path();
            log::info!("Reading value from: {:?}", path);

            if !path.is_file() || path.file_name().unwrap() == "index.json" {
                continue;
            }

            let metadata = fs::metadata(&path)?;
            let new_timestamp = metadata.modified()?;

            let file_index = path
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .split('_')
                .nth(1)
                .unwrap()
                .parse::<usize>()
                .unwrap();

            let mut file = File::open(&path)?;
            let mut contents: String = String::new();
            file.read_to_string(&mut contents)?;

            let data: BTreeMap<K, V> = serde_json::from_str(&contents)?;

        //     for (id, value) in data {
        //         let disk_timestamp = self
        //             .disk_timestamps
        //             .get(&id)
        //             .cloned()
        //             .unwrap_or(SystemTime::UNIX_EPOCH);
        //         if disk_timestamp < new_timestamp {
        //             self.data.entries.insert(id.clone(), value);
        //             self.disk_timestamps.insert(id, new_timestamp);
        //             self.index.insert(id, file_index);
        //         }
        //     }

        //     self.disk_timestamps.extend(new_timestamps);
        //     self.save_index()?;

        //     Ok(new_value_by_id)
        }

        // let file = fs::File::open(&self.path)?;
        // let data: FolderStorageData<K, V> = serde_json::from_reader(file)
        //     .map_err(|err| {
        //         ArklibError::Storage(self.label.clone(), err.to_string())
        //     })?;
        // let version = data.version;
        // if version != STORAGE_VERSION {
        //     return Err(ArklibError::Storage(
        //         self.label.clone(),
        //         format!(
        //             "Storage version mismatch: expected {}, got {}",
        //             STORAGE_VERSION, version
        //         ),
        //     ));
        // }

        Ok(data)
    }

    fn find_changed_ids(&self) -> Vec<K> {
        self.ram_timestamps
            .iter()
            .filter_map(|(id, ram_ft)| {
                let disk_ft = self.disk_timestamps.get(id);
                if disk_ft.is_none() || disk_ft.unwrap() != ram_ft {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl<K, V> BaseStorage<K, V> for FolderStorage<K, V>
where
    K: Ord
        + Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr
        + std::fmt::Debug,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::str::FromStr
        + Monoid<V>,
{
    fn set(&mut self, id: K, value: V) {
        // Perform all this either in init or readFS
        // let file_index = if let Some(&existing_index) = self.index.get(&id) {
        //     existing_index
        // } else if self.current_file_entries >= MAX_ENTRIES_PER_FILE {
        //     self.current_file_index += 1;
        //     self.current_file_entries = 0;
        //     self.current_file_index
        // } else {
        //     self.current_file_index
        // };

        // let file_path = self.get_file_path(file_index);
        // let mut data: BTreeMap<String, V> = if file_path.exists() {
        //     let mut file = File::open(&file_path).unwrap();
        //     let mut contents = String::new();
        //     file.read_to_string(&mut contents).unwrap();
        //     serde_json::from_str(&contents).unwrap()
        // } else {
        //     BTreeMap::new()
        // };

        self.data.entries.insert(id.clone(), value);
        self.ram_timestamps.insert(id, SystemTime::now());

        // data.insert(id.clone(), value);
        // let mut file = File::create(file_path)?;
        // let contents = serde_json::to_string(&data)?;
        // file.write_all(contents.as_bytes())?; // remove this instead add to a self.data

        // self.index.insert(id.clone(), file_index);
        // self.current_file_entries += 1;
        // self.save_index()?;

        // let now = SystemTime::now()
        //     .duration_since(UNIX_EPOCH)
        //     .unwrap()
        //     .as_secs();
        // self.ram_timestamps.insert(
        //     id,
        //     SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(now),
        // );
    }

    fn remove(&mut self, id: &K) -> Result<()> {
        // if let Some(&file_index) = self.index.get(id) {
        //     let file_path = self.get_file_path(file_index);
        //     let mut file = File::open(&file_path)?;
        //     let mut contents = String::new();
        //     file.read_to_string(&mut contents)?;
        //     let mut data: BTreeMap<String, V> =
        //         serde_json::from_str(&contents)?;

        //     data.remove(id);

        //     let mut file = File::create(file_path)?;
        //     let contents = serde_json::to_string(&data)?;
        //     file.write_all(contents.as_bytes())?;

        //     self.index.remove(id);
        //     self.save_index()?;
        //     self.ram_timestamps.remove(id);
        //     self.disk_timestamps.remove(id);
        // }
        self.data.entries.remove(id).ok_or_else(|| {
            ArklibError::Storage(self.label.clone(), "Key not found".to_owned())
        })?;
        self.ram_timestamps.remove(id);
        Ok(())
    }

    /// Compare the timestamp of the storage file
    /// with the timestamp of the in-memory storage and the last written
    /// to time to determine if either of the two requires syncing.
    fn sync_status(&self, id: &K) -> Result<SyncStatus> {
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
            self.ram_timestamps.get(id) > self.disk_timestamps.get(id),
            file_updated > self.disk_timestamps.get(id).unwrap().clone(),
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

    fn read_fs(&mut self) -> Result<&BTreeMap<K, V>> {
        let data = self.load_fs_data()?;
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
        fs::create_dir_all(&self.path)?;
        let changed_values: BTreeMap<K, V> = BTreeMap::new(); // TODO
        for (id, val) in changed_values.clone() {
            let file_index = if let Some(&existing_index) = self.index.get(&id)
            {
                existing_index
            } else if self.current_file_entries >= MAX_ENTRIES_PER_FILE {
                self.current_file_index += 1;
                self.current_file_entries = 0;
                self.current_file_index
            } else {
                self.current_file_index
            };

            let file_path = self.get_file_path(file_index);
            let mut data: BTreeMap<K, V> = if file_path.exists() {
                let mut file = File::open(&file_path).unwrap();
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();
                serde_json::from_str(&contents).unwrap()
            } else {
                BTreeMap::new()
            };

            data.insert(id.clone(), val);

            let mut file = File::create(file_path)?;
            let contents = serde_json::to_string(&data)?;
            file.write_all(contents.as_bytes())?;

            self.index.insert(id.clone(), file_index);
            self.current_file_entries += 1;
            self.save_index()?;

            let now = SystemTime::now();
            self.ram_timestamps.insert(id.clone(), now);
            self.disk_timestamps.insert(id, now);
        }

        log::info!(
            "{} {} entries have been written",
            self.label,
            changed_values.len()
        );

        // let changed_value_by_ids: BTreeMap<_, _> = self
        //     .find_changed_ids()
        //     .into_iter()
        //     .filter_map(|id| value_by_id.get(&id).map(|v| (id, v.clone())))
        //     .collect();

        Ok(())
    }

    /// Erase the file from disk
    /// Implement later
    fn erase(&self) -> Result<()> {
        unimplemented!("erase")
        // fs::remove_file(&self.path).map_err(|err| {
        //     ArklibError::Storage(self.label.clone(), err.to_string())
        // })
    }

    /// Merge the data from another storage instance into this storage instance
    fn merge_from(&mut self, _other: impl AsRef<BTreeMap<K, V>>) -> Result<()>
    where
        V: Monoid<V>,
    {
        unimplemented!("merge_from")
        // let other_entries = other.as_ref();
        // for (key, value) in other_entries {
        //     if let Some(existing_value) = self.data.entries.get(key) {
        //         let resolved_value = V::combine(existing_value, value);
        //         self.set(key.clone(), resolved_value);
        //     } else {
        //         self.set(key.clone(), value.clone())
        //     }
        // }
        // self.modified = std::time::SystemTime::now();
        // Ok(())
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
