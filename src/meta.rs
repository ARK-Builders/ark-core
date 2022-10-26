use crate::id::ResourceId;

use anyhow::Error;
use canonical_path::CanonicalPathBuf;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use std::ops::Add;
use std::time::{Duration, UNIX_EPOCH};
use walkdir::DirEntry;

#[derive(Eq, PartialEq, Hash, Clone, Debug, Deserialize, Serialize)]
pub struct ResourceMeta {
    pub id: ResourceId,
    pub modified: SystemTime,
}

pub const RESOURCE_META_DELIMITER: char = ':';

impl ResourceMeta {
    pub fn store(self) -> String {
        format!(
            "{} {}",
            self.id.store(),
            self.modified
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
        )
    }

    //todo: Option
    pub fn load(encoded: &str) -> Self {
        let mut parts = encoded.split(RESOURCE_META_DELIMITER);

        let id: ResourceId = ResourceId::load(parts.next().unwrap());
        let modified: SystemTime = UNIX_EPOCH.add(Duration::from_millis(
            parts.next().unwrap().parse().unwrap(),
        ));

        ResourceMeta {
            id,
            modified
        }
    }

    pub fn scan(
        path: CanonicalPathBuf,
        entry: DirEntry,
    ) -> Result<(CanonicalPathBuf, Self), Error> {
        if entry.file_type().is_dir() {
            return Err(Error::msg("DirEntry is directory"));
        }

        let metadata = entry.metadata()?;
        let size = metadata.len();
        if size == 0 {
            return Err(Error::msg("Empty resource"));
        }

        let id = ResourceId::compute(size, &path);
        let modified = metadata.modified()?;

        let meta = ResourceMeta { id, modified };

        Ok((path.clone(), meta))
    }
}
