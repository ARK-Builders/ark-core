use crate::id::ResourceId;

use anyhow::Error;
use canonical_path::CanonicalPathBuf;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use walkdir::DirEntry;

#[derive(Eq, PartialEq, Hash, Clone, Debug, Deserialize, Serialize)]
pub struct ResourceMeta {
    pub id: ResourceId,
    pub modified: SystemTime,
}

impl ResourceMeta {
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
