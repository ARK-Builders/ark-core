use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use data_error::{ArklibError, Result};
use linked_hash_map::LinkedHashMap;

pub struct MemoryLimitedStorage<K, V> {
    /// Label for logging
    label: String,
    /// Path to the underlying folder where data is persisted
    path: PathBuf,
    /// In-memory LRU cache combining map and queue functionality
    memory_cache: LinkedHashMap<K, V>,
    // Bytes present in memory
    current_memory_bytes: usize,
    /// Maximum bytes to keep in memory
    max_memory_bytes: usize,
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
        max_memory_bytes: usize,
    ) -> Result<Self> {
        let mut storage = Self {
            label,
            path: PathBuf::from(path),
            memory_cache: LinkedHashMap::new(),
            current_memory_bytes: 0,
            max_memory_bytes,
        };

        storage.load_fs()?;

        Ok(storage)
    }

    pub fn label(&self) -> String {
        self.label.clone()
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        // Check memory cache first - will update LRU order automatically
        if let Some(value) = self.memory_cache.get_refresh(key) {
            return Some(value.clone());
        }

        // Try to load from disk
        let file_path = self.path.join(format!("{}.json", key));
        if file_path.exists() {
            // Doubt: Update file's modiied time (in disk) on read to preserve LRU across app restarts?
            match self.load_value_from_disk(key) {
                Ok(value) => {
                    self.add_to_memory_cache(key.clone(), value.clone());
                    Some(value)
                }
                Err(err) => {
                    log::error!(
                        "{} cache: failed to load key={}: {}",
                        self.label,
                        key,
                        err
                    );
                    None
                }
            }
        } else {
            None
        }
    }

    pub fn set(&mut self, key: K, value: V) -> Result<()> {
        // Always write to disk first
        self.write_value_to_disk(&key, &value)?;

        // Then update memory cache
        self.add_to_memory_cache(key, value);

        Ok(())
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

        // First pass: collect metadata only
        let mut file_metadata = Vec::new();
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .map_or(false, |ext| ext == "json")
            {
                if let Ok(metadata) = fs::metadata(&path) {
                    let key = extract_key_from_file_path(&self.label, &path)?;
                    file_metadata.push((key, metadata.len() as usize));
                }
            }
        }

        // Sort by timestamp (newest first) before loading any values
        file_metadata.sort_by(|a, b| b.1.cmp(&a.1));

        // Clear existing cache
        self.memory_cache.clear();
        self.current_memory_bytes = 0;

        // TODO: Need some work here
        // Second pass: load only the values that will fit in memory
        let mut loaded_bytes = 0;
        let mut total_bytes = 0;

        for (key, approx_size) in file_metadata {
            total_bytes += approx_size;

            // Only load value if it will likely fit in memory
            if loaded_bytes + approx_size <= self.max_memory_bytes {
                match self.load_value_from_disk(&key) {
                    Ok(value) => {
                        let actual_size = Self::estimate_size(&value);
                        if loaded_bytes + actual_size <= self.max_memory_bytes {
                            self.memory_cache.insert(key, value);
                            loaded_bytes += actual_size;
                        }
                    }
                    Err(err) => {
                        log::warn!(
                            "{} cache: failed to load key={}: {}",
                            self.label,
                            key,
                            err
                        );
                    }
                }
            }
        }

        self.current_memory_bytes = loaded_bytes;

        log::debug!(
            "{} loaded {}/{} bytes in memory",
            self.label,
            self.current_memory_bytes,
            total_bytes
        );

        Ok(())
    }

    fn estimate_size(value: &V) -> usize {
        serde_json::to_vec(value)
            .map(|v| v.len())
            .unwrap_or(0)
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

    fn add_to_memory_cache(&mut self, key: K, value: V) {
        let value_size = Self::estimate_size(&value);

        // If single value is larger than total limit, just skip memory caching
        if value_size > self.max_memory_bytes {
            log::debug!(
                "{} cache: value size {} exceeds limit {}",
                self.label,
                value_size,
                self.max_memory_bytes
            );
            return;
        }

        // Remove oldest entries until we have space for new value
        while self.current_memory_bytes + value_size > self.max_memory_bytes
            && !self.memory_cache.is_empty()
        {
            if let Some((_, old_value)) = self.memory_cache.pop_front() {
                self.current_memory_bytes = self
                    .current_memory_bytes
                    .saturating_sub(Self::estimate_size(&old_value));
            }
        }

        // Add new value and update size
        self.memory_cache.insert(key, value);
        self.current_memory_bytes += value_size;

        log::debug!(
            "{} cache: added {} bytes, total {}/{}",
            self.label,
            value_size,
            self.current_memory_bytes,
            self.max_memory_bytes
        );
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
