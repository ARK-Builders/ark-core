use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write;
use std::time::SystemTime;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use data_error::{ArklibError, Result};
use fs_storage::base_storage::SyncStatus;
use fs_storage::monoid::Monoid;
use linked_hash_map::LinkedHashMap;

pub struct MemoryLimitedStorage<K, V> {
    /// Label for logging
    label: String,
    /// Path to the underlying folder where data is persisted
    path: PathBuf,
    /// In-memory LRU cache combining map and queue functionality
    memory_cache: LinkedHashMap<K, V>,
    /// Maximum number of items to keep in memory
    max_memory_items: usize,
    /// Track disk timestamps only
    disk_timestamps: BTreeMap<K, SystemTime>,
    /// Temporary store for deleted keys until storage is synced
    deleted_keys: BTreeSet<K>,
}

impl<K, V> MemoryLimitedStorage<K, V>
where
    K: Ord
        + Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::fmt::Display
        + std::hash::Hash
        + std::str::FromStr,
    V: Clone + serde::Serialize + serde::de::DeserializeOwned + Monoid<V>,
{
    pub fn new(
        label: String,
        path: &Path,
        max_memory_items: usize,
    ) -> Result<Self> {
        let storage = Self {
            label,
            path: PathBuf::from(path),
            memory_cache: LinkedHashMap::with_capacity(max_memory_items),
            max_memory_items,
            disk_timestamps: BTreeMap::new(),
            deleted_keys: BTreeSet::new(),
        };

        // TODO: add load_fs;

        // Create directory if it doesn't exist
        fs::create_dir_all(&storage.path)?;

        Ok(storage)
    }

    pub fn load_fs(&mut self) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }

        // Collect all files with their timestamps
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .map_or(false, |ext| ext == "json")
            {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        let key: K =
                            extract_key_from_file_path(&self.label, &path)?;
                        entries.push((key, modified, path));
                    }
                }
            }
        }

        // Sort by timestamp, newest first
        entries.sort_by(|a, b| b.1.cmp(&a.1));

        // Clear current cache and timestamps
        self.memory_cache.clear();
        self.disk_timestamps.clear();

        // Load only up to max_memory_items, newest first
        for (key, timestamp, path) in entries.iter().take(self.max_memory_items)
        {
            match File::open(path) {
                Ok(file) => {
                    if let Ok(value) = serde_json::from_reader(file) {
                        self.memory_cache.insert(key.clone(), value);
                    }
                }
                Err(err) => {
                    log::warn!("Failed to read file for key {}: {}", key, err);
                    continue;
                }
            }
            self.disk_timestamps
                .insert(key.clone(), *timestamp);
        }

        // Add remaining timestamps to disk_timestamps without loading values
        for (key, timestamp, _) in entries.iter().skip(self.max_memory_items) {
            self.disk_timestamps
                .insert(key.clone(), *timestamp);
        }

        log::info!(
            "{} loaded {} items in memory, {} total on disk",
            self.label,
            self.memory_cache.len(),
            self.disk_timestamps.len()
        );

        Ok(())
    }

    // Write a single value to disk
    fn write_value_to_disk(&mut self, key: &K, value: &V) -> Result<()> {
        let file_path = self.path.join(format!("{}.json", key));
        let mut file = File::create(&file_path)?;
        file.write_all(serde_json::to_string_pretty(&value)?.as_bytes())?;
        file.flush()?;

        let new_timestamp = SystemTime::now();
        file.set_modified(new_timestamp)?;
        file.sync_all()?;

        self.disk_timestamps
            .insert(key.clone(), new_timestamp);

        Ok(())
    }

    // Load a single value from disk
    fn load_value_from_disk(&self, key: &K) -> Result<V> {
        let file_path = self.path.join(format!("{}.json", key));
        let file = File::open(&file_path)?;
        let value: V = serde_json::from_reader(file).map_err(|err| {
            ArklibError::Storage(
                self.label.clone(),
                format!("Failed to read value for key {}: {}", key, err),
            )
        })?;
        Ok(value)
    }

    pub fn get(&mut self, key: &K) -> Result<Option<V>> {
        // Check memory cache first - will update LRU order automatically
        if let Some(value) = self.memory_cache.get_refresh(key) {
            return Ok(Some(value.clone()));
        }

        // Try to load from disk
        let file_path = self.path.join(format!("{}.json", key));
        if file_path.exists() && !self.deleted_keys.contains(key) {
            let value = self.load_value_from_disk(key)?;
            self.add_to_memory_cache(key.clone(), value.clone());
            return Ok(Some(value));
        }

        Ok(None)
    }

    fn add_to_memory_cache(&mut self, key: K, value: V) {
        // If at capacity, LinkedHashMap will remove oldest entry automatically
        if self.memory_cache.len() >= self.max_memory_items {
            self.memory_cache.pop_front(); // Removes least recently used
        }
        self.memory_cache.insert(key, value);
    }

    pub fn set(&mut self, key: K, value: V) -> Result<()> {
        // Always write to disk first
        self.write_value_to_disk(&key, &value)?;

        // Then update memory cache
        self.add_to_memory_cache(key.clone(), value);
        self.deleted_keys.remove(&key);

        Ok(())
    }

    pub fn sync(&mut self) -> Result<()> {
        // Since we write through on set(), we only need to handle removals
        for key in self.deleted_keys.iter() {
            let file_path = self.path.join(format!("{}.json", key));
            if file_path.exists() {
                fs::remove_file(&file_path)?;
                self.disk_timestamps.remove(key);
            }
        }

        // Also add latest externally added cache in memory, by comparing timestamps
        // for entry in fs::read_dir(&self.path)? {
        //     let entry = entry?;
        //     let path = entry.path();
        //     if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
        //         let key = extract_key_from_file_path(&self.label, &path)?;

        //         // Only handle completely new keys
        //         if !self.memory_cache.contains_key(&key)
        //            && !self.disk_timestamps.contains_key(&key)
        //            && !self.deleted_keys.contains(&key) {

        //             if let Ok(metadata) = fs::metadata(&path) {
        //                 if let Ok(modified) = metadata.modified() {
        //                     // New key found - load it
        //                     if let Ok(value) = self.load_value_from_disk(&key) {
        //                         self.disk_timestamps.insert(key.clone(), modified);

        //                         // Only add to memory if we have space or it's newer than oldest
        //                         if self.memory_cache.len() < self.max_memory_items {
        //                             self.add_to_memory_cache(key, value);
        //                         }
        //                         // Could add logic here to compare with oldest memory item
        //                     }
        //                 }
        //             }
        //         }
        //     }
        // }

        self.deleted_keys.clear();
        Ok(())
    }

    pub fn sync_status(&self) -> Result<SyncStatus> {
        // Since we write-through on set(), the only thing that can make storage stale
        // is pending deletions
        let ram_newer = !self.deleted_keys.is_empty();

        // Check for new files on disk that we don't know about
        let mut disk_newer = false;
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .map_or(false, |ext| ext == "json")
            {
                let key = extract_key_from_file_path(&self.label, &path)?;
                if !self.memory_cache.contains_key(&key)
                    && !self.disk_timestamps.contains_key(&key)
                    && !self.deleted_keys.contains(&key)
                {
                    disk_newer = true;
                    break;
                }
            }
        }

        Ok(match (ram_newer, disk_newer) {
            (false, false) => SyncStatus::InSync,
            (true, false) => SyncStatus::StorageStale,
            (false, true) => SyncStatus::MappingStale,
            (true, true) => SyncStatus::Diverge,
        })
    }
}

fn extract_key_from_file_path<K>(label: &str, path: &Path) -> Result<K>
where
    K: std::str::FromStr,
{
    path.file_stem()
        .ok_or_else(|| {
            ArklibError::Storage(
                label.to_owned(),
                "Failed to extract file stem from filename".to_owned(),
            )
        })?
        .to_str()
        .ok_or_else(|| {
            ArklibError::Storage(
                label.to_owned(),
                "Failed to convert file stem to string".to_owned(),
            )
        })?
        .parse::<K>()
        .map_err(|_| {
            ArklibError::Storage(
                label.to_owned(),
                "Failed to parse key from filename".to_owned(),
            )
        })
}
