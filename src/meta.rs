use crate::id::ResourceId;

use anyhow::Error;
use canonical_path::CanonicalPathBuf;
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::SystemTime;
use walkdir::DirEntry;

const METADATA_RELATIVE_PATH: &str = ".ark/meta";

#[derive(Eq, PartialEq, Hash, Clone, Debug, Deserialize, Serialize)]
pub struct ResourceMeta {
    pub id: ResourceId,
    pub modified: SystemTime,
    pub kind: Option<ResourceKind>,
    pub extra: Option<ResourceExtra>,
}

impl ResourceMeta {
    pub fn store<P: AsRef<Path>>(
        root: P,
        path: P,
        resource_id: ResourceId,
        kind: Option<ResourceKind>,
        extra: Option<ResourceExtra>,
    ) {
        let metadata_path = root.as_ref().join(METADATA_RELATIVE_PATH);
        fs::create_dir_all(metadata_path.to_owned())
            .expect(&format!("Creating {} directory", METADATA_RELATIVE_PATH));
        let mut metadata_file = File::create(
            metadata_path
                .to_owned()
                .join(format!("{}-{}", fs::metadata(path).unwrap().len(), resource_id.crc32)),
        )
        .unwrap();
        let metadata = Self {
            id: resource_id,
            modified: SystemTime::now(),
            kind,
            extra,
        };
        let metadata_json = serde_json::to_string(&metadata).unwrap();
        metadata_file
            .write(metadata_json.into_bytes().as_slice())
            .unwrap();
    }

    pub fn locate<P: AsRef<Path>>(
        root: P,
        path: P,
        resource_id: ResourceId,
    ) -> Option<ResourceMeta> {
        let metadata_home = root.as_ref().join(METADATA_RELATIVE_PATH);
        let metadata_path =
            metadata_home.join(format!("{}-{}", fs::metadata(path).unwrap().len(), resource_id.crc32));

        if !metadata_path.exists() {
            return None;
        }
        let metadata_bytes = std::fs::read(metadata_path.to_owned()).unwrap();
        let meta: ResourceMeta =
            serde_json::from_slice(metadata_bytes.as_slice()).unwrap();
        Some(meta)
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

        let kind = None;
        let extra = None;

        let meta = ResourceMeta {
            id,
            modified: modified,
            kind,
            extra,
        };

        Ok((path.clone(), meta))
    }
}

pub type ResourceKind = Vec<u8>;
pub type ResourceExtra = ();

fn convert_str(option: Option<&OsStr>) -> Option<OsString> {
    if let Some(value) = option {
        return Some(value.to_os_string());
    }
    None
}


#[cfg(test)]
mod tests {
    use crate::meta;

    use super::*;

    #[test]
    fn overall() {
        use tempdir::TempDir;
        let dir = TempDir::new("arklib_test").unwrap();
        let tmp_path = dir.path();
        println!("temp path: {}", tmp_path.display());
        let file_path = Path::new("./tests/lena.jpg");
        let file_size = fs::metadata(file_path)
            .expect(&format!(
                "Could not open image test file_path.{}",
                file_path.display()
            ))
            .len();
        let resource_id = ResourceId::compute(file_size.try_into().unwrap(), file_path);
        ResourceMeta::store(tmp_path, file_path, resource_id.to_owned(), Some(vec![1,2,3,4].into()), None);
        let meta = ResourceMeta::locate(tmp_path, file_path, resource_id.to_owned()).unwrap();
        assert_eq!(meta.kind.unwrap(), vec![1,2,3,4])
    }
}