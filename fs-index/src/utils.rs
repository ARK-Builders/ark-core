use std::{
    collections::HashMap,
    fs,
    io::BufReader,
    path::{Path, PathBuf},
};

use walkdir::{DirEntry, WalkDir};

use data_error::{ArklibError, Result};
use data_resource::ResourceId;
use fs_storage::{ARK_FOLDER, INDEX_PATH};

use crate::{index::Timestamped, ResourceIndex};

/// Load the index from the file system
fn load_index<P: AsRef<Path>, Id: ResourceId>(
    root_path: P,
) -> Result<ResourceIndex<Id>> {
    let index_path = Path::new(ARK_FOLDER).join(INDEX_PATH);
    let index_path = fs::canonicalize(root_path.as_ref())?.join(index_path);
    let index_file = fs::File::open(index_path)?;
    let reader = BufReader::new(index_file);
    let index = serde_json::from_reader(reader)?;

    Ok(index)
}

/// Load the index from the file system, or build a new index if it doesn't
/// exist
///
/// If `update` is true, the index will be updated and stored after loading
/// it.
pub fn load_or_build_index<P: AsRef<Path>, Id: ResourceId>(
    root_path: P,
    update: bool,
) -> Result<ResourceIndex<Id>> {
    log::debug!(
        "Attempting to load or build index at root path: {:?}",
        root_path.as_ref()
    );

    let index_path = Path::new(ARK_FOLDER).join(INDEX_PATH);
    let index_path = fs::canonicalize(root_path.as_ref())?.join(index_path);
    log::trace!("Index path: {:?}", index_path);

    if index_path.exists() {
        log::trace!("Index file exists, loading index");

        let mut index = load_index(root_path)?;
        if update {
            log::trace!("Updating loaded index");

            index.update_all()?;
            index.store()?;
        }
        Ok(index)
    } else {
        log::trace!("Index file does not exist, building index");

        // Build a new index if it doesn't exist and store it
        let index = ResourceIndex::build(root_path.as_ref())?;
        index.store().map_err(|e| {
            ArklibError::Path(format!("Failed to store index: {}", e))
        })?;
        Ok(index)
    }
}

/// A helper function to discover paths in a directory
///
/// This function walks the directory tree starting from the root path and
/// returns a list of file paths.
///
/// Ignore hidden files and empty files.
pub(crate) fn discover_paths<P: AsRef<Path>>(
    root_path: P,
) -> Result<Vec<DirEntry>> {
    log::debug!("Discovering paths at root path: {:?}", root_path.as_ref());

    let paths = WalkDir::new(root_path)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(should_index)
        .collect();

    Ok(paths)
}

/// A helper function to scan entries and create indexed resources
pub(crate) fn scan_entries<P: AsRef<Path>, Id: ResourceId>(
    root_path: P,
    paths: Vec<DirEntry>,
) -> HashMap<PathBuf, Timestamped<Id>> {
    let mut path_to_resource = HashMap::new();
    for entry in paths {
        let resource = scan_entry(entry.clone());

        let path = entry.path().to_path_buf();
        // Strip the root path from the entry path
        let path = path
            .strip_prefix(root_path.as_ref())
            .expect("Failed to strip prefix");
        let path = path.to_path_buf();

        path_to_resource.insert(path, resource);
    }
    path_to_resource
}

/// A helper function to scan one entry and create an indexed resource
pub(crate) fn scan_entry<Id: ResourceId>(entry: DirEntry) -> Timestamped<Id> {
    let metadata = entry.metadata().expect("Failed to get metadata");
    let last_modified = metadata
        .modified()
        .expect("Failed to get modified");

    // Get the ID of the resource
    let id = Id::from_path(entry.path()).expect("Failed to get ID from path");

    Timestamped {
        item: id,
        last_modified,
    }
}

/// A helper function to check if the entry should be indexed (not hidden or
/// empty)
fn should_index(entry: &walkdir::DirEntry) -> bool {
    // Check if the entry is hidden
    if entry
        .file_name()
        .to_string_lossy()
        .starts_with('.')
    {
        log::trace!("Ignoring hidden file: {:?}", entry.path());
        return false;
    }

    // Check if the entry is empty
    if entry
        .metadata()
        .map(|m| m.len() == 0)
        .unwrap_or(false)
    {
        log::trace!("Ignoring empty file: {:?}", entry.path());
        return false;
    }

    // Check if the entry isn't a file
    if !entry.file_type().is_file() {
        log::trace!("Ignoring non-file: {:?}", entry.path());
        return false;
    }

    // Check if it's the index file
    if entry.file_name() == INDEX_PATH {
        log::trace!("Ignoring index file: {:?}", entry.path());
        return false;
    }

    true
}
