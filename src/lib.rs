#[macro_use]
extern crate lazy_static;

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crc32fast::Hasher;
use walkdir::{DirEntry, WalkDir};

use anyhow::Error;
use log::trace;

pub const TAG_STORAGE_FILENAME: &str = ".ark-tags";

pub type ResourceId = u32;

lazy_static! {
    pub static ref INDEX: RwLock<HashMap<ResourceId, PathBuf>> =
        RwLock::new(HashMap::new());
    pub static ref COLLISIONS: RwLock<HashMap<ResourceId, usize>> =
        RwLock::new(HashMap::new());
}

pub fn build_index<P: AsRef<Path>>(root_path: P) -> Result<(), Error> {
    trace!(
        "Calculating IDs of all files under path {}",
        root_path.as_ref().display()
    );

    let mut index = INDEX.write().unwrap();
    let mut collisions = COLLISIONS.write().unwrap();

    let all_files = WalkDir::new(root_path)
        .into_iter()
        .filter_entry(|e| !is_hidden(e));

    for entry in all_files {
        if let Some((path, size)) = indexable(entry?) {
            let id = compute_id(size, &path);

            if index.contains_key(&id) {
                if let Some(nonempty) = collisions.get_mut(&id) {
                    *nonempty += 1;
                } else {
                    collisions.insert(id, 2);
                }
            } else {
                index.insert(id, path);
            }
        }
    }
    return Ok(());
}

pub fn compute_id<P: AsRef<Path>>(file_size: u64, file_path: P) -> u32 {
    trace!(
        "Calculating hash of {} (given size is {} megabytes)",
        file_path.as_ref().display(),
        file_size / MEGABYTE
    );

    let source = fs::OpenOptions::new()
        .read(true)
        .open(file_path.as_ref())
        .expect(&format!(
            "Failed to read from {}",
            file_path.as_ref().display()
        ));

    let mut reader = BufReader::with_capacity(BUFFER_CAPACITY, source);
    assert!(reader.buffer().is_empty());

    let mut hasher = Hasher::new();
    let mut bytes_read: u32 = 0;
    loop {
        let bytes_read_iteration: usize = reader
            .fill_buf()
            .expect(&format!(
                "Failed to read from {}",
                file_path.as_ref().display()
            ))
            .len();
        if bytes_read_iteration == 0 {
            break;
        }
        hasher.update(reader.buffer());
        reader.consume(bytes_read_iteration);
        bytes_read += u32::try_from(bytes_read_iteration).expect(&format!(
            "Failed to read from {}",
            file_path.as_ref().display()
        ))
    }

    let checksum: u32 = hasher.finalize().into();
    trace!("{} bytes has been read", bytes_read);
    trace!("checksum: {:#02x}", checksum);
    assert!(bytes_read == file_size.try_into().unwrap());

    return checksum;
}

const KILOBYTE: u64 = 1024;
const MEGABYTE: u64 = 1024 * KILOBYTE;
const BUFFER_CAPACITY: usize = 512 * KILOBYTE as usize;

fn indexable(entry: DirEntry) -> Option<(PathBuf, u64)> {
    if entry.file_type().is_dir() {
        return None;
    }

    if let Ok(meta) = entry.metadata() {
        let size = meta.len();
        if size == 0 {
            return None;
        }

        let path = entry.path().to_path_buf();

        return Some((path, size));
    } else {
        return None;
    }
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
    use super::*;

    #[test]
    fn compute_id_test() {
        let file_path = Path::new("./tests/lena.jpg");
        let file_size = fs::metadata(file_path)
            .expect(&format!(
                "Could not open image test file_path.{}",
                file_path.display()
            ))
            .len();

        let checksum = compute_id(file_size.try_into().unwrap(), file_path);
        assert_eq!(checksum, 0x342a3d4a);
    }
}
