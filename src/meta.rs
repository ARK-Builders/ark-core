use crate::id::ResourceId;

use anyhow::Error;
use canonical_path::CanonicalPathBuf;
use std::ffi::{OsStr, OsString};
use std::time::SystemTime;
use walkdir::DirEntry;

#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct ResourceMeta {
    pub id: ResourceId,
    pub modified: SystemTime,
    pub name: Option<OsString>,
    pub extension: Option<OsString>,
    pub kind: Option<ResourceKind>,
    pub extra: Option<ResourceExtra>,
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
        let name = convert_str(path.file_name());
        let extension = convert_str(path.extension());
        let modified = metadata.modified()?;

        //todo
        let kind = None;
        let extra = None;

        let meta = ResourceMeta {
            id,
            modified,
            name,
            extension,
            kind,
            extra,
        };

        Ok((path.clone(), meta))
    }
}

//todo
pub type ResourceKind = ();
pub type ResourceExtra = ();

fn convert_str(option: Option<&OsStr>) -> Option<OsString> {
    if let Some(value) = option {
        return Some(value.to_os_string());
    }
    None
}
