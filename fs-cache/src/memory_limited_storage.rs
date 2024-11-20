use std::fs::{self, File};
use std::io::Write;
use std::time::SystemTime;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use data_error::{ArklibError, Result};
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
    V: Clone + serde::Serialize + serde::de::DeserializeOwned,
{
    pub fn new(
        label: String,
        path: &Path,
        max_memory_items: usize,
    ) -> Result<Self> {
        let mut storage = Self {
            label,
            path: PathBuf::from(path),
            memory_cache: LinkedHashMap::with_capacity(max_memory_items),
            max_memory_items,
            disk_timestamps: BTreeMap::new(),
        };

        storage.load_fs()?;

        Ok(storage)
    }

    pub fn label(&self) -> String {
        self.label.clone()
    }

    pub fn load_fs(&mut self) -> Result<()> {
        if !self.path.exists() {
            return Err(ArklibError::Storage(
                self.label.clone(),
                "Folder does not exist".to_owned(),
            ));
        }

        if !self.path.is_dir() {
            return Err(ArklibError::Storage(
                self.label.clone(),
                "Path is not a directory".to_owned(),
            ));
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

        // TODO: WHY?: Later used in sync-status to detect is it recently externally modified or not
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
        if file_path.exists() {
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

        Ok(())
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
