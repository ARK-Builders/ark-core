use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use itertools::Itertools;

use canonical_path::CanonicalPathBuf;
use walkdir::{DirEntry, WalkDir};

use anyhow::Error;
use log;

use crate::id::ResourceId;
use crate::meta::ResourceMeta;
use crate::{INDEX_PATH, STORAGES_FOLDER};

#[derive(Debug)]
pub struct ResourceIndex {
    pub path2meta: HashMap<CanonicalPathBuf, ResourceMeta>,
    pub collisions: HashMap<ResourceId, usize>,
    ids: HashSet<ResourceId>,
    root: PathBuf,
}

#[derive(Debug)]
pub struct IndexUpdate {
    pub deleted: HashSet<ResourceId>,
    pub added: HashMap<CanonicalPathBuf, ResourceMeta>,
}

pub const INDEX_ENTRY_DELIMITER: char = ' ';

impl ResourceIndex {
    pub fn size(&self) -> usize {
        //the actual size is lower in presence of collisions
        self.path2meta.len()
    }

    pub fn load<P: AsRef<Path>>(root_path: P) -> Result<Self, Error> {
        log::info!("Loading the index from file");

        let mut index = ResourceIndex {
            path2meta: HashMap::new(),
            collisions: HashMap::new(),
            ids: HashSet::new(),
            root: root_path.as_ref().to_owned(),
        };

        let index_path = root_path
            .as_ref()
            .to_owned()
            .join(STORAGES_FOLDER)
            .join(INDEX_PATH);

        if let Ok(file) = File::open(&index_path) {
            for line in BufReader::new(file).lines() {
                if let Ok(entry) = line {
                    let mut parts = entry.split(INDEX_ENTRY_DELIMITER);
                    let meta = ResourceMeta::load(parts.next().unwrap());

                    let delimiter = INDEX_ENTRY_DELIMITER.to_string();
                    let path: String = parts.intersperse(&delimiter).collect();
                    //todo: the path must relative to the root

                    log::trace!("[index] {:?} -> {}", meta, path);
                }
            }
        } else {
            log::info!(
                "No persisted index was found by path {:?}",
                index_path.clone()
            );
        }

        return Ok(index);
    }

    pub fn build<P: AsRef<Path>>(root_path: P) -> Result<Self, Error> {
        log::info!("Building the index from scratch");

        let paths = discover_paths(root_path.as_ref().to_owned());
        let metadata = scan_metadata(paths);

        let mut index = ResourceIndex {
            path2meta: HashMap::new(),
            collisions: HashMap::new(),
            ids: HashSet::new(),
            root: root_path.as_ref().to_owned(),
        };

        for (path, meta) in metadata {
            add_meta(
                path,
                meta,
                &mut index.path2meta,
                &mut index.collisions,
                &mut index.ids,
            );
        }

        log::info!("Index built");
        return Ok(index);
    }

    //todo: automatic test verifying that forall f, build(f) = build2(f)
    pub fn build2<P: AsRef<Path>>(root_path: P) -> Result<Self, Error> {
        log::info!("Building the index using persisted index");
        //todo: if persisted index was not found, we should call build() since it's faster

        let loaded = ResourceIndex::load(root_path);
        log::debug!("Index loaded:{:?}", loaded);

        if let Ok(mut index) = loaded {
            index.update().unwrap(); //todo
            return Ok(index);
        } else {
            return loaded;
        }
    }

    pub fn update(&mut self) -> Result<IndexUpdate, Error> {
        log::info!("Updating the index");
        log::trace!("Known paths:\n{:?}", self.path2meta.keys());

        let curr_entries = discover_paths(self.root.clone());

        //assuming that collections manipulation is
        // quicker than asking `path.exists()` for every path
        let curr_paths: Paths = curr_entries.keys().cloned().collect();
        let prev_paths: Paths = self.path2meta.keys().cloned().collect();
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

        log::info!("Checking updated paths");
        let updated_paths: HashMap<CanonicalPathBuf, DirEntry> = curr_entries
            .into_iter()
            .filter(|(path, entry)| {
                if !preserved_paths.contains(path.as_canonical_path()) {
                    false
                } else {
                    let prev_modified = self.path2meta[path].modified;

                    let result = entry.metadata();
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
                            Ok(curr_modified) => curr_modified > prev_modified,
                        },
                    }
                }
            })
            .collect();

        let mut deleted: HashSet<ResourceId> = HashSet::new();

        // treating deleted and updated paths as deletions
        prev_paths
            .difference(&preserved_paths)
            .cloned()
            .chain(updated_paths.keys().cloned())
            .for_each(|path| {
                if let Some(meta) = self.path2meta.remove(&path) {
                    let k = self.collisions.remove(&meta.id).unwrap_or(1);
                    if k > 1 {
                        self.collisions.insert(meta.id, k - 1);
                    } else {
                        log::debug!("Removing {:?} from index", meta.id);
                        self.ids.remove(&meta.id);
                        deleted.insert(meta.id);
                    }
                } else {
                    log::warn!("Path {} was not known", path.display());
                }
            });

        let added: HashMap<CanonicalPathBuf, ResourceMeta> =
            scan_metadata(updated_paths)
                .into_iter()
                .chain({
                    log::info!("The same for new paths");
                    scan_metadata(created_paths).into_iter()
                })
                .filter(|(_, meta)| !self.ids.contains(&meta.id))
                .collect();

        for (path, meta) in added.iter() {
            if deleted.contains(&meta.id) {
                // emitting the resource as both deleted and added
                // (renaming a duplicate might remain undetected)
                log::info!(
                    "Resource {:?} was moved to {}",
                    meta.id,
                    path.display()
                );
            }

            add_meta(
                path.clone(),
                meta.clone(),
                &mut self.path2meta,
                &mut self.collisions,
                &mut self.ids,
            );
        }

        Ok(IndexUpdate { deleted, added })
    }
}

fn discover_paths<P: AsRef<Path>>(
    root_path: P,
) -> HashMap<CanonicalPathBuf, DirEntry> {
    log::info!(
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

fn scan_metadata(
    entries: HashMap<CanonicalPathBuf, DirEntry>,
) -> HashMap<CanonicalPathBuf, ResourceMeta> {
    log::info!("Scanning metadata");

    entries
        .into_iter()
        .filter_map(|(path, entry)| {
            log::trace!("\n\t{:?}\n\t\t{:?}", path, entry);

            let result = ResourceMeta::scan(path.clone(), entry);
            match result {
                Err(msg) => {
                    log::error!(
                        "Couldn't retrieve metadata for {}:\n{}",
                        path.display(),
                        msg
                    );
                    None
                }
                Ok(meta) => Some(meta),
            }
        })
        .collect()
}

fn add_meta(
    path: CanonicalPathBuf,
    meta: ResourceMeta,
    path2meta: &mut HashMap<CanonicalPathBuf, ResourceMeta>,
    collisions: &mut HashMap<ResourceId, usize>,
    ids: &mut HashSet<ResourceId>,
) {
    log::debug!("Adding new entry to index:{:?} -> {:?}", path, meta);

    let id = meta.id.clone();
    path2meta.insert(path, meta);

    if ids.contains(&id) {
        if let Some(nonempty) = collisions.get_mut(&id) {
            *nonempty += 1;
        } else {
            collisions.insert(id, 2);
        }
    } else {
        ids.insert(id.clone());
    }
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

type Paths = HashSet<CanonicalPathBuf>;

#[cfg(test)]
mod tests {
    use crate::id::ResourceId;
    use crate::index::discover_paths;
    use crate::ResourceIndex;
    use canonical_path::CanonicalPathBuf;
    use std::fs::{File, Permissions};
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
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

            let actual = ResourceIndex::build(path.clone())
                .expect("Could not build index");

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2meta.len(), 1);
            assert_eq!(actual.ids.len(), 1);
            assert!(actual.ids.contains(&ResourceId {
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

            let actual = ResourceIndex::build(path.clone())
                .expect("Could not build index");

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2meta.len(), 2);
            assert_eq!(actual.ids.len(), 1);
            assert!(actual.ids.contains(&ResourceId {
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

            let mut actual = ResourceIndex::build(path.clone())
                .expect("Could not build index");

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
                .update()
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

            let mut actual = ResourceIndex::build(path.clone())
                .expect("Could not build index");

            let (_, expected_path) =
                create_file_at(path.clone(), Some(FILE_SIZE_2), None);

            let update = actual
                .update()
                .expect("Should update index correctly");

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2meta.len(), 2);
            assert_eq!(actual.ids.len(), 2);
            assert!(actual.ids.contains(&ResourceId {
                data_size: FILE_SIZE_1,
                crc32: CRC32_1,
            }));
            assert!(actual.ids.contains(&ResourceId {
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
                    .id,
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

            let mut actual = ResourceIndex::build(path.clone())
                .expect("Could not build index");

            let mut file_path = path.clone();
            file_path.push(FILE_NAME_1);
            std::fs::remove_file(file_path)
                .expect("Should remove file successfully");

            let update = actual
                .update()
                .expect("Should update index successfully");

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2meta.len(), 0);
            assert_eq!(actual.ids.len(), 0);
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

            let mut actual = ResourceIndex::build(path.clone())
                .expect("Could not build index");

            assert_eq!(actual.collisions.len(), 0);
            assert_eq!(actual.size(), 2);

            file.set_permissions(Permissions::from_mode(0o222))
                .expect("Should be fine");

            let update = actual
                .update()
                .expect("Should update index correctly");

            assert_eq!(actual.collisions.len(), 0);
            assert_eq!(actual.size(), 2);
            assert_eq!(update.deleted.len(), 0);
            assert_eq!(update.added.len(), 0);
        })
    }

    // error cases

    #[test]
    fn should_not_index_empty_file() {
        run_test_and_clean_up(|path| {
            create_file_at(path.clone(), Some(0), None);
            let actual = ResourceIndex::build(path.clone())
                .expect("Could not generate index");

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2meta.len(), 0);
            assert_eq!(actual.ids.len(), 0);
            assert_eq!(actual.collisions.len(), 0);
        })
    }

    #[test]
    fn should_not_index_hidden_file() {
        run_test_and_clean_up(|path| {
            create_file_at(path.clone(), Some(FILE_SIZE_1), Some(".hidden"));
            let actual = ResourceIndex::build(path.clone())
                .expect("Could not generate index");

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2meta.len(), 0);
            assert_eq!(actual.ids.len(), 0);
            assert_eq!(actual.collisions.len(), 0);
        })
    }

    #[test]
    fn should_not_index_1_empty_directory() {
        run_test_and_clean_up(|path| {
            create_dir_at(path.clone());

            let actual = ResourceIndex::build(path.clone())
                .expect("Could not build index");

            assert_eq!(actual.root, path.clone());
            assert_eq!(actual.path2meta.len(), 0);
            assert_eq!(actual.ids.len(), 0);
            assert_eq!(actual.collisions.len(), 0);
        })
    }

    #[test]
    fn should_fail_when_indexing_file_without_read_rights() {
        run_test_and_clean_up(|path| {
            let (file, _) = create_file_at(path.clone(), Some(1), None);
            file.set_permissions(Permissions::from_mode(0o222))
                .expect("Should be fine");

            let actual =
                std::panic::catch_unwind(|| ResourceIndex::build(path.clone()));
            assert!(actual.is_err());
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
}
