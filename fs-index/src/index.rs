use anyhow::anyhow;
use canonical_path::{CanonicalPath, CanonicalPathBuf};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, Metadata};
use std::io::{BufRead, BufReader, Write};
use std::ops::Add;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use walkdir::{DirEntry, WalkDir};

use log;

use data_error::{ArklibError, Result};
use data_resource::ResourceId;
use fs_storage::{ARK_FOLDER, INDEX_PATH};

#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Debug)]
pub struct IndexEntry<Id: ResourceId> {
    pub modified: SystemTime,
    pub id: Id,
}

#[derive(PartialEq, Clone, Debug)]
pub struct ResourceIndex<Id: ResourceId> {
    pub id2path: HashMap<Id, CanonicalPathBuf>,
    pub path2id: HashMap<CanonicalPathBuf, IndexEntry<Id>>,

    pub collisions: HashMap<Id, usize>,
    pub root: PathBuf,
}

#[derive(PartialEq, Debug)]
pub struct IndexUpdate<Id: ResourceId> {
    pub deleted: HashSet<Id>,
    pub added: HashMap<CanonicalPathBuf, Id>,
}

pub const RESOURCE_UPDATED_THRESHOLD: Duration = Duration::from_millis(1);

pub type Paths = HashSet<CanonicalPathBuf>;

impl<Id: ResourceId> ResourceIndex<Id> {
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
        let lines = BufReader::new(file).lines();
        for line in lines {
            let line = line?;

            let mut parts = line.split(' ');

            let modified = {
                let str = parts.next().ok_or(ArklibError::Parse)?;
                UNIX_EPOCH.add(Duration::from_millis(
                    str.parse().map_err(|_| ArklibError::Parse)?,
                ))
            };

            let id = {
                let str = parts.next().ok_or(ArklibError::Parse)?;
                Id::from_str(str).map_err(|_| ArklibError::Parse)?
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

        let mut path2id: Vec<(&CanonicalPathBuf, &IndexEntry<Id>)> =
            self.path2id.iter().collect();
        path2id.sort_by_key(|(_, entry)| *entry);

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

            writeln!(file, "{} {} {}", timestamp, entry.id, path.display())?;
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

    pub fn update_all(&mut self) -> Result<IndexUpdate<Id>> {
        log::debug!("Updating the index");
        log::trace!("[update] known paths: {:?}", self.path2id.keys());

        let mut added = HashMap::new();
        let mut deleted = HashSet::new();

        let new_index: ResourceIndex<Id> = ResourceIndex::build(&self.root);
        for (path, entry) in new_index.path2id.iter() {
            if !self.path2id.contains_key(path) {
                added.insert(path.clone(), entry.id.clone());
            }
        }
        for (path, entry) in self.path2id.iter() {
            if !new_index.path2id.contains_key(path) {
                deleted.insert(entry.id.clone());
            }
        }

        *self = new_index;
        Ok(IndexUpdate { added, deleted })
    }

    // the caller must ensure that:
    // * the index is up-to-date except this single path
    // * the path hasn't been indexed before
    pub fn index_new(
        &mut self,
        path: &dyn AsRef<Path>,
    ) -> Result<IndexUpdate<Id>> {
        log::debug!("Indexing a new path");

        if !path.as_ref().exists() {
            return Err(ArklibError::Path(
                "Absent paths cannot be indexed".into(),
            ));
        }

        let path_buf = CanonicalPathBuf::canonicalize(path)?;
        let path = path_buf.as_canonical_path();

        return match fs::metadata(path) {
            Err(_) => {
                return Err(ArklibError::Path(
                    "Couldn't to retrieve file metadata".into(),
                ));
            }
            Ok(metadata) => match scan_entry(path, metadata) {
                Err(_) => {
                    return Err(ArklibError::Path(
                        "The path points to a directory or empty file".into(),
                    ));
                }
                Ok(new_entry) => {
                    let id = new_entry.clone().id;

                    if let Some(nonempty) = self.collisions.get_mut(&id) {
                        *nonempty += 1;
                    }

                    let mut added = HashMap::new();
                    added.insert(path_buf.clone(), id.clone());

                    self.id2path.insert(id, path_buf.clone());
                    self.path2id.insert(path_buf, new_entry);

                    Ok(IndexUpdate {
                        added,
                        deleted: HashSet::new(),
                    })
                }
            },
        };
    }

    // the caller must ensure that:
    // * the index is up-to-date except this single path
    // * the path has been indexed before
    // * the path maps into `old_id`
    // * the content by the path has been modified
    pub fn update_one(
        &mut self,
        path: &dyn AsRef<Path>,
        old_id: Id,
    ) -> Result<IndexUpdate<Id>> {
        log::debug!("Updating a single entry in the index");

        if !path.as_ref().exists() {
            return self.forget_id(old_id);
        }

        let path_buf = CanonicalPathBuf::canonicalize(path)?;
        let path = path_buf.as_canonical_path();

        log::trace!(
            "[update] paths {:?} has id {:?}",
            path,
            self.path2id[path]
        );

        return match fs::metadata(path) {
            Err(_) => {
                // updating the index after resource removal
                // is a correct scenario
                self.forget_path(path, old_id)
            }
            Ok(metadata) => {
                match scan_entry(path, metadata) {
                    Err(_) => {
                        // a directory or empty file exists by the path
                        self.forget_path(path, old_id)
                    }
                    Ok(new_entry) => {
                        // valid resource exists by the path

                        let curr_entry = &self.path2id.get(path);
                        if curr_entry.is_none() {
                            // if the path is not indexed, then we can't have
                            // `old_id` if you want
                            // to index new path, use `index_new` method
                            return Err(ArklibError::Path(
                                "Couldn't find the path in the index".into(),
                            ));
                        }
                        let curr_entry = curr_entry.unwrap();

                        if curr_entry.id == new_entry.id {
                            // in rare cases we are here due to hash collision
                            if curr_entry.modified == new_entry.modified {
                                log::warn!("path {:?} was not modified", &path);
                            } else {
                                log::warn!("path {:?} was modified but not its content", &path);
                            }

                            // the caller must have ensured that the path was
                            // indeed update
                            return Err(ArklibError::Collision(
                                "New content has the same id".into(),
                            ));
                        }

                        // new resource exists by the path
                        self.forget_path(path, old_id).map(|mut update| {
                            update
                                .added
                                .insert(path_buf.clone(), new_entry.clone().id);
                            self.insert_entry(path_buf, new_entry);

                            update
                        })
                    }
                }
            }
        };
    }

    pub fn forget_id(&mut self, old_id: Id) -> Result<IndexUpdate<Id>> {
        let old_path = self
            .path2id
            .drain()
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

        Ok(IndexUpdate {
            added: HashMap::new(),
            deleted,
        })
    }

    fn insert_entry(&mut self, path: CanonicalPathBuf, entry: IndexEntry<Id>) {
        log::trace!("[add] {} by path {}", entry.id, path.display());
        let id = entry.clone().id;

        if let std::collections::hash_map::Entry::Vacant(e) =
            self.id2path.entry(id.clone())
        {
            e.insert(path.clone());
        } else if let Some(nonempty) = self.collisions.get_mut(&id) {
            *nonempty += 1;
        } else {
            self.collisions.insert(id, 2);
        }

        self.path2id.insert(path, entry);
    }

    fn forget_path(
        &mut self,
        path: &CanonicalPath,
        old_id: Id,
    ) -> Result<IndexUpdate<Id>> {
        self.path2id.remove(path);

        if let Some(collisions) = self.collisions.get_mut(&old_id) {
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
                let old_path = self
                    .id2path
                    .insert(old_id.clone(), collided_path.clone());

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
            self.id2path.remove(&old_id.clone());
        }

        let mut deleted = HashSet::new();
        deleted.insert(old_id);

        Ok(IndexUpdate {
            added: HashMap::new(),
            deleted,
        })
    }
}

pub(crate) fn discover_paths<P: AsRef<Path>>(
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

fn scan_entry<Id>(
    path: &CanonicalPath,
    metadata: Metadata,
) -> Result<IndexEntry<Id>>
where
    Id: ResourceId,
{
    if metadata.is_dir() {
        return Err(ArklibError::Path("Path is expected to be a file".into()));
    }

    let size = metadata.len();
    if size == 0 {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Empty resource",
        ))?;
    }

    let id = Id::from_path(path)?;
    let modified = metadata.modified()?;

    Ok(IndexEntry { id, modified })
}

fn scan_entries<Id>(
    entries: HashMap<CanonicalPathBuf, DirEntry>,
) -> HashMap<CanonicalPathBuf, IndexEntry<Id>>
where
    Id: ResourceId,
{
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
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}
