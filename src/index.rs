use std::collections::HashMap;
use std::path::Path;

use canonical_path::CanonicalPathBuf;

use walkdir::{DirEntry, WalkDir};

use anyhow::Error;
use log;

use crate::id::ResourceId;
use crate::meta::ResourceMeta;

#[derive(Debug)]
pub struct ResourceIndex {
    pub id2path: HashMap<ResourceId, CanonicalPathBuf>,
    pub path2meta: HashMap<CanonicalPathBuf, ResourceMeta>,
    pub collisions: HashMap<ResourceId, usize>,
}

impl ResourceIndex {
    pub fn size(&self) -> usize {
        return self.id2path.len();
    }

    pub fn build<P: AsRef<Path>>(root_path: P) -> Result<Self, Error> {
        log::info!(
            "Calculating IDs of all files under path {}",
            root_path.as_ref().display()
        );

        let mut index = ResourceIndex {
            id2path: HashMap::new(),
            path2meta: HashMap::new(),
            collisions: HashMap::new(),
        };

        let all_files = WalkDir::new(root_path)
            .into_iter()
            .filter_entry(|e| !is_hidden(e));

        for entry in all_files {
            if let Ok((path, meta)) = ResourceMeta::scan(entry?) {
                let id = meta.id.clone();

                if index.id2path.contains_key(&id) {
                    if let Some(nonempty) = index.collisions.get_mut(&id) {
                        *nonempty += 1;
                    } else {
                        index.collisions.insert(id, 2);
                    }
                } else {
                    index.id2path.insert(id.clone(), path.clone());
                    index.path2meta.insert(path, meta);
                }
            }
        }

        log::info!("Index built");
        return Ok(index);
    }
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}
