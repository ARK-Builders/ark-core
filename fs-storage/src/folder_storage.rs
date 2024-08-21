use std::fs::{self, File};
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};
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
    /// `ram_timestamps` can be used to track the last time a file was modified in memory.
    /// where the key is the path of the file inside the directory.
    ram_timestamps: BTreeMap<K, SystemTime>,
    /// `disk_timestamps` can be used to track the last time a file written or read from disk.
    /// where the key is the path of the file inside the directory.
    disk_timestamps: BTreeMap<K, SystemTime>,
    data: FolderStorageData<K, V>,

    prev_sync_status: SyncStatus,
    // Tracks deleted entries till the next sync
    soft_delete: BTreeMap<K, SystemTime>,
}

/// A struct that represents the data stored in a [`FolderStorage`] instance.
pub struct FolderStorageData<K, V> {
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
            ram_timestamps: BTreeMap::new(),
            disk_timestamps: BTreeMap::new(),
            data: FolderStorageData {
                entries: BTreeMap::new(),
            },
            prev_sync_status: SyncStatus::InSync,
            soft_delete: BTreeMap::new(),
        };

        if Path::exists(path) {
            storage.read_fs()?;
        }

        Ok(storage)
    }

    /// Load mapping from folder storage
    fn load_fs_data(&mut self) -> Result<FolderStorageData<K, V>> {
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

        let mut data = FolderStorageData {
            entries: BTreeMap::new(),
        };

        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path.extension().map_or(false, |ext| ext == "bin")
            {
                let key: K = self.extract_key_from_file_path(&path)?;
                let mut file = File::open(&path)?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;

                let value: V = bincode::deserialize(&buffer).map_err(|e| {
                    ArklibError::Storage(
                        self.label.clone(),
                        format!("Failed to deserialize value: {}", e),
                    )
                })?;
                data.entries.insert(key.clone(), value);

                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        self.disk_timestamps.insert(key.clone(), modified);
                        self.ram_timestamps.insert(key, modified);
                    }
                }
            }
        }
        Ok(data)
    }

    /// Resolve differences between memory and disk data
    fn resolve_divergence(&mut self) -> Result<()> {
        let new_data = self.load_fs_data()?;
        let mut merged_data = BTreeMap::new();

        // filter new_Data
        if !self.soft_delete.is_empty() {
            for (key, value) in new_data.entries.iter() {
                if let Some(new_value) = self.soft_delete.get(key) {
                    if new_value > self.disk_timestamps.get(key).unwrap() {
                        continue;
                    }
                }
                merged_data.insert(key.clone(), value.clone());
            }
        } else {
            merged_data = new_data.entries.clone();
        }

        for (key, value) in self.data.entries.iter() {
            if let Some(new_value) = new_data.entries.get(key) {
                let resolved_value = V::combine(value, new_value);
                merged_data.insert(key.clone(), resolved_value);
            } else {
                merged_data.insert(key.clone(), value.clone());
            }
        }

        self.data.entries = merged_data;
        Ok(())
    }

    /// Remove files from disk that are not present in memory
    fn remove_files_not_in_ram(&mut self) -> Result<()> {
        if !self.soft_delete.is_empty() {
            for (key, time) in self.soft_delete.iter() {
                if time
                    > self
                        .disk_timestamps
                        .get(key)
                        .unwrap_or(&SystemTime::UNIX_EPOCH)
                {
                    let file_path = self.path.join(format!("{}.bin", key));
                    if !file_path.exists() {
                        continue;
                    }
                    if let Err(e) = fs::remove_file(&file_path) {
                        return Err(ArklibError::Storage(
                            self.label.clone(),
                            format!(
                                "Failed to remove file {:?}: {}",
                                file_path, e
                            ),
                        ));
                    }
                    self.disk_timestamps
                        .insert(key.clone(), SystemTime::now());
                }
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
        self.data.entries.insert(key.clone(), value);
        self.ram_timestamps.insert(key, SystemTime::now());
    }

    /// Remove an entry from the internal mapping given a key
    fn remove(&mut self, id: &K) -> Result<()> {
        self.data.entries.remove(id).ok_or_else(|| {
            ArklibError::Storage(self.label.clone(), "Key not found".to_owned())
        })?;
        // self.sync();
        self.soft_delete
            .insert(id.clone(), SystemTime::now());
        self.ram_timestamps
            .insert(id.clone(), SystemTime::now());
        Ok(())
    }

    /// Compare the timestamp of the storage files
    /// with the timestamps of the in-memory storage and the last written
    /// to time to determine if either of the two requires syncing.
    fn sync_status(&mut self) -> Result<SyncStatus> {
        let mut ram_newer = false;
        let mut disk_newer = false;

        for key in self.data.entries.keys() {
            let file_path = self.path.join(format!("{}.bin", key));
            let ram_timestamp = self
                .ram_timestamps
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
        if !ram_newer || !disk_newer {
            // Check for files on disk that aren't in RAM
            for entry in fs::read_dir(&self.path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file()
                    && path.extension().map_or(false, |ext| ext == "bin")
                {
                    let key = self.extract_key_from_file_path(&path)?;

                    if !self.data.entries.contains_key(&key) {
                        match self.soft_delete.get(&key) {
                            Some(soft_del) => {
                                let disk_time = self
                                    .disk_timestamps
                                    .get(&key)
                                    .unwrap_or(&UNIX_EPOCH);

                                if soft_del > disk_time {
                                    ram_newer = true;

                                    if let Ok(metadata) = fs::metadata(&path) {
                                        if let Ok(disk_timestamp) =
                                            metadata.modified()
                                        {
                                            if disk_timestamp > *soft_del {
                                                disk_newer = true;
                                            }
                                        }
                                    }
                                }
                            }
                            None => {
                                disk_newer = true;
                            }
                        }
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

        // Compare with previous status to detect transitions
        let final_status = match (self.prev_sync_status.clone(), &new_status) {
            (_, SyncStatus::InSync) => SyncStatus::InSync,
            (SyncStatus::Diverge, _) => SyncStatus::Diverge,
            (_, SyncStatus::Diverge) => SyncStatus::Diverge,
            (SyncStatus::StorageStale, SyncStatus::MappingStale) => {
                SyncStatus::Diverge
            }
            (SyncStatus::MappingStale, SyncStatus::StorageStale) => {
                SyncStatus::Diverge
            }
            _ => new_status,
        };

        self.prev_sync_status = final_status.clone();

        log::info!("{} sync status is {}", self.label, final_status);
        Ok(final_status)
    }

    /// Sync the in-memory storage with the storage on disk
    fn sync(&mut self) -> Result<()> {
        // self.prev_sync_status = SyncStatus::InSync;
        match self.sync_status()? {
            SyncStatus::InSync => Ok(()),
            SyncStatus::MappingStale => self.read_fs().map(|_| ()),
            SyncStatus::StorageStale => self.write_fs().map(|_| ()),
            SyncStatus::Diverge => {
                // let data = self.load_fs_data()?;
                // self.merge_from(&data)?;
                self.resolve_divergence()?;
                self.write_fs()?;
                Ok(())
            }
        }
    }

    /// Read the data from folder storage
    fn read_fs(&mut self) -> Result<&BTreeMap<K, V>> {
        let data = self.load_fs_data()?;
        self.data = data;
        Ok(&self.data.entries)
    }

    /// Get a value from the internal mapping
    fn get(&self, id: &K) -> Option<&V> {
        self.data.entries.get(id)
    }

    /// Write the data to folder
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
            self.ram_timestamps
                .insert(key.clone(), new_timestamp);
        }

        // Remove files for keys that no longer exist
        self.remove_files_not_in_ram()?;

        // Clear soft delete
        self.soft_delete.clear();

        log::info!(
            "{} {} entries have been written",
            self.label,
            self.data.entries.len()
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

#[cfg(test)]
mod tests {
    use crate::{
        base_storage::{BaseStorage, SyncStatus},
        folder_storage::FolderStorage,
        monoid::Monoid,
    };
    use std::{
        fs::{self, File},
        io::Write,
        thread,
        time::Duration,
    };

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

    use data_error::Result;
    use quickcheck::{Arbitrary, Gen};
    use std::collections::{BTreeMap, HashSet};
    use std::time::SystemTime;

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
                let op = match u8::arbitrary(g) % 5 {
                    0 => {
                        let key = u8::arbitrary(g).to_string();
                        existing_keys.insert(key.clone());
                        StorageOperation::Set(key)
                    }
                    1 if !existing_keys.is_empty() => {
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
                    2 => StorageOperation::Sync,
                    3 if !existing_keys.is_empty() => {
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
                        existing_keys.insert(key.clone());
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
        let mut expected_data_in_ram = BTreeMap::new();
        let mut expected_data_in_disk = BTreeMap::new();
        let mut removed_data = BTreeMap::new();
        let mut setted_data = BTreeMap::new();

        println!("Created storage");
        // Check initial state
        assert_eq!(
            storage.sync_status().unwrap(),
            SyncStatus::InSync,
            "Storage should be InSync when created"
        );

        let v = Dummy;
        for op in operations {
            let prev_status = storage.sync_status().unwrap();
            println!("Applying op: {:?}", op);

            match op {
                StorageOperation::Set(k) => {
                    storage.set(k.clone(), v);
                    expected_data_in_ram.insert(k.clone(), v);
                    *setted_data.entry(k.clone()).or_insert(0) += 1;

                    let status = storage.sync_status().unwrap();
                    match prev_status {
                        SyncStatus::InSync => {
                            assert_eq!(
                                status,
                                SyncStatus::StorageStale,
                                "Setting a key should make storage stale"
                            );
                        }
                        SyncStatus::MappingStale => {
                            assert_eq!(
                                status,
                                SyncStatus::Diverge,
                                "Setting a key in stale mapping diverges the mapping",
                            );
                        }
                        SyncStatus::StorageStale => {
                            assert_eq!(
                                status,
                                SyncStatus::StorageStale,
                                "Setting a key should make storage stale"
                            );
                        }
                        SyncStatus::Diverge => {
                            assert_eq!(
                                status,
                                SyncStatus::Diverge,
                                "Setting a key in a divergent storage keeps it divergent"
                            );
                        }
                    };
                }
                StorageOperation::Remove(k) => {
                    if expected_data_in_ram.contains_key(&k) {
                        storage.remove(&k).unwrap();
                        expected_data_in_disk.remove(&k);
                        expected_data_in_ram.remove(&k);

                        *removed_data.entry(k).or_insert(0) += 1;

                        let status = storage.sync_status().unwrap();
                        match prev_status {
                            SyncStatus::InSync => {
                                assert_eq!(
                                    status,
                                    SyncStatus::StorageStale,
                                    "Removing a key should make storage stale"
                                );
                            }
                            SyncStatus::MappingStale => {
                                assert_eq!(status, SyncStatus::Diverge, "Removing a key in stale mapping diverges the mapping");
                            }
                            SyncStatus::StorageStale => {
                                if removed_data == setted_data {
                                    assert_eq!(status, SyncStatus::InSync, "Removing a key should make storage in sync when there is no data left");
                                } else {
                                    assert_eq!(status, SyncStatus::StorageStale, "Removing a key should keep storage stale");
                                }
                            }
                            SyncStatus::Diverge => {
                                assert_eq!(status, SyncStatus::Diverge, "Removing a key in a divergent storage keeps it divergent");
                            }
                        };
                    }
                }
                StorageOperation::Sync => {
                    storage.sync().unwrap();
                    expected_data_in_ram.append(&mut expected_data_in_disk);
                    expected_data_in_disk = expected_data_in_ram.clone();
                    removed_data.clear();
                    setted_data.clear();
                    assert_eq!(&storage.data.entries, &expected_data_in_ram, "In-memory mapping should match expected data after sync");
                    assert_eq!(
                        storage.sync_status().unwrap(),
                        SyncStatus::InSync,
                        "Status should be InSync after sync operation"
                    );
                }
                StorageOperation::ExternalModify(k) => {
                    if expected_data_in_ram.contains_key(&k) {
                        let _ = perform_external_modification(&path, &k, v)
                            .unwrap();
                        expected_data_in_disk.insert(k, v);

                        let status = storage.sync_status().unwrap();
                        match prev_status {
                            SyncStatus::InSync => {
                                assert_eq!(status, SyncStatus::MappingStale, "External modification when InSync should make memory stale");
                            }
                            SyncStatus::MappingStale => {
                                assert_eq!(status, SyncStatus::MappingStale, "External modification should keep mapping stale");
                            }
                            SyncStatus::StorageStale => {
                                assert_eq!(status, SyncStatus::Diverge, "External modification when StorageStale should make status Diverge");
                            }
                            SyncStatus::Diverge => {
                                assert_eq!(status, SyncStatus::Diverge, "External modification should keep status Diverge");
                            }
                        };
                    }
                }
                StorageOperation::ExternalSet(k) => {
                    let _ =
                        perform_external_modification(&path, &k, v).unwrap();
                    expected_data_in_disk.insert(k, v);

                    let status = storage.sync_status().unwrap();
                    match prev_status {
                        SyncStatus::InSync => {
                            assert_eq!(status, SyncStatus::MappingStale, "External set when InSync should make memory stale");
                        }
                        SyncStatus::MappingStale => {
                            assert_eq!(
                                status,
                                SyncStatus::MappingStale,
                                "External set should keep mapping stale"
                            );
                        }
                        SyncStatus::StorageStale => {
                            assert_eq!(status, SyncStatus::Diverge, "External set when StorageStale should make status Diverge");
                        }
                        SyncStatus::Diverge => {
                            assert_eq!(
                                status,
                                SyncStatus::Diverge,
                                "External set should keep status Diverge"
                            );
                        }
                    };
                }
            }
        }
    }

    fn perform_external_modification(
        path: &std::path::Path,
        key: &str,
        value: Dummy,
    ) -> Result<()> {
        let mut file = File::create(path.join(format!("{}.bin", key)))?;
        let bytes = bincode::serialize(&value).unwrap();
        file.write_all(&bytes)?;
        let time = SystemTime::now();
        file.set_modified(time).unwrap();
        file.sync_all()?;
        Ok(())
    }
}
