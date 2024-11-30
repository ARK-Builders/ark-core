use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use data_error::{ArklibError, Result};
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
    K: Ord + Clone + std::fmt::Display + std::hash::Hash + std::str::FromStr,
    V: Clone + std::fmt::Debug + AsRef<[u8]> + From<Vec<u8>>,
{
    /// Creates a new cache with the given label, storage path, and memory limit.
    pub fn new(
        label: String,
        path: &Path,
        max_memory_bytes: usize,
    ) -> Result<Self> {
        let mut cache = Self {
            label: label.clone(),
            path: PathBuf::from(path),
            // TODO: NEED FIX
            memory_cache: LruCache::new(
                NonZeroUsize::new(max_memory_bytes)
                    .expect("Capacity can't be zero"),
            ),
            current_memory_bytes: 0,
            max_memory_bytes,
        };

        log::debug!(
            "cache/{}: initialized with {} bytes limit",
            label,
            max_memory_bytes
        );

        cache.load_fs()?;
        Ok(cache)
    }

    /// Retrieves a value by key from memory cache or disk, returns None if not found.
    pub fn get(&mut self, key: &K) -> Option<V> {
        log::debug!("cache/{}: retrieving value for key {}", self.label, key);

        if let Some(value) = self.get_from_memory(key) {
            return Some(value);
        }

        self.get_from_disk(key)
    }

    /// Stores a value with the given key, writing to disk and updating memory cache.
    pub fn set(&mut self, key: K, value: V) -> Result<()> {
        log::debug!("cache/{}: setting value for key {}", self.label, key);
        // Check if value already exists
        if self.get(&key).is_some() {
            log::debug!("cache/{}: skipping existing key {}", self.label, key);
            return Ok(());
        }

        // Always write to disk first
        self.write_to_disk(&key, &value)?;

        // Then update memory cache
        self.cache_in_memory(&key, &value)?;

        log::debug!("cache/{}: set key={}", self.label, key);
        Ok(())
    }

    fn load_fs(&mut self) -> Result<()> {
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
            let path = entry?.path();
            if path.is_file() {
                let key: K =
                    extract_key_from_file_path(&self.label, &path, true)?;
                file_metadata.push((key.clone(), self.get_file_size(&key)?));
            }
        }

        // Sort by size before loading
        file_metadata.sort_by(|a, b| b.1.cmp(&a.1));

        // Clear existing cache
        self.memory_cache.clear();
        self.current_memory_bytes = 0;

        // Load files that fit in memory
        let mut loaded_bytes = 0;
        let total_bytes: usize =
            file_metadata.iter().map(|(_, size)| size).sum();

        for (key, approx_size) in file_metadata {
            if loaded_bytes + approx_size <= self.max_memory_bytes {
                match self.load_value_from_disk(&key) {
                    Ok(value) => {
                        // let actual_size = Self::estimate_size(&value);
                        let actual_size = self.get_file_size(&key)?;
                        if loaded_bytes + actual_size <= self.max_memory_bytes {
                            self.memory_cache.put(
                                key,
                                CacheEntry {
                                    value,
                                    size: actual_size,
                                },
                            );
                            loaded_bytes += actual_size;
                        }
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

    fn get_from_memory(&mut self, key: &K) -> Option<V> {
        self.memory_cache
            .get(key)
            .map(|entry| entry.value.clone())
    }

    fn get_from_disk(&mut self, key: &K) -> Option<V> {
        let file_path = self.path.join(key.to_string());
        if !file_path.exists() {
            log::warn!("cache/{}: no value found for key {}", self.label, key);
            return None;
        }

        match self.load_value_from_disk(key) {
            Ok(value) => {
                if let Err(err) = self.cache_in_memory(key, &value) {
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

    fn write_to_disk(&mut self, key: &K, value: &V) -> Result<()> {
        log::debug!("cache/{}: writing to disk for key {}", self.label, key);
        fs::create_dir_all(&self.path)?;

        let file_path = self.path.join(key.to_string());
        debug_assert!(
            !file_path.exists(),
            "File {} should not exist before writing",
            file_path.display()
        );

        fs::write(&file_path, value.as_ref()).map_err(|err| {
            ArklibError::Storage(
                self.label.clone(),
                format!("Failed to write value for key {}: {}", key, err),
            )
        })
    }

    fn load_value_from_disk(&self, key: &K) -> Result<V>
    where
        V: From<Vec<u8>>, // Add trait bound for reading binary data
    {
        let file_path = self.path.join(key.to_string());
        let contents = fs::read(&file_path)?;
        Ok(V::from(contents))
    }

    fn get_file_size(&self, key: &K) -> Result<usize> {
        Ok(fs::metadata(self.path.join(key.to_string()))?.len() as usize)
    }

    fn cache_in_memory(&mut self, key: &K, value: &V) -> Result<()> {
        log::debug!("cache/{}: caching in memory for key {}", self.label, key);
        let size = self.get_file_size(key)?;

        // If single value is larger than total limit, just skip memory caching
        if size > self.max_memory_bytes {
            return Err(ArklibError::Storage(
                self.label.clone(),
                format!(
                    "value size {} exceeds limit {}",
                    size, self.max_memory_bytes
                ),
            ));
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
    use rstest::{fixture, rstest};
    use std::fs;
    use tempdir::TempDir;

    // Helper struct that implements required traits
    #[derive(Clone, Debug, PartialEq)]
    struct TestValue(Vec<u8>);

    impl AsRef<[u8]> for TestValue {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    impl From<Vec<u8>> for TestValue {
        fn from(bytes: Vec<u8>) -> Self {
            TestValue(bytes)
        }
    }

    #[fixture]
    fn temp_dir() -> TempDir {
        TempDir::new("tmp").expect("Failed to create temporary directory")
    }

    #[fixture]
    fn cache(temp_dir: TempDir) -> Cache<String, TestValue> {
        Cache::new(
            "test".to_string(),
            temp_dir.path(),
            1024 * 1024, // 1MB
        )
        .expect("Failed to create cache")
    }

    #[rstest]
    fn test_new_cache(cache: Cache<String, TestValue>) {
        assert_eq!(cache.current_memory_bytes, 0);
        assert_eq!(cache.max_memory_bytes, 1024 * 1024);
    }

    #[rstest]
    fn test_set_and_get(mut cache: Cache<String, TestValue>) {
        let key = "test_key".to_string();
        let value = TestValue(vec![1, 2, 3, 4]);

        // Test set
        cache
            .set(key.clone(), value.clone())
            .expect("Failed to set value");

        // Test get
        let retrieved = cache.get(&key).expect("Failed to get value");
        assert_eq!(retrieved.0, value.0);
    }

    #[rstest]
    fn test_get_nonexistent(mut cache: Cache<String, TestValue>) {
        assert!(cache.get(&"nonexistent".to_string()).is_none());
    }

    #[rstest]
    fn test_memory_eviction(temp_dir: TempDir) {
        // Create cache with small memory limit
        let mut cache = Cache::new(
            "test".to_string(),
            temp_dir.path(),
            5, // Very small limit to force eviction
        )
        .expect("Failed to create cache");

        // Add first value
        let key1 = "key1".to_string();
        let value1 = TestValue(vec![1, 2, 3, 4]);
        cache
            .set(key1.clone(), value1.clone())
            .expect("Failed to set value1");

        // Add second value to trigger eviction
        let key2 = "key2".to_string();
        let value2 = TestValue(vec![5, 6, 7, 8]);
        cache
            .set(key2.clone(), value2.clone())
            .expect("Failed to set value2");

        // First value should be evicted from memory but still on disk
        assert!(cache.memory_cache.get(&key1).is_none());
        assert_eq!(cache.get(&key1).unwrap(), value1); // Should load from disk
    }

    #[rstest]
    fn test_large_value_handling(mut cache: Cache<String, TestValue>) {
        // let (mut cache, _dir) = setup_temp_cache();
        let key = "large_key".to_string();
        let large_value = TestValue(vec![0; 2 * 1024 * 1024]); // 2MB, larger than cache

        // Should fail to cache in memory but succeed in writing to disk
        assert!(cache
            .set(key.clone(), large_value.clone())
            .is_err());
    }

    #[rstest]
    fn test_persistence(temp_dir: TempDir) {
        let key = "persist_key".to_string();
        let value = TestValue(vec![1, 2, 3, 4]);

        // Scope for first cache instance
        {
            let mut cache =
                Cache::new("test".to_string(), temp_dir.path(), 1024)
                    .expect("Failed to create first cache");
            cache
                .set(key.clone(), value.clone())
                .expect("Failed to set value");
        }

        // Create new cache instance pointing to same directory
        let mut cache2 = Cache::new("test".to_string(), temp_dir.path(), 1024)
            .expect("Failed to create second cache");

        // Should be able to read value written by first instance
        let retrieved: TestValue =
            cache2.get(&key).expect("Failed to get value");
        assert_eq!(retrieved.0, value.0);
    }

    #[rstest]
    fn test_duplicate_set(mut cache: Cache<String, TestValue>) {
        let key = "dup_key".to_string();
        let value1 = TestValue(vec![1, 2, 3, 4]);
        let value2 = TestValue(vec![5, 6, 7, 8]);

        // First set
        cache
            .set(key.clone(), value1.clone())
            .expect("Failed to set first value");

        // Second set with same key should be skipped
        cache
            .set(key.clone(), value2)
            .expect("Failed to set second value");

        // Should still have first value
        let retrieved = cache.get(&key).expect("Failed to get value");
        assert_eq!(retrieved.0, value1.0);
    }

    #[rstest]
    fn test_load_fs(temp_dir: TempDir) {
        let path = temp_dir.path();

        // Manually create some files
        fs::write(path.join("key1"), vec![1, 2, 3])
            .expect("Failed to write file 1");
        fs::write(path.join("key2"), vec![4, 5, 6])
            .expect("Failed to write file 2");

        // Create new cache instance to load existing files
        let mut cache2: Cache<String, TestValue> =
            Cache::new("test".to_string(), path, 1024)
                .expect("Failed to create cache");

        // Check if files were loaded
        assert!(cache2.get(&"key1".to_string()).is_some());
        assert!(cache2.get(&"key2".to_string()).is_some());
    }

    #[rstest]
    #[should_panic(expected = "Capacity can't be zero")]
    fn test_zero_capacity(temp_dir: TempDir) {
        let _cache: std::result::Result<Cache<String, TestValue>, ArklibError> =
            Cache::new("test".to_string(), temp_dir.path(), 0);
    }

    #[rstest]
    fn test_memory_tracking(mut cache: Cache<String, TestValue>) {
        let key = "track_key".to_string();
        let value = TestValue(vec![1, 2, 3, 4]); // 4 bytes

        cache
            .set(key.clone(), value)
            .expect("Failed to set value");

        // Memory usage should match file size
        assert_eq!(cache.current_memory_bytes, 4);
    }
}
