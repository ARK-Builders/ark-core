use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use data_error::{ArklibError, Result};
use fs_atomic_light::temp_and_move;
use fs_storage::utils::extract_key_from_file_path;
use lru::LruCache;

/// A cache entry that stores a value and its size in bytes.
///
/// This structure is used to track both the actual data (value)
/// and its memory usage (size) in the cache.
struct CacheEntry<V> {
    value: V,
    size: usize,
}

/// A combined in-memory and disk-based cache system.
///
/// This cache uses an LRU (Least Recently Used) eviction policy for the
/// in-memory portion and persists data to disk for long-term storage.
pub struct Cache<K, V> {
    /// Label for logging
    label: String,
    /// Path to the underlying folder where data is persisted
    path: PathBuf,
    /// An in-memory LRU cache for quick access to frequently used items.
    memory_cache: LruCache<K, CacheEntry<V>>,
    /// The current memory usage in bytes.
    current_memory_bytes: usize,
    /// The maximum allowable memory usage in bytes.
    max_memory_bytes: usize,
}

impl<K, V> Cache<K, V>
where
    K: Ord
        + Clone
        + std::fmt::Display
        + std::hash::Hash
        + std::str::FromStr
        + std::fmt::Debug,
    V: Clone + std::fmt::Debug + AsRef<[u8]> + From<Vec<u8>>,
{
    /// Creates a new cache instance.
    ///
    /// # Arguments
    /// * `label` - Identifier used in logs
    /// * `path` - Directory where cache files are stored
    /// * `max_memory_bytes` - Maximum bytes to keep in memory
    /// * `preload_cache` - Whether to pre-load the cache from disk on initialization
    pub fn new(
        label: String,
        path: &Path,
        max_memory_bytes: usize,
        preload_cache: bool,
    ) -> Result<Self> {
        Self::validate_path(path, &label)?;

        let memory_cache = LruCache::new(
            NonZeroUsize::new(max_memory_bytes)
                .expect("Capacity can't be zero"),
        );

        let mut cache = Self {
            label: label.clone(),
            path: PathBuf::from(path),
            memory_cache,
            current_memory_bytes: 0,
            max_memory_bytes,
        };

        log::debug!(
            "cache/{}: initialized with {} bytes limit",
            label,
            max_memory_bytes
        );

        if preload_cache {
            cache.load_fs()?;
        }
        Ok(cache)
    }

    /// Validates the provided path.
    ///
    /// # Arguments
    /// * `path` - The path to validate
    /// * `label` - Identifier used in logs
    fn validate_path(path: &Path, label: &str) -> Result<()> {
        if !path.exists() {
            return Err(ArklibError::Storage(
                label.to_owned(),
                "Folder does not exist".to_owned(),
            ));
        }

        if !path.is_dir() {
            return Err(ArklibError::Storage(
                label.to_owned(),
                "Path is not a directory".to_owned(),
            ));
        }

        Ok(())
    }

    /// Retrieves a value by its key, checking memory first then disk.
    /// Returns None if the key doesn't exist.
    pub fn get(&mut self, key: &K) -> Option<V> {
        log::debug!("cache/{}: retrieving value for key {}", self.label, key);

        if let Some(v) = self.fetch_from_memory(key) {
            log::debug!(
                "cache/{}: value for key {} retrieved from memory",
                self.label,
                key
            );
            return Some(v);
        }
        if let Some(v) = self.fetch_from_disk(key) {
            log::debug!(
                "cache/{}: value for key {} retrieved from disk",
                self.label,
                key
            );
            return Some(v);
        }

        log::warn!("cache/{}: no value found for key {}", self.label, key);
        None
    }

    /// Stores a new value with the given key.
    /// Returns error if the key already exists or if writing fails.
    pub fn set(&mut self, key: K, value: V) -> Result<()> {
        log::debug!("cache/{}: setting value for key {}", self.label, key);

        // Check if value already exists
        if self.exists(&key) {
            return Err(ArklibError::Storage(
                self.label.clone(),
                format!("Key {} already exists in cache", key),
            ));
        }

        // Always write to disk first
        self.persist_to_disk(&key, &value)?;

        // Then update memory cache
        self.update_memory_cache(&key, &value)?;

        log::debug!("cache/{}: set key={}", self.label, key);
        Ok(())
    }

    /// Checks if a value exists either in memory or on disk.
    pub fn exists(&self, key: &K) -> bool {
        self.memory_cache.contains(key)
            || self.path.join(key.to_string()).exists()
    }

    /// Returns an ordered iterator over all cached keys.
    pub fn keys(&self) -> Result<impl Iterator<Item = K>> {
        let keys: Vec<K> = fs::read_dir(&self.path)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path.is_file() {
                    extract_key_from_file_path(&self.label, &path, true).ok()
                } else {
                    None
                }
            })
            .collect();

        Ok(keys.into_iter())
    }

    /// Internal Methods:
    /// Initializes the memory cache by loading the most recently modified files up to the memory limit.
    ///
    /// First collects metadata for all files, sorts them by modification time, and then loads as many
    /// recent files as possible within the memory limit. Files that don't fit in memory remain only
    /// on disk.
    fn load_fs(&mut self) -> Result<()> {
        // Collect metadata for all files
        let mut file_metadata = Vec::new();
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let key: K =
                    extract_key_from_file_path(&self.label, &path, true)?;
                let metadata = entry.metadata()?;
                let modified = metadata.modified()?;
                let size = metadata.len() as usize;
                file_metadata.push((key, size, modified));
            }
        }

        // Sort by modified time (most recent first)
        file_metadata.sort_by(|a, b| b.2.cmp(&a.2));

        // Clear existing cache
        self.memory_cache.clear();
        self.current_memory_bytes = 0;

        // Load files that fit in memory
        let mut loaded_bytes = 0;
        let total_bytes: usize = file_metadata
            .iter()
            .map(|(_, size, _)| size)
            .sum();

        for (key, size, _) in file_metadata {
            if loaded_bytes + size <= self.max_memory_bytes {
                match self.read_from_disk(&key) {
                    Ok(value) => {
                        self.memory_cache
                            .put(key, CacheEntry { value, size });
                        loaded_bytes += size;
                    }
                    Err(err) => {
                        log::warn!(
                            "cache/{}: failed to load key={}: {}",
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
            "cache/{}: loaded {}/{} bytes in memory",
            self.label,
            self.current_memory_bytes,
            total_bytes
        );

        Ok(())
    }

    /// Retrieves a value from the memory cache.
    fn fetch_from_memory(&mut self, key: &K) -> Option<V> {
        self.memory_cache
            .get(key)
            .map(|entry| entry.value.clone())
    }

    /// Retrieves a value from disk and caches it in memory if possible.
    fn fetch_from_disk(&mut self, key: &K) -> Option<V> {
        let file_path = self.path.join(key.to_string());
        if !file_path.exists() {
            log::warn!("cache/{}: no value found for key {}", self.label, key);
            return None;
        }

        match self.read_from_disk(key) {
            Ok(value) => {
                if let Err(err) = self.update_memory_cache(key, &value) {
                    log::error!(
                    "cache/{}: failed to add to memory cache for key {}: {}", 
                    self.label,
                    key,
                    err
                );
                    return None;
                }
                Some(value)
            }
            Err(err) => {
                log::error!(
                    "cache/{}: failed to load from disk for key {}: {}",
                    self.label,
                    key,
                    err
                );
                None
            }
        }
    }

    /// Writes a value to disk using atomic operations.
    fn persist_to_disk(&mut self, key: &K, value: &V) -> Result<()> {
        log::debug!("cache/{}: writing to disk for key {}", self.label, key);

        if !self.path.exists() {
            return Err(ArklibError::Storage(
                self.label.clone(),
                format!(
                    "Cache directory does not exist: {}",
                    self.path.display()
                ),
            ));
        }

        let file_path = self.path.join(key.to_string());
        debug_assert!(
            !file_path.exists(),
            "File {} should not exist before writing",
            file_path.display()
        );

        temp_and_move(value.as_ref(), self.path.clone(), &key.to_string())
            .map_err(|err| {
                ArklibError::Storage(
                    self.label.clone(),
                    format!("Failed to write value for key {}: {}", key, err),
                )
            })
    }

    /// Reads a value from disk.
    fn read_from_disk(&self, key: &K) -> Result<V>
    where
        V: From<Vec<u8>>, // Add trait bound for reading binary data
    {
        let file_path = self.path.join(key.to_string());
        let contents = fs::read(&file_path)?;
        Ok(V::from(contents))
    }

    /// Returns the size of a value in bytes.
    ///
    /// First checks the memory cache for size information to avoid disk access.
    /// Falls back to checking the file size on disk if not found in memory.
    fn get_file_size(&self, key: &K) -> Result<usize> {
        if let Some(entry) = self.memory_cache.peek(key) {
            return Ok(entry.size);
        }
        Ok(fs::metadata(self.path.join(key.to_string()))?.len() as usize)
    }

    /// Adds or updates a value in the memory cache, evicting old entries if needed.
    /// Logs error if value is larger than maximum memory limit.
    fn update_memory_cache(&mut self, key: &K, value: &V) -> Result<()> {
        log::debug!("cache/{}: caching in memory for key {}", self.label, key);
        let size = self.get_file_size(key)?;

        // If single value is larger than total limit, just skip memory caching
        if size > self.max_memory_bytes {
            log::error!(
                "cache/{}: value size {} exceeds limit {}",
                self.label,
                size,
                self.max_memory_bytes
            );
            return Ok(());
        }

        // Remove oldest entries until we have space for new value
        while self.current_memory_bytes + size > self.max_memory_bytes {
            let (_, old_entry) = self
                .memory_cache
                .pop_lru()
                .expect("Cache should have entries to evict");
            debug_assert!(
                self.current_memory_bytes >= old_entry.size,
                "Memory tracking inconsistency detected"
            );
            self.current_memory_bytes = self
                .current_memory_bytes
                .saturating_sub(old_entry.size);
        }

        // Add new value and update size
        self.memory_cache.put(
            key.clone(),
            CacheEntry {
                value: value.clone(),
                size,
            },
        );
        self.current_memory_bytes += size;

        log::debug!(
            "cache/{}: added {} bytes, total {}/{}",
            self.label,
            size,
            self.current_memory_bytes,
            self.max_memory_bytes
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::File,
        io::Write,
        time::{Duration, SystemTime},
    };
    use tempdir::TempDir;

    /// Helper function to create a temporary directory
    fn create_temp_dir() -> TempDir {
        TempDir::new("tmp").expect("Failed to create temporary directory")
    }

    /// Helper function to create a test cache with default settings
    fn create_test_cache(temp_dir: &TempDir) -> Cache<String, Vec<u8>> {
        Cache::new(
            "test".to_string(),
            temp_dir.path(),
            1024 * 1024, // 1MB
            true,        // Enable preloading by default
        )
        .expect("Failed to create cache")
    }

    #[test]
    fn test_new_cache() {
        let temp_dir = create_temp_dir();
        let cache = create_test_cache(&temp_dir);
        assert_eq!(cache.current_memory_bytes, 0);
        assert_eq!(cache.max_memory_bytes, 1024 * 1024);
    }

    #[test]
    fn test_set_and_get() {
        let temp_dir = create_temp_dir();
        let mut cache = create_test_cache(&temp_dir);
        let key = "test_key".to_string();
        let value = vec![1, 2, 3, 4];

        cache
            .set(key.clone(), value.clone())
            .expect("Failed to set value");
        let retrieved = cache.get(&key).expect("Failed to get value");
        assert_eq!(retrieved, value);
    }

    #[test]
    fn test_exists() {
        let temp_dir = create_temp_dir();
        let mut cache = create_test_cache(&temp_dir);
        let key = "test_key".to_string();
        let value = vec![1, 2, 3, 4];

        assert!(!cache.exists(&key));
        cache
            .set(key.clone(), value)
            .expect("Failed to set value");
        assert!(cache.exists(&key));
    }

    #[test]
    fn test_get_nonexistent() {
        let temp_dir = create_temp_dir();
        let mut cache = create_test_cache(&temp_dir);
        assert!(cache.get(&"nonexistent".to_string()).is_none());
    }

    #[test]
    fn test_keys() {
        let temp_dir = create_temp_dir();
        let mut cache = create_test_cache(&temp_dir);
        let values = vec![
            ("key1".to_string(), vec![1, 2]),
            ("key2".to_string(), vec![3, 4]),
            ("key3".to_string(), vec![5, 6]),
        ];

        // Add values
        for (key, data) in values.iter() {
            cache
                .set(key.clone(), data.clone())
                .expect("Failed to set value");
        }

        // Check keys
        let mut cache_keys: Vec<_> = cache
            .keys()
            .expect("Failed to get keys")
            .collect();
        cache_keys.sort();
        let mut expected_keys: Vec<_> =
            values.iter().map(|(k, _)| k.clone()).collect();
        expected_keys.sort();

        assert_eq!(cache_keys, expected_keys);
    }

    #[test]
    fn test_memory_eviction() {
        let temp_dir = create_temp_dir();
        let mut cache = Cache::new(
            "test".to_string(),
            temp_dir.path(),
            8,    // Very small limit to force eviction
            true, // Enable preloading by default
        )
        .expect("Failed to create cache");

        // Add first value
        let key1 = "key1.txt".to_string();
        let value1 = vec![1, 2, 3, 4, 5, 7];
        cache
            .set(key1.clone(), value1.clone())
            .expect("Failed to set value1");

        // Add second value to trigger eviction
        let key2 = "key2.json".to_string();
        let value2 = vec![5, 6, 8];
        cache
            .set(key2.clone(), value2.clone())
            .expect("Failed to set value2");

        // First value should be evicted from memory but still on disk
        assert!(cache.memory_cache.get(&key1).is_none());
        assert_eq!(cache.get(&key1).unwrap(), value1); // Should load from disk
    }

    #[test]
    fn test_large_value_handling() {
        let temp_dir = create_temp_dir();
        let mut cache = create_test_cache(&temp_dir);
        let key = "large_key".to_string();
        let large_value = vec![0; 2 * 1024 * 1024]; // 2MB, larger than cache

        // Should fail to cache in memory but succeed in writing to disk
        assert!(cache
            .set(key.clone(), large_value.clone())
            .is_ok());
        assert_eq!(cache.get(&key).unwrap(), large_value); // Should load from disk
    }

    #[test]
    fn test_persistence() {
        let temp_dir = create_temp_dir();
        let key = "persist_key".to_string();
        let value = vec![1, 2, 3, 4];

        // Scope for first cache instance
        {
            let mut cache =
                Cache::new("test".to_string(), temp_dir.path(), 1024, true)
                    .expect("Failed to create first cache");
            cache
                .set(key.clone(), value.clone())
                .expect("Failed to set value");
        }

        // Create new cache instance pointing to same directory
        let mut cache2 =
            Cache::new("test".to_string(), temp_dir.path(), 1024, true)
                .expect("Failed to create second cache");

        // Should be able to read value written by first instance
        let retrieved: Vec<u8> = cache2.get(&key).expect("Failed to get value");
        assert_eq!(retrieved, value);
    }

    #[test]
    fn test_concurrent_reads() {
        use std::thread;

        let temp_dir = create_temp_dir();
        let mut cache = create_test_cache(&temp_dir);
        let key = "test_key".to_string();
        let value = vec![1, 2, 3, 4];

        // Set up initial cache with data
        cache
            .set(key.clone(), value.clone())
            .expect("Failed to set value");

        // Create multiple reader caches
        let mut handles: Vec<thread::JoinHandle<Option<Vec<u8>>>> = vec![];
        for _ in 0..3 {
            let key = key.clone();
            let cache_path = temp_dir.path().to_path_buf();

            handles.push(thread::spawn(move || {
                let mut reader_cache =
                    Cache::new("test".to_string(), &cache_path, 1024, true)
                        .expect("Failed to create reader cache");

                reader_cache.get(&key)
            }));
        }

        // All readers should get the same value
        for handle in handles {
            let result = handle.join().expect("Thread panicked");
            assert_eq!(result.unwrap(), value);
        }
    }

    #[test]
    fn test_duplicate_set() {
        let temp_dir = create_temp_dir();
        let mut cache = create_test_cache(&temp_dir);
        let key = "dup_key".to_string();
        let value1 = vec![1, 2, 3, 4];
        let value2 = vec![5, 6, 7, 8];

        // First set
        cache
            .set(key.clone(), value1.clone())
            .expect("Failed to set first value");

        // Second set with same key should panic
        assert!(cache.set(key.clone(), value2).is_err());

        // Should still have first value
        let retrieved = cache.get(&key).expect("Failed to get value");
        assert_eq!(retrieved, value1);
    }

    #[test]
    fn test_loads_recent_files_first() {
        let temp_dir = create_temp_dir();
        let mut cache: Cache<String, Vec<u8>> = Cache::new(
            "test".to_string(),
            temp_dir.path(),
            4,    // Small limit to force selection
            true, // Enable preloading by default
        )
        .expect("Failed to create cache");

        // Create files with different timestamps
        let files = vec![
            (
                "old.txt",
                vec![1, 2, 3],
                SystemTime::now() - Duration::from_secs(100),
            ),
            ("new.txt", vec![3, 4], SystemTime::now()),
        ];

        for (name, data, time) in files {
            let path = temp_dir.path().join(name);
            let mut file = File::create(path).unwrap();
            file.write_all(&data).unwrap();
            file.set_modified(time).unwrap();
            file.sync_all().unwrap();
        }

        // Reload cache
        cache.load_fs().expect("Failed to load files");

        // Verify newer file is in memory
        assert!(cache
            .memory_cache
            .contains(&"new.txt".to_string()));
        assert!(!cache
            .memory_cache
            .contains(&"old.txt".to_string()));
    }

    #[test]
    #[should_panic(expected = "Capacity can't be zero")]
    fn test_zero_capacity() {
        let temp_dir = create_temp_dir();
        let _cache: std::result::Result<Cache<String, Vec<u8>>, ArklibError> =
            Cache::new("test".to_string(), temp_dir.path(), 0, true);
    }

    #[test]
    fn test_memory_tracking() {
        let temp_dir = create_temp_dir();
        let mut cache = create_test_cache(&temp_dir);
        let key = "track_key".to_string();
        let value = vec![1, 2, 3, 4]; // 4 bytes

        cache
            .set(key.clone(), value)
            .expect("Failed to set value");

        // Memory usage should match file size
        assert_eq!(cache.current_memory_bytes, 4);
    }

    // TODO: Add More Test
}
