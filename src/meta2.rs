use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use anyhow::Error;
use serde::Serialize;

use crate::id::ResourceId;

/// Dynamic metadata: stored as JSON and interpreted
/// differently depending on resource's kind
const METADATA_RELATIVE_PATH: &str = ".ark/meta";

pub fn store_meta<S: Serialize, P: AsRef<Path>>(
    root: P,
    id: ResourceId,
    extra: &S,
) {
    let metadata_path = root.as_ref().join(METADATA_RELATIVE_PATH);
    fs::create_dir_all(metadata_path.to_owned())
        .expect(&format!("Creating {} directory", METADATA_RELATIVE_PATH));
    let mut metadata_file = File::create(
        metadata_path
            .to_owned()
            .join(format!("{}-{}", id.data_size, id.crc32)),
    )
    .unwrap();

    // only dynamical metadata a.k.a. `extra` goes into `.ark/meta`
    let metadata_json = serde_json::to_string(&extra).unwrap();
    metadata_file
        .write(metadata_json.into_bytes().as_slice())
        .unwrap();
}

/// The file must exist if this method is called
pub fn load_meta_bytes<P: AsRef<Path>>(
    root: P,
    id: ResourceId,
) -> Result<Vec<u8>, Error> {
    let storage = root.as_ref().join(METADATA_RELATIVE_PATH);
    let path = storage.join(format!("{}-{}", id.data_size, id.crc32));

    Ok(std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    use std::collections::HashMap;
    type TestMetadata = HashMap<String, String>;

    #[test]
    fn test_store_and_load() {
        let dir = TempDir::new("arklib_test").unwrap();
        let root = dir.path();
        log::debug!("temporary root: {}", root.display());

        let id = ResourceId {
            crc32: 0x342a3d4a,
            data_size: 1,
        };

        let mut meta = TestMetadata::new();
        meta.insert("abc".to_string(), "def".to_string());
        meta.insert("xyz".to_string(), "123".to_string());

        store_meta(root, id, &meta);

        let bytes = load_meta_bytes(root, id).unwrap();
        let meta2: TestMetadata = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(meta, meta2);
    }
}
