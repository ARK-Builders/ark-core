use anyhow::anyhow;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, Metadata};
use std::io::{BufRead, BufReader, Write};
use std::ops::Add;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use canonical_path::{CanonicalPath, CanonicalPathBuf};
use walkdir::{DirEntry, WalkDir};

use log;

use crate::id::ResourceId;
use crate::{ArklibError, Result, ARK_FOLDER, INDEX_PATH};

#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Debug)]
pub struct IndexEntry {
    pub modified: SystemTime,
    pub id: ResourceId,
}

#[derive(Clone, Debug)]
pub struct ResourceIndex {
    pub id2path: HashMap<ResourceId, CanonicalPathBuf>,
    pub path2id: HashMap<CanonicalPathBuf, IndexEntry>,

    pub collisions: HashMap<ResourceId, usize>,
    root: PathBuf,
}

#[derive(Debug)]
pub struct IndexUpdate {
    pub deleted: HashSet<ResourceId>,
    pub added: HashMap<CanonicalPathBuf, ResourceId>,
}

pub const RESOURCE_UPDATED_THRESHOLD: Duration = Duration::from_millis(1);

type Paths = HashSet<CanonicalPathBuf>;

impl ResourceIndex {
    pub fn size(&self) -> usize {
        //the actual size is lower in presence of collisions
        self.path2id.len()
    }

    pub fn build<P: AsRef<Path>>(root_path: P) -> Self {
        log::info!("Building the index from scratch");
        let root_path: PathBuf = root_path.as_ref().to_owned();

        let entries = discover_paths(&root_path);
        let entries = scan_entries(entries);

        let mut index = ResourceIndex {
            id2path: HashMap::new(),
            path2id: HashMap::new(),
            collisions: HashMap::new(),
            root: root_path,
        };

        for (path, entry) in entries {
            index.insert_entry(path, entry);
        }

        log::info!("Index built");
        index
    }

    pub fn load<P: AsRef<Path>>(root_path: P) -> Result<Self> {
        let root_path: PathBuf = root_path.as_ref().to_owned();

        let index_path: PathBuf = root_path.join(ARK_FOLDER).join(INDEX_PATH);
        log::info!("Loading the index from file {}", index_path.display());
        let file = File::open(&index_path)?;
        let mut index = ResourceIndex {
            id2path: HashMap::new(),
            path2id: HashMap::new(),
            collisions: HashMap::new(),
            root: root_path.clone(),
        };

        // We should not return early in case of missing files
        for line in BufReader::new(file).lines() {
            if let Ok(entry) = line {
                let mut parts = entry.split(' ');

                let modified = {
                    let str = parts.next().ok_or(ArklibError::Parse)?;
                    UNIX_EPOCH.add(Duration::from_millis(
                        str.parse().map_err(|_| ArklibError::Parse)?,
                    ))
                };

                let id = {
                    let str = parts.next().ok_or(ArklibError::Parse)?;
                    ResourceId::from_str(str)?
                };

                let path: String =
                    itertools::Itertools::intersperse(parts, " ").collect();
                let path: PathBuf = root_path.join(Path::new(&path));
                match CanonicalPathBuf::canonicalize(&path) {
                    Ok(path) => {
                        log::trace!("[load] {} -> {}", id, path.display());
                        index.insert_entry(path, IndexEntry { id, modified });
                    }
                    Err(_) => {
                        log::warn!("File {} not found", path.display());
                        continue;
                    }
                }
            }
        }

        Ok(index)
    }

    pub fn store(&self) -> Result<()> {
        log::info!("Storing the index to file");

        let start = SystemTime::now();

        let index_path = self
            .root
            .to_owned()
            .join(ARK_FOLDER)
            .join(INDEX_PATH);

        let ark_dir = index_path.parent().unwrap();
        fs::create_dir_all(ark_dir)?;

        let mut file = File::create(index_path)?;

        let mut path2id: Vec<(&CanonicalPathBuf, &IndexEntry)> =
            self.path2id.iter().collect();
        path2id.sort_by_key(|(_, entry)| entry.clone());

        for (path, entry) in path2id.iter() {
            log::trace!("[store] {} by path {}", entry.id, path.display());

            let timestamp = entry
                .modified
                .duration_since(UNIX_EPOCH)
                .map_err(|_| {
                    ArklibError::Other(anyhow!("Error using duration since"))
                })?
                .as_millis();

            let path =
                pathdiff::diff_paths(path.to_str().unwrap(), self.root.clone())
                    .ok_or(ArklibError::Path(
                        "Couldn't calculate path diff".into(),
                    ))?;

            write!(file, "{} {} {}\n", timestamp, entry.id, path.display())?;
        }

        log::trace!(
            "Storing the index took {:?}",
            start
                .elapsed()
                .map_err(|_| ArklibError::Other(anyhow!("SystemTime error")))
        );
        Ok(())
    }

    pub fn provide<P: AsRef<Path>>(root_path: P) -> Result<Self> {
        match Self::load(&root_path) {
            Ok(mut index) => {
                log::debug!("Index loaded: {} entries", index.path2id.len());

                match index.update_all() {
                    Ok(update) => {
                        log::debug!(
                            "Index updated: {} added, {} deleted",
                            update.added.len(),
                            update.deleted.len()
                        );
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to update index: {}",
                            e.to_string()
                        );
                    }
                }

                if let Err(e) = index.store() {
                    log::error!("{}", e.to_string());
                }
                Ok(index)
            }
            Err(e) => {
                log::warn!("{}", e.to_string());
                Ok(Self::build(root_path))
            }
        }
    }

    pub fn update_all(&mut self) -> Result<IndexUpdate> {
        log::debug!("Updating the index");
        log::trace!("[update] known paths: {:?}", self.path2id.keys());

        let curr_entries = discover_paths(self.root.clone());

        //assuming that collections manipulation is
        // quicker than asking `path.exists()` for every path
        let curr_paths: Paths = curr_entries.keys().cloned().collect();
        let prev_paths: Paths = self.path2id.keys().cloned().collect();
        let preserved_paths: Paths = curr_paths
            .intersection(&prev_paths)
            .cloned()
            .collect();

        let created_paths: HashMap<CanonicalPathBuf, DirEntry> = curr_entries
            .iter()
            .filter_map(|(path, entry)| {
                if !preserved_paths.contains(path.as_canonical_path()) {
                    Some((path.clone(), entry.clone()))
                } else {
                    None
                }
            })
            .collect();

        log::debug!("Checking updated paths");
        let updated_paths: HashMap<CanonicalPathBuf, DirEntry> = curr_entries
            .into_iter()
            .filter(|(path, dir_entry)| {
                if !preserved_paths.contains(path.as_canonical_path()) {
                    false
                } else {
                    let our_entry = &self.path2id[path];
                    let prev_modified = our_entry.modified;

                    let result = dir_entry.metadata();
                    match result {
                        Err(msg) => {
                            log::error!(
                                "Couldn't retrieve metadata for {}: {}",
                                &path.display(),
                                msg
                            );
                            false
                        }
                        Ok(metadata) => match metadata.modified() {
                            Err(msg) => {
                                log::error!(
                                    "Couldn't retrieve timestamp for {}: {}",
                                    &path.display(),
                                    msg
                                );
                                false
                            }
                            Ok(curr_modified) => {
                                let elapsed = curr_modified
                                    .duration_since(prev_modified)
                                    .unwrap();

                                let was_updated =
                                    elapsed >= RESOURCE_UPDATED_THRESHOLD;
                                if was_updated {
                                    log::trace!(
                                        "[update] modified {} by path {}
                                        \twas {:?}
                                        \tnow {:?}
                                        \telapsed {:?}",
                                        our_entry.id,
                                        path.display(),
                                        prev_modified,
                                        curr_modified,
                                        elapsed
                                    );
                                }

                                was_updated
                            }
                        },
                    }
                }
            })
            .collect();

        let mut deleted: HashSet<ResourceId> = HashSet::new();

        // treating both deleted and updated paths as deletions
        prev_paths
            .difference(&preserved_paths)
            .cloned()
            .chain(updated_paths.keys().cloned())
            .for_each(|path| {
                if let Some(entry) =
                    self.path2id.remove(path.as_canonical_path())
                {
                    let k = self.collisions.remove(&entry.id).unwrap_or(1);
                    if k > 1 {
                        self.collisions.insert(entry.id, k - 1);
                    } else {
                        log::trace!(
                            "[delete] {} by path {}",
                            entry.id,
                            path.display()
                        );
                        self.id2path.remove(&entry.id);
                        deleted.insert(entry.id);
                    }
                } else {
                    log::warn!("Path {} was not known", path.display());
                }
            });

        let added: HashMap<CanonicalPathBuf, IndexEntry> =
            scan_entries(updated_paths)
                .into_iter()
                .chain({
                    log::debug!("Checking added paths");
                    scan_entries(created_paths).into_iter()
                })
                .filter(|(_, entry)| !self.id2path.contains_key(&entry.id))
                .collect();

        for (path, entry) in added.iter() {
            if deleted.contains(&entry.id) {
                // emitting the resource as both deleted and added
                // (renaming a duplicate might remain undetected)
                log::trace!(
                    "[update] moved {} to path {}",
                    entry.id,
                    path.display()
                );
            }

            self.insert_entry(path.clone(), entry.clone());
        }

        let added: HashMap<CanonicalPathBuf, ResourceId> = added
            .into_iter()
            .map(|(path, entry)| (path, entry.id))
            .collect();

        Ok(IndexUpdate { deleted, added })
    }

    pub fn update_one(
        &mut self,
        path: &dyn AsRef<Path>,
        old_id: ResourceId,
    ) -> Result<IndexUpdate> {
        log::debug!("Updating a single entry in the index");

        if !path.as_ref().exists() {
            return self.forget_id(old_id);
        }

        let canonical_path_buf = CanonicalPathBuf::canonicalize(path)?;
        let canonical_path = canonical_path_buf.as_canonical_path();

        log::trace!(
            "[update] paths {:?} has id {:?}",
            canonical_path,
            self.path2id[canonical_path]
        );

        return match fs::metadata(canonical_path) {
            Err(_) => {
                // updating the index after resource removal is a correct
                // scenario
                self.forget_path(canonical_path, old_id)
            }
            Ok(metadata) => {
                match scan_entry(canonical_path, metadata) {
                    Err(_) => {
                        // a directory or empty file exists by the path
                        self.forget_path(canonical_path, old_id)
                    }
                    Ok(new_entry) => {
                        // valid resource exists by the path
                        let curr_entry = &self.path2id[canonical_path];

                        if curr_entry.id == new_entry.id {
                            // in rare cases we are here due to hash collision

                            if curr_entry.modified == new_entry.modified {
                                log::warn!("path {:?} was modified but not its content", &canonical_path);
                            }

                            // the caller must have ensured that the path was
                            // updated
                            return Err(ArklibError::Collision(
                                "New content has the same id".into(),
                            ));
                        }

                        // new resource exists by the path
                        self.forget_path(canonical_path, old_id).map(
                            |mut update| {
                                update.added.insert(
                                    canonical_path_buf.clone(),
                                    new_entry.id,
                                );
                                self.insert_entry(
                                    canonical_path_buf,
                                    new_entry,
                                );

                                update
                            },
                        )
                    }
                }
            }
        };
    }

    pub fn forget_id(&mut self, old_id: ResourceId) -> Result<IndexUpdate> {
        let old_path = self
            .path2id
            .drain()
            .into_iter()
            .filter_map(|(k, v)| {
                if v.id == old_id {
                    Some(k)
                } else {
                    None
                }
            })
            .collect_vec();
        for p in old_path {
            self.path2id.remove(&p);
        }
        self.id2path.remove(&old_id);
        let mut deleted = HashSet::new();
        deleted.insert(old_id);

        return Ok(IndexUpdate {
            added: HashMap::new(),
            deleted,
        });
    }

    fn insert_entry(&mut self, path: CanonicalPathBuf, entry: IndexEntry) {
        log::trace!("[add] {} by path {}", entry.id, path.display());
        let id = entry.id;

        if self.id2path.contains_key(&id) {
            if let Some(nonempty) = self.collisions.get_mut(&id) {
                *nonempty += 1;
            } else {
                self.collisions.insert(id, 2);
            }
        } else {
            self.id2path.insert(id, path.clone());
        }

        self.path2id.insert(path, entry);
    }

    fn forget_path(
        &mut self,
        path: &CanonicalPath,
        old_id: ResourceId,
    ) -> Result<IndexUpdate> {
        self.path2id.remove(path);

        if let Some(mut collisions) = self.collisions.get_mut(&old_id) {
            debug_assert!(
                *collisions > 1,
                "Any collision must involve at least 2 resources"
            );
            *collisions -= 1;

            if *collisions == 1 {
                self.collisions.remove(&old_id);
            }

            // minor performance issue:
            // we must find path of one of the collided
            // resources and use it as new value
            let maybe_collided_path =
                self.path2id.iter().find_map(|(path, entry)| {
                    if entry.id == old_id {
                        Some(path)
                    } else {
                        None
                    }
                });

            if let Some(collided_path) = maybe_collided_path {
                let old_path =
                    self.id2path.insert(old_id, collided_path.clone());

                debug_assert_eq!(
                    old_path.unwrap().as_canonical_path(),
                    path,
                    "Must forget the requested path"
                );
            } else {
                return Err(ArklibError::Collision(
                    "Illegal state of collision tracker".into(),
                ));
            }
        } else {
            self.id2path.remove(&old_id);
        }

        let mut deleted = HashSet::new();
        deleted.insert(old_id);

        return Ok(IndexUpdate {
            added: HashMap::new(),
            deleted,
        });
    }
}

fn discover_paths<P: AsRef<Path>>(
    root_path: P,
) -> HashMap<CanonicalPathBuf, DirEntry> {
    log::debug!(
        "Discovering all files under path {}",
        root_path.as_ref().display()
    );

    WalkDir::new(root_path)
        .into_iter()
        .filter_entry(|entry| !is_hidden(entry))
        .filter_map(|result| match result {
            Ok(entry) => {
                let path = entry.path();
                if !entry.file_type().is_dir() {
                    match CanonicalPathBuf::canonicalize(path) {
                        Ok(canonical_path) => Some((canonical_path, entry)),
                        Err(msg) => {
                            log::warn!(
                                "Couldn't canonicalize {}:\n{}",
                                path.display(),
                                msg
                            );
                            None
                        }
                    }
                } else {
                    None
                }
            }
            Err(msg) => {
                log::error!("Error during walking: {}", msg);
                None
            }
        })
        .collect()
}

fn scan_entry(path: &CanonicalPath, metadata: Metadata) -> Result<IndexEntry> {
    if metadata.is_dir() {
        return Err(ArklibError::Path("Path is expected to be a file".into()));
    }

    let size = metadata.len();
    if size == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Empty resource",
        ))?;
    }

    let id = ResourceId::compute(size, path)?;
    let modified = metadata.modified()?;

    Ok(IndexEntry { id, modified })
}

fn scan_entries(
    entries: HashMap<CanonicalPathBuf, DirEntry>,
) -> HashMap<CanonicalPathBuf, IndexEntry> {
    entries
        .into_iter()
        .filter_map(|(path_buf, entry)| {
            let metadata = entry.metadata().ok()?;

            let path = path_buf.as_canonical_path();
            let result = scan_entry(path, metadata);
            match result {
                Err(msg) => {
                    log::error!(
                        "Couldn't retrieve metadata for {}:\n{}",
                        path.display(),
                        msg
                    );
                    None
                }
                Ok(entry) => Some((path_buf, entry)),
            }
        })
        .collect()
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use crate::id::ResourceId;
    use crate::index::{discover_paths, IndexEntry};
    use crate::ArklibError;
    use crate::ResourceIndex;
    use canonical_path::CanonicalPathBuf;
    use std::fs::{File, Permissions};
    #[cfg(target_os = "unix")]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::str::FromStr;
    use std::time::SystemTime;
    use uuid::Uuid;

    const FILE_SIZE_1: u64 = 10;
    const FILE_SIZE_2: u64 = 11;

    const FILE_NAME_1: &str = "test1.txt";
    const FILE_NAME_2: &str = "test2.txt";
    const FILE_NAME_3: &str = "test3.txt";

    const CRC32_1: u32 = 3817498742;
    const CRC32_2: u32 = 1804055020;

    fn get_temp_dir() -> PathBuf {
        create_dir_at(std::env::temp_dir())
    }

    fn create_dir_at(path: PathBuf) -> PathBuf {
        let mut dir_path = path.clone();
        dir_path.push(Uuid::new_v4().to_string());
        std::fs::create_dir(&dir_path).expect("Could not create temp dir");
        dir_path
    }

    fn create_file_at(
        path: PathBuf,
        size: Option<u64>,
        name: Option<&str>,
    ) -> (File, PathBuf) {
        let mut file_path = path.clone();
        if let Some(file_name) = name {
            file_path.push(file_name);
        } else {
            file_path.push(Uuid::new_v4().to_string());
        }
        let file = File::create(file_path.clone())
            .expect("Could not create temp file");
        file.set_len(size.unwrap_or(0))
            .expect("Could not set file size");
        (file, file_path)
    }

    fn run_test_and_clean_up(
        test: impl FnOnce(PathBuf) -> () + std::panic::UnwindSafe,
    ) -> () {
        let path = get_temp_dir();
        let result = std::panic::catch_unwind(|| test(path.clone()));
        std::fs::remove_dir_all(path.clone())
            .expect("Could not clean up after test");
        if result.is_err() {
            panic!("{}", result.err().map(|_| "Test panicked").unwrap())
        }
        assert!(result.is_ok());
    }

    // resource index build

    #[test]
    fn should_build_resource_index_with_1_file_successfully() {
        run_test_and_clean_up(|path| {
            create_file_at(path.clone(), Some(FILE_SIZE_1), None);

            let actual = ResourceIndex::build(path.clone());

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2id.len(), 1);
            assert_eq!(actual.id2path.len(), 1);
            assert!(actual.id2path.contains_key(&ResourceId {
                data_size: FILE_SIZE_1,
                crc32: CRC32_1,
            }));
            assert_eq!(actual.collisions.len(), 0);
            assert_eq!(actual.size(), 1);
        })
    }

    #[test]
    fn should_index_colliding_files_correctly() {
        run_test_and_clean_up(|path| {
            create_file_at(path.clone(), Some(FILE_SIZE_1), None);
            create_file_at(path.clone(), Some(FILE_SIZE_1), None);

            let actual = ResourceIndex::build(path.clone());

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2id.len(), 2);
            assert_eq!(actual.id2path.len(), 1);
            assert!(actual.id2path.contains_key(&ResourceId {
                data_size: FILE_SIZE_1,
                crc32: CRC32_1,
            }));
            assert_eq!(actual.collisions.len(), 1);
            assert_eq!(actual.size(), 2);
        })
    }

    // resource index update

    #[test]
    fn should_update_index_with_renamed_file_correctly() {
        run_test_and_clean_up(|path| {
            create_file_at(path.clone(), Some(FILE_SIZE_1), Some(FILE_NAME_1));
            create_file_at(path.clone(), Some(FILE_SIZE_2), Some(FILE_NAME_2));

            let mut actual = ResourceIndex::build(path.clone());

            assert_eq!(actual.collisions.len(), 0);
            assert_eq!(actual.size(), 2);

            // rename test2.txt to test3.txt
            let mut name_from = path.clone();
            name_from.push(FILE_NAME_2);
            let mut name_to = path.clone();
            name_to.push(FILE_NAME_3);
            std::fs::rename(name_from, name_to)
                .expect("Should rename file successfully");

            let update = actual
                .update_all()
                .expect("Should update index correctly");

            assert_eq!(actual.collisions.len(), 0);
            assert_eq!(actual.size(), 2);
            assert_eq!(update.deleted.len(), 1);
            assert_eq!(update.added.len(), 1);
        })
    }

    #[test]
    fn should_update_resource_index_adding_1_additional_file_successfully() {
        run_test_and_clean_up(|path| {
            create_file_at(path.clone(), Some(FILE_SIZE_1), None);

            let mut actual = ResourceIndex::build(path.clone());

            let (_, expected_path) =
                create_file_at(path.clone(), Some(FILE_SIZE_2), None);

            let update = actual
                .update_all()
                .expect("Should update index correctly");

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2id.len(), 2);
            assert_eq!(actual.id2path.len(), 2);
            assert!(actual.id2path.contains_key(&ResourceId {
                data_size: FILE_SIZE_1,
                crc32: CRC32_1,
            }));
            assert!(actual.id2path.contains_key(&ResourceId {
                data_size: FILE_SIZE_2,
                crc32: CRC32_2,
            }));
            assert_eq!(actual.collisions.len(), 0);
            assert_eq!(actual.size(), 2);
            assert_eq!(update.deleted.len(), 0);
            assert_eq!(update.added.len(), 1);

            let added_key =
                CanonicalPathBuf::canonicalize(&expected_path.clone())
                    .expect("CanonicalPathBuf should be fine");
            assert_eq!(
                update
                    .added
                    .get(&added_key)
                    .expect("Key exists")
                    .clone(),
                ResourceId {
                    data_size: FILE_SIZE_2,
                    crc32: CRC32_2
                }
            )
        })
    }

    #[test]
    fn should_update_resource_index_deleting_1_additional_file_successfully() {
        run_test_and_clean_up(|path| {
            create_file_at(path.clone(), Some(FILE_SIZE_1), Some(FILE_NAME_1));

            let mut actual = ResourceIndex::build(path.clone());

            let mut file_path = path.clone();
            file_path.push(FILE_NAME_1);
            std::fs::remove_file(file_path.clone())
                .expect("Should remove file successfully");

            let update = actual
                .update_one(
                    &file_path.clone(),
                    ResourceId {
                        data_size: FILE_SIZE_1,
                        crc32: CRC32_1,
                    },
                )
                .expect("Should update index successfully");

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2id.len(), 0);
            assert_eq!(actual.id2path.len(), 0);
            assert_eq!(actual.collisions.len(), 0);
            assert_eq!(actual.size(), 0);
            assert_eq!(update.deleted.len(), 1);
            assert_eq!(update.added.len(), 0);

            assert!(update.deleted.contains(&ResourceId {
                data_size: FILE_SIZE_1,
                crc32: CRC32_1
            }))
        })
    }

    #[test]
    fn should_not_update_index_on_files_without_permissions() {
        run_test_and_clean_up(|path| {
            create_file_at(path.clone(), Some(FILE_SIZE_1), Some(FILE_NAME_1));
            let (file, _) = create_file_at(
                path.clone(),
                Some(FILE_SIZE_2),
                Some(FILE_NAME_2),
            );

            let mut actual = ResourceIndex::build(path.clone());

            assert_eq!(actual.collisions.len(), 0);
            assert_eq!(actual.size(), 2);
            #[cfg(target_os = "unix")]
            file.set_permissions(Permissions::from_mode(0o222))
                .expect("Should be fine");

            let update = actual
                .update_all()
                .expect("Should update index correctly");

            assert_eq!(actual.collisions.len(), 0);
            assert_eq!(actual.size(), 2);
            assert_eq!(update.deleted.len(), 0);
            assert_eq!(update.added.len(), 0);
        })
    }

    // error cases

    #[test]
    fn should_not_update_nonexistent_path() {
        run_test_and_clean_up(|path| {
            let mut missing_path = path.clone();
            missing_path.push("missing/directory");
            let mut actual = ResourceIndex::build(path.clone());
            let old_id = ResourceId {
                data_size: 1,
                crc32: 2,
            };
            let result = actual
                .update_one(&missing_path, old_id)
                .map(|i| i.deleted.clone().take(&old_id))
                .ok()
                .flatten();

            assert_eq!(
                result,
                Some(ResourceId {
                    data_size: 1,
                    crc32: 2,
                })
            );
        })
    }

    #[test]
    fn should_not_index_empty_file() {
        run_test_and_clean_up(|path| {
            create_file_at(path.clone(), Some(0), None);
            let actual = ResourceIndex::build(path.clone());

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2id.len(), 0);
            assert_eq!(actual.id2path.len(), 0);
            assert_eq!(actual.collisions.len(), 0);
        })
    }

    #[test]
    fn should_not_index_hidden_file() {
        run_test_and_clean_up(|path| {
            create_file_at(path.clone(), Some(FILE_SIZE_1), Some(".hidden"));
            let actual = ResourceIndex::build(path.clone());

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2id.len(), 0);
            assert_eq!(actual.id2path.len(), 0);
            assert_eq!(actual.collisions.len(), 0);
        })
    }

    #[test]
    fn should_not_index_1_empty_directory() {
        run_test_and_clean_up(|path| {
            create_dir_at(path.clone());

            let actual = ResourceIndex::build(path.clone());

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2id.len(), 0);
            assert_eq!(actual.id2path.len(), 0);
            assert_eq!(actual.collisions.len(), 0);
        })
    }

    #[test]
    fn discover_paths_should_not_walk_on_invalid_path() {
        run_test_and_clean_up(|path| {
            let mut missing_path = path.clone();
            missing_path.push("missing/directory");
            let actual = discover_paths(missing_path);
            assert_eq!(actual.len(), 0);
        })
    }

    #[test]
    fn index_entry_order() {
        let old1 = IndexEntry {
            id: ResourceId {
                data_size: 1,
                crc32: 2,
            },
            modified: SystemTime::UNIX_EPOCH,
        };
        let old2 = IndexEntry {
            id: ResourceId {
                data_size: 2,
                crc32: 1,
            },
            modified: SystemTime::UNIX_EPOCH,
        };

        let new1 = IndexEntry {
            id: ResourceId {
                data_size: 1,
                crc32: 1,
            },
            modified: SystemTime::now(),
        };
        let new2 = IndexEntry {
            id: ResourceId {
                data_size: 1,
                crc32: 2,
            },
            modified: SystemTime::now(),
        };

        assert!(new1 == new1);
        assert!(new2 == new2);
        assert!(old1 == old1);
        assert!(old2 == old2);

        assert!(new1 != new2);
        assert!(new1 != old1);

        assert!(old2 > old1);
        assert!(new1 > old1);
        assert!(new1 > old2);
        assert!(new2 > old1);
        assert!(new2 > old2);
        assert!(new2 > new1);
    }
}
