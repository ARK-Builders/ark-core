use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use canonical_path::CanonicalPathBuf;
use walkdir::{DirEntry, WalkDir};

use anyhow::Error;
use log;

use crate::id::ResourceId;
use crate::meta::ResourceMeta;

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

impl ResourceIndex {
    pub fn size(&self) -> usize {
        //the actual size is lower in presence of collisions
        self.path2meta.len()
    }

    pub fn build<P: AsRef<Path>>(root_path: P) -> Result<Self, Error> {
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

    pub fn update(&mut self) -> Result<IndexUpdate, Error> {
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
                                    "Couldn't retrieve metadata for {}: {}",
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

        let deleted: HashSet<ResourceId> = prev_paths
            .difference(&preserved_paths)
            .cloned()
            .map(|path| self.path2meta[&path].id.clone())
            .chain(
                updated_paths
                    .keys()
                    .map(|path| self.path2meta[path].id.clone()),
            )
            .collect();

        let added: HashMap<CanonicalPathBuf, ResourceMeta> =
            scan_metadata(updated_paths)
                .into_iter()
                .filter(|(_, meta)| !self.ids.contains(&meta.id))
                .chain(scan_metadata(created_paths).into_iter())
                .collect();

        let mut path2meta = HashMap::new();
        let mut collisions: HashMap<ResourceId, usize> = HashMap::new();
        let mut ids: HashSet<ResourceId> = HashSet::new();

        for (path, meta) in self.path2meta.iter() {
            if !deleted.contains(&meta.id) {
                add_meta(
                    path.clone(),
                    meta.clone(),
                    &mut path2meta,
                    &mut collisions,
                    &mut ids,
                );
            }
        }

        for (path, meta) in added.iter() {
            add_meta(
                path.clone(),
                meta.clone(),
                &mut path2meta,
                &mut collisions,
                &mut ids,
            );
        }

        self.path2meta = path2meta;
        self.collisions = collisions;
        self.ids = ids;

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
                match CanonicalPathBuf::canonicalize(path) {
                    Ok(canonical_path) => Some((canonical_path, entry)),
                    Err(msg) => {
                        log::error!(
                            "Couldn't canonicalize {}:\n{}",
                            path.display(),
                            msg
                        );
                        None
                    }
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
    entries
        .into_iter()
        .filter_map(|(path, entry)| {
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
    let id = meta.id.clone();

    if ids.contains(&id) {
        if let Some(nonempty) = collisions.get_mut(&id) {
            *nonempty += 1;
        } else {
            collisions.insert(id, 2);
        }
    } else {
        ids.insert(id.clone());
        path2meta.insert(path, meta);
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
