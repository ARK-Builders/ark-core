use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write;
use std::time::SystemTime;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::base_storage::{BaseStorage, SyncStatus};
use crate::monoid::Monoid;
use data_error::{ArklibError, Result};

/// Represents a folder storage system that persists data to disk.
pub struct FolderStorage<K, V> {
    /// Label for logging
    label: String,
    /// Path to the underlying folder where data is persisted
    path: PathBuf,
    /// `modified` can be used to track the last time a file was modified in memory.
    /// where the key is the path of the file inside the directory.
    modified: BTreeMap<K, SystemTime>,
    /// `written_to_disk` can be used to track the last time a file written or read from disk.
    /// where the key is the path of the file inside the directory.
    written_to_disk: BTreeMap<K, SystemTime>,
    data: BTreeMap<K, V>,
    /// Temporary store for deleted keys until storage is synced
    deleted_keys: BTreeSet<K>,
}

impl<K, V> AsRef<BTreeMap<K, V>> for FolderStorage<K, V>
where
    K: Ord,
{
    fn as_ref(&self) -> &BTreeMap<K, V> {
        &self.data
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
    V: Clone + serde::Serialize + serde::de::DeserializeOwned + Monoid<V>,
{
    /// Create a new folder storage with a diagnostic label and directory path
    /// The storage will be initialized using the disk data, if the path exists
    ///
    /// Note: if the folder storage already exists, the data will be read from the folder
    /// without overwriting it.
    pub fn new(label: String, path: &Path) -> Result<Self> {
        let mut storage = Self {
            label,
            path: PathBuf::from(path),
            modified: BTreeMap::new(),
            written_to_disk: BTreeMap::new(),
            data: BTreeMap::new(),
            deleted_keys: BTreeSet::new(),
        };

        if Path::exists(path) {
            storage.read_fs()?;
        }

        Ok(storage)
    }

    /// Load mapping from folder storage
    fn load_fs_data(&mut self) -> Result<()> {
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

        let mut data = BTreeMap::new();

        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .map_or(false, |ext| ext == "json")
            {
                let key: K = self.extract_key_from_file_path(&path)?;
                let file = File::open(&path)?;
                let value: V =
                    serde_json::from_reader(file).map_err(|err| {
                        ArklibError::Storage(
                            self.label.clone(),
                            err.to_string(),
                        )
                    })?;

                data.insert(key.clone(), value);

                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        self.written_to_disk.insert(key.clone(), modified);
                        self.modified.insert(key, modified);
                    }
                }
            }
        }

        self.data = data;
        Ok(())
    }
    /// Resolve differences between memory and disk data
    fn resolve_divergence(&mut self) -> Result<()> {
        let new_data = FolderStorage::new("new_data".into(), &self.path)?;

        for (key, new_value) in new_data.data.iter() {
            if let Some(existing_value) = self.data.get(key) {
                let existing_value_updated = self
                    .modified
                    .get(key)
                    .and_then(|ram_stamp| {
                        self.written_to_disk
                            .get(key)
                            .map(|disk_stamp| ram_stamp > disk_stamp)
                    })
                    .unwrap_or(false);

                // Use monoid to combine value for the given key
                // if the memory and disk have diverged
                if existing_value_updated {
                    let resolved_value = V::combine(existing_value, new_value);
                    self.data.insert(key.clone(), resolved_value);
                } else {
                    self.data.insert(key.clone(), new_value.clone());
                }
            } else {
                self.data.insert(key.clone(), new_value.clone());
            }
        }

        Ok(())
    }

    /// Remove files from disk that are not present in memory
    fn remove_files_not_in_ram(&mut self) -> Result<()> {
        for key in self.deleted_keys.iter() {
            let file_path = self.path.join(format!("{}.json", key));
            if file_path.exists() {
                fs::remove_file(&file_path).expect("Failed to delete file");
            }
        }
        Ok(())
    }

    pub fn extract_key_from_file_path(&self, path: &Path) -> Result<K> {
        path.file_stem()
            .ok_or_else(|| {
                ArklibError::Storage(
                    self.label.clone(),
                    "Failed to extract file stem from filename".to_owned(),
                )
            })?
            .to_str()
            .ok_or_else(|| {
                ArklibError::Storage(
                    self.label.clone(),
                    "Failed to convert file stem to string".to_owned(),
                )
            })?
            .parse::<K>()
            .map_err(|_| {
                ArklibError::Storage(
                    self.label.clone(),
                    "Failed to parse key from filename".to_owned(),
                )
            })
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
    V: Clone + serde::Serialize + serde::de::DeserializeOwned + Monoid<V>,
{
    /// Set a key-value pair in the internal mapping
    fn set(&mut self, key: K, value: V) {
        self.data.insert(key.clone(), value);
        self.deleted_keys.remove(&key);
        self.modified.insert(key, SystemTime::now());
    }

    /// Remove an entry from the internal mapping given a key
    fn remove(&mut self, id: &K) -> Result<()> {
        match self.data.remove(id) {
            Some(_) => {
                self.deleted_keys.insert(id.clone());
                Ok(())
            }
            None => Err(ArklibError::Storage(
                self.label.clone(),
                "Key not found".to_owned(),
            )),
        }
    }

    /// Compare the timestamp of the storage files
    /// with the timestamps of the in-memory storage and the last written
    /// to time to determine if either of the two requires syncing.
    fn sync_status(&mut self) -> Result<SyncStatus> {
        let mut ram_newer = !self.deleted_keys.is_empty();
        let mut disk_newer = false;

        for key in self.data.keys() {
            let file_path = self.path.join(format!("{}.json", key));
            let ram_timestamp = self
                .modified
                .get(key)
                .expect("Data entry key should have ram timestamp");

            if let Ok(metadata) = fs::metadata(&file_path) {
                if let Ok(disk_timestamp) = metadata.modified() {
                    match ram_timestamp.cmp(&disk_timestamp) {
                        std::cmp::Ordering::Greater => {
                            ram_newer = true;
                            log::debug!(
                                "RAM newer: file {} is newer in RAM",
                                file_path.display()
                            );
                        }
                        std::cmp::Ordering::Less => {
                            disk_newer = true;
                            log::debug!(
                                "Disk newer: file {} is newer on disk, ram: {}, disk: {}",
                                file_path.display(),
                                ram_timestamp.elapsed().unwrap().as_secs(),
                                disk_timestamp.elapsed().unwrap().as_secs()
                            );
                        }
                        std::cmp::Ordering::Equal => {}
                    }
                } else {
                    // If we can't read the disk timestamp, assume RAM is newer
                    ram_newer = true;
                    log::debug!(
                        "RAM newer: couldn't read disk timestamp for {}",
                        file_path.display()
                    );
                }
            } else {
                // If the file doesn't exist on disk, RAM is newer
                ram_newer = true;
                log::debug!(
                    "RAM newer: file {} doesn't exist on disk",
                    file_path.display()
                );
            }

            // If we've found both RAM and disk modifications, we can stop checking
            if ram_newer && disk_newer {
                log::debug!(
                    "Both RAM and disk modifications found, stopping check"
                );
                break;
            }
        }

        // Skip this check if this divergent condition has already been reached
        if !(ram_newer && disk_newer) {
            // Check for files on disk that aren't in RAM
            for entry in fs::read_dir(&self.path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .map_or(false, |ext| ext == "json")
                {
                    let key = self.extract_key_from_file_path(&path)?;
                    if !self.data.contains_key(&key)
                        && !self.deleted_keys.contains(&key)
                    {
                        disk_newer = true;
                        break;
                    }
                }
            }
        }

        let new_status = match (ram_newer, disk_newer) {
            (false, false) => SyncStatus::InSync,
            (true, false) => SyncStatus::StorageStale,
            (false, true) => SyncStatus::MappingStale,
            (true, true) => SyncStatus::Diverge,
        };

        log::info!("{} sync status is {}", self.label, new_status);
        Ok(new_status)
    }

    /// Sync the in-memory storage with the storage on disk
    fn sync(&mut self) -> Result<()> {
        match self.sync_status()? {
            SyncStatus::InSync => {}
            SyncStatus::MappingStale => {
                self.read_fs()?;
            }
            SyncStatus::StorageStale => {
                self.write_fs()?;
            }
            SyncStatus::Diverge => {
                self.resolve_divergence()?;
                self.write_fs()?;
            }
        };

        self.deleted_keys.clear();
        Ok(())
    }

    /// Read the data from folder storage
    fn read_fs(&mut self) -> Result<&BTreeMap<K, V>> {
        self.load_fs_data()?;
        Ok(&self.data)
    }

    /// Get a value from the internal mapping
    fn get(&self, id: &K) -> Option<&V> {
        self.data.get(id)
    }

    /// Writes the data to a folder.
    ///
    /// Updates the file's modified timestamp to avoid OS timing issues, which may arise due to file system timestamp precision.
    /// EXT3 has 1-second precision, while EXT4 can be more precise but not always.
    /// This is addressed by modifying the metadata and calling `sync_all()` after file writes.
    fn write_fs(&mut self) -> Result<()> {
        fs::create_dir_all(&self.path)?;

        for (key, value) in &self.data {
            let file_path = self.path.join(format!("{}.json", key));
            let mut file = File::create(&file_path)?;
            file.write_all(serde_json::to_string_pretty(&value)?.as_bytes())?;
            file.flush()?;

            let new_timestamp = SystemTime::now();
            file.set_modified(new_timestamp)?;
            file.sync_all()?;

            self.written_to_disk
                .insert(key.clone(), new_timestamp);
            self.modified.insert(key.clone(), new_timestamp);
        }

        // Delete files for previously deleted keys
        self.deleted_keys.iter().for_each(|key| {
            log::debug!("Deleting key: {}", key);
            self.data.remove(key);
            self.modified.remove(key);
            self.written_to_disk.remove(key);
            let file_path = self.path.join(format!("{}.json", key));
            if file_path.exists() {
                fs::remove_file(&file_path).expect("Failed to delete file");
            }
        });
        self.deleted_keys.clear();

        // Remove files for keys that no longer exist
        self.remove_files_not_in_ram()?;

        log::info!(
            "{} {} entries have been written",
            self.label,
            self.data.len()
        );
        Ok(())
    }

    /// Erase the folder from disk
    fn erase(&self) -> Result<()> {
        fs::remove_dir_all(&self.path).map_err(|err| {
            ArklibError::Storage(self.label.clone(), err.to_string())
        })
    }

    /// Merge the data from another folder storage instance into this folder storage instance
    fn merge_from(&mut self, other: impl AsRef<BTreeMap<K, V>>) -> Result<()>
    where
        V: Monoid<V>,
    {
        let other_entries = other.as_ref();
        for (key, value) in other_entries {
            if let Some(existing_value) = self.data.get(key) {
                let resolved_value = V::combine(existing_value, value);
                self.set(key.clone(), resolved_value);
            } else {
                self.set(key.clone(), value.clone())
            }
            self.modified
                .insert(key.clone(), SystemTime::now());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        base_storage::{BaseStorage, SyncStatus},
        folder_storage::FolderStorage,
        monoid::Monoid,
    };
    use std::{
        collections::BTreeSet,
        fs::{self, File},
        io::Write,
        thread,
        time::{Duration, SystemTime},
    };

    use data_error::Result;
    use quickcheck_macros::quickcheck;
    use serde::{Deserialize, Serialize};
    use tempdir::TempDir;

    #[test]
    fn test_folder_storage_write_read() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let mut storage =
            FolderStorage::new("test".to_owned(), temp_dir.path()).unwrap();

        storage.set("key1".to_owned(), "value1".to_string());
        storage.set("key2".to_owned(), "value2".to_string());

        assert!(storage.remove(&"key1".to_string()).is_ok());
        storage
            .write_fs()
            .expect("Failed to write data to disk");

        let data_read = storage
            .read_fs()
            .expect("Failed to read data from disk");
        assert_eq!(data_read.len(), 1);
        assert_eq!(data_read.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_folder_storage_auto_delete() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let mut storage =
            FolderStorage::new("test".to_owned(), temp_dir.path()).unwrap();

        storage.set("key1".to_string(), "value1".to_string());
        storage.set("key1".to_string(), "value2".to_string());
        assert!(storage.write_fs().is_ok());
        assert_eq!(temp_dir.path().exists(), true);

        if let Err(err) = storage.erase() {
            panic!("Failed to delete folder: {:?}", err);
        }
        assert!(!temp_dir.path().exists());
    }

    #[test]
    fn test_folder_metadata_timestamp_updated() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let mut storage =
            FolderStorage::new("test".to_owned(), temp_dir.path()).unwrap();
        storage.write_fs().unwrap();

        storage.set("key1".to_string(), "value1".to_string());
        let before_write = fs::metadata(&temp_dir.path())
            .unwrap()
            .modified()
            .unwrap();
        thread::sleep(Duration::from_millis(10));
        storage.write_fs().unwrap();
        let after_write = fs::metadata(&temp_dir.path())
            .unwrap()
            .modified()
            .unwrap();
        println!(
            "before_write: {:?}, after_write: {:?}",
            before_write, after_write
        );
        assert!(before_write < after_write);
    }

    #[test]
    fn test_folder_storage_is_storage_updated() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let mut storage =
            FolderStorage::new("test".to_owned(), temp_dir.path()).unwrap();
        storage.write_fs().unwrap();
        assert_eq!(storage.sync_status().unwrap(), SyncStatus::InSync);

        storage.set("key1".to_string(), "value1".to_string());
        assert_eq!(storage.sync_status().unwrap(), SyncStatus::StorageStale);
        storage.write_fs().unwrap();
        assert_eq!(storage.sync_status().unwrap(), SyncStatus::InSync);

        // External data manipulation
        let mut mirror_storage = FolderStorage::new(
            "MirrorTestStorage".to_string(),
            temp_dir.path(),
        )
        .unwrap();
        assert_eq!(mirror_storage.sync_status().unwrap(), SyncStatus::InSync);

        mirror_storage.set("key1".to_string(), "value3".to_string());
        assert_eq!(
            mirror_storage.sync_status().unwrap(),
            SyncStatus::StorageStale
        );

        mirror_storage.write_fs().unwrap();
        assert_eq!(mirror_storage.sync_status().unwrap(), SyncStatus::InSync);

        // receive updates from external data manipulation
        assert_eq!(storage.sync_status().unwrap(), SyncStatus::MappingStale);
        storage.read_fs().unwrap();
        assert_eq!(storage.sync_status().unwrap(), SyncStatus::InSync);
        assert_eq!(mirror_storage.sync_status().unwrap(), SyncStatus::InSync);
    }

    #[test]
    fn test_monoid_combine() {
        let temp_dir =
            TempDir::new("tmp").expect("Failed to create temporary directory");
        let mut storage1 =
            FolderStorage::new("test".to_owned(), temp_dir.path()).unwrap();
        let mut storage2 =
            FolderStorage::new("test".to_owned(), temp_dir.path()).unwrap();

        storage1.set("key1".to_string(), 2);
        storage1.set("key2".to_string(), 6);

        storage2.set("key1".to_string(), 3);
        storage2.set("key3".to_string(), 9);

        storage1.merge_from(&storage2).unwrap();
        assert_eq!(storage1.as_ref().get("key1"), Some(&3));
        assert_eq!(storage1.as_ref().get("key2"), Some(&6));
        assert_eq!(storage1.as_ref().get("key3"), Some(&9));
    }

    use quickcheck::{Arbitrary, Gen};
    use std::collections::{BTreeMap, HashSet};

    // Assuming FolderStorage, BaseStorage, SyncStatus, and other necessary types are in scope

    #[derive(Clone, Debug)]
    enum StorageOperation {
        Set(String),
        Remove(String),
        Sync,
        ExternalModify(String),
        ExternalSet(String),
    }

    #[derive(Clone, Debug)]
    struct StorageOperationSequence(Vec<StorageOperation>);

    impl Arbitrary for StorageOperationSequence {
        fn arbitrary(g: &mut Gen) -> Self {
            let mut existing_keys = HashSet::new();
            let mut ops = Vec::new();
            let size = usize::arbitrary(g) % 100 + 1; // Generate 1 to 100 operations

            for _ in 0..size {
                let op = match u8::arbitrary(g) % 9 {
                    0 | 1 | 2 | 3 | 4 => {
                        let key = u8::arbitrary(g).to_string();
                        existing_keys.insert(key.clone());
                        StorageOperation::Set(key)
                    }
                    5 if !existing_keys.is_empty() => {
                        let key = g
                            .choose(
                                &existing_keys
                                    .iter()
                                    .cloned()
                                    .collect::<Vec<_>>(),
                            )
                            .unwrap()
                            .clone();
                        existing_keys.remove(&key);
                        StorageOperation::Remove(key)
                    }
                    6 => StorageOperation::Sync,
                    7 if !existing_keys.is_empty() => {
                        let key = g
                            .choose(
                                &existing_keys
                                    .iter()
                                    .cloned()
                                    .collect::<Vec<_>>(),
                            )
                            .unwrap()
                            .clone();
                        StorageOperation::ExternalModify(key)
                    }
                    _ => {
                        let key = u8::arbitrary(g).to_string();
                        StorageOperation::ExternalSet(key)
                    }
                };
                ops.push(op);
            }

            StorageOperationSequence(ops)
        }
    }

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    struct Dummy;

    impl Monoid<Dummy> for Dummy {
        fn neutral() -> Dummy {
            Dummy
        }

        fn combine(_a: &Dummy, _b: &Dummy) -> Dummy {
            Dummy
        }
    }

    // #[test_log::test]
    #[quickcheck]
    fn prop_folder_storage_correct(
        StorageOperationSequence(operations): StorageOperationSequence,
    ) {
        let temp_dir =
            TempDir::new("temp").expect("Failed to create temporary directory");
        let path = temp_dir.path();

        let mut storage =
            FolderStorage::<String, Dummy>::new("test".to_string(), &path)
                .unwrap();
        let mut expected_data: BTreeMap<String, Dummy> = BTreeMap::new();
        let mut pending_deletes = BTreeSet::new();
        let mut pending_sets: BTreeMap<String, usize> = BTreeMap::new();
        let mut pending_external: BTreeMap<String, usize> = BTreeMap::new();

        // Check initial state
        assert_eq!(
            storage.sync_status().unwrap(),
            SyncStatus::InSync,
            "Storage should be InSync when created"
        );

        let v = Dummy;
        for (i, op) in operations.into_iter().enumerate() {
            match op {
                StorageOperation::Set(k) => {
                    storage.set(k.clone(), v);
                    pending_sets.insert(k.clone(), i);
                    pending_deletes.remove(&k);

                    let status = storage.sync_status().unwrap();
                    let expected_status = expected_status(
                        &pending_external,
                        &pending_sets,
                        &pending_deletes,
                    );
                    assert_eq!(status, expected_status);
                }
                StorageOperation::Remove(k) => {
                    storage.remove(&k).unwrap();
                    pending_sets.remove(&k);
                    pending_deletes.insert(k.clone());

                    let status = storage.sync_status().unwrap();
                    let expected_status = expected_status(
                        &pending_external,
                        &pending_sets,
                        &pending_deletes,
                    );
                    assert_eq!(status, expected_status);
                }
                StorageOperation::Sync => {
                    storage.sync().unwrap();

                    // Note: Concurrent deletes are overriden by sets
                    // Hence, deletes are weak. Also, for values where
                    // monoidal combination is relevant, this logic will
                    // have to be updated.
                    pending_sets
                        .keys()
                        .chain(pending_external.keys())
                        .for_each(|k| {
                            expected_data.insert(k.clone(), v);
                        });
                    pending_deletes.iter().for_each(|key| {
                        expected_data.remove(key);
                    });

                    pending_sets.clear();
                    pending_external.clear();
                    pending_deletes.clear();

                    let status = storage.sync_status().unwrap();
                    assert_eq!(status, SyncStatus::InSync);
                    assert_eq!(storage.data, expected_data);
                }
                StorageOperation::ExternalModify(k)
                | StorageOperation::ExternalSet(k) => {
                    perform_external_modification(path, &k, v).unwrap();
                    pending_external.insert(k.clone(), i);
                    let status = storage.sync_status().unwrap();
                    let expected_status = expected_status(
                        &pending_external,
                        &pending_sets,
                        &pending_deletes,
                    );
                    assert_eq!(status, expected_status);
                }
            }

            assert!(
                pending_sets
                    .keys()
                    .filter(|key| pending_deletes.contains(*key))
                    .count()
                    == 0
            );
        }
    }

    fn perform_external_modification(
        path: &std::path::Path,
        key: &str,
        value: Dummy,
    ) -> Result<()> {
        let mut file = File::create(path.join(format!("{}.json", key)))?;
        file.write_all(serde_json::to_string_pretty(&value)?.as_bytes())?;
        file.flush()?;
        let time = SystemTime::now();
        file.set_modified(time).unwrap();
        file.sync_all()?;
        Ok(())
    }

    fn expected_status(
        pending_external: &BTreeMap<String, usize>,
        pending_sets: &BTreeMap<String, usize>,
        pending_deletes: &BTreeSet<String>,
    ) -> SyncStatus {
        let ram_newer = !pending_deletes.is_empty();
        let ram_newer = ram_newer
            || pending_sets
                .iter()
                .any(|(k, v)| pending_external.get(k).map_or(true, |e| v > e));
        let disk_newer = pending_external
            .iter()
            .filter(|(k, _)| !pending_deletes.contains(*k))
            .any(|(k, v)| pending_sets.get(k).map_or(true, |s| v > s));

        match (ram_newer, disk_newer) {
            (false, false) => SyncStatus::InSync,
            (true, false) => SyncStatus::StorageStale,
            (false, true) => SyncStatus::MappingStale,
            (true, true) => SyncStatus::Diverge,
        }
    }
}
